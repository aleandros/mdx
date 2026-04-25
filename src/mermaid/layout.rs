use super::{Direction, FlowChart, NodeShape};

const H_SPACING: usize = 6;
const V_SPACING: usize = 4;

#[derive(Debug, Clone)]
pub struct PositionedNode {
    #[allow(dead_code)]
    pub id: String,
    pub label: String,
    pub shape: super::NodeShape,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    /// Use compact rendering (3-row hexagon) for diamonds in LR/RL direction
    pub compact: bool,
    pub node_style: Option<super::NodeStyle>,
}

#[derive(Debug, Clone)]
pub struct PositionedEdge {
    #[allow(dead_code)]
    pub from: String,
    #[allow(dead_code)]
    pub to: String,
    pub label: Option<String>,
    pub style: super::EdgeStyle,
    pub points: Vec<(usize, usize)>,
    pub edge_style: Option<super::MermaidEdgeStyle>,
}

#[derive(Debug, Clone)]
pub struct SubgraphBox {
    pub label: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    /// Resolved border color; set by the caller (render_mermaid) after layout.
    pub border_color: Option<crate::render::Color>,
}

#[derive(Debug)]
pub struct LayoutResult {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
    pub subgraph_boxes: Vec<SubgraphBox>,
    pub width: usize,
    pub height: usize,
}

fn node_dimensions(label: &str, shape: &NodeShape, compact_diamond: bool) -> (usize, usize) {
    match shape {
        NodeShape::Rect | NodeShape::Rounded | NodeShape::Circle => (label.len() + 4, 3),
        NodeShape::Diamond => {
            if compact_diamond {
                // Compact 3-row hexagon for LR/RL
                (label.len() + 4, 3)
            } else {
                let inner_w = label.len() + 2;
                let half = inner_w.div_ceil(2);
                (inner_w + 2, half * 2 + 1)
            }
        }
    }
}

fn route_edge(
    start: (usize, usize),
    end: (usize, usize),
    vertical_primary: bool,
) -> Vec<(usize, usize)> {
    if start.0 == end.0 {
        return vec![start, end];
    }
    if start.1 == end.1 {
        return vec![start, end];
    }

    let dx = start.0.abs_diff(end.0);
    let dy = start.1.abs_diff(end.1);

    // Snap near-aligned edges to straight lines
    if dx <= 1 {
        return vec![(start.0, start.1), (start.0, end.1)];
    }
    if dy <= 1 {
        return vec![(start.0, start.1), (end.0, start.1)];
    }

    if vertical_primary {
        // TD/BT: vertical first, then horizontal
        let mid_y = (start.1 + end.1) / 2;
        vec![start, (start.0, mid_y), (end.0, mid_y), end]
    } else if start.1 > end.1 {
        // LR/RL upward edge: short horizontal step first, then vertical, then horizontal.
        // Keeps the horizontal segment at start.y short to avoid running along box bottom
        // borders, while keeping the vertical segment away from the source node border.
        const UPWARD_STEP: usize = 4;
        if end.0.saturating_sub(start.0) > UPWARD_STEP {
            let mid_x = start.0 + UPWARD_STEP;
            vec![start, (mid_x, start.1), (mid_x, end.1), end]
        } else {
            vec![start, (start.0, end.1), end]
        }
    } else {
        // LR/RL level/downward edge: horizontal first, then vertical
        let mid_x = (start.0 + end.0) / 2;
        vec![start, (mid_x, start.1), (mid_x, end.1), end]
    }
}

/// Returns a rank for each cluster ID. Subgraphs are clusters; free nodes each get
/// a synthetic singleton cluster ID `"__free__<node_id>"`.
/// Uses Kosaraju's SCC so cyclic clusters (bidirectional inter-cluster edges) share the same rank.
fn assign_cluster_ranks(chart: &FlowChart) -> std::collections::HashMap<String, usize> {
    let mut node_to_cluster: std::collections::HashMap<&str, String> =
        std::collections::HashMap::new();
    for sg in &chart.subgraphs {
        for nid in &sg.node_ids {
            node_to_cluster.insert(nid.as_str(), sg.id.clone());
        }
    }
    for node in &chart.nodes {
        node_to_cluster
            .entry(node.id.as_str())
            .or_insert_with(|| format!("__free__{}", node.id));
    }

    let mut seen = std::collections::HashSet::new();
    let mut cluster_ids: Vec<String> = Vec::new();
    for sg in &chart.subgraphs {
        if seen.insert(sg.id.clone()) {
            cluster_ids.push(sg.id.clone());
        }
    }
    for node in &chart.nodes {
        let cid = node_to_cluster[node.id.as_str()].clone();
        if seen.insert(cid.clone()) {
            cluster_ids.push(cid);
        }
    }

    let cluster_index: std::collections::HashMap<String, usize> = cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let nc = cluster_ids.len();

    let mut inter_edges: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();
    for edge in &chart.edges {
        let fc = node_to_cluster.get(edge.from.as_str());
        let tc = node_to_cluster.get(edge.to.as_str());
        if let (Some(fc), Some(tc)) = (fc, tc)
            && fc != tc
            && let (Some(&fi), Some(&ti)) = (cluster_index.get(fc), cluster_index.get(tc))
        {
            inter_edges.insert((fi, ti));
        }
    }

    // Build successor list for Kosaraju phase 1
    let mut successors: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(f, t) in &inter_edges {
        successors[f].push(t);
    }

    // Kosaraju phase 1: iterative DFS, record finish order
    let mut finished: Vec<usize> = Vec::new();
    let mut visited = vec![false; nc];
    for start in 0..nc {
        if visited[start] {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        visited[start] = true;
        while let Some((u, ni)) = stack.last_mut() {
            let u = *u;
            if *ni < successors[u].len() {
                let v = successors[u][*ni];
                *ni += 1;
                if !visited[v] {
                    visited[v] = true;
                    stack.push((v, 0));
                }
            } else {
                stack.pop();
                finished.push(u);
            }
        }
    }

    // Kosaraju phase 2: DFS on reverse graph in reverse finish order → SCCs
    let mut rev_successors: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(f, t) in &inter_edges {
        rev_successors[t].push(f);
    }
    let mut scc_id = vec![0usize; nc];
    let mut scc_count = 0usize;
    let mut visited2 = vec![false; nc];
    for &start in finished.iter().rev() {
        if visited2[start] {
            continue;
        }
        let mut stack = vec![start];
        visited2[start] = true;
        while let Some(u) = stack.pop() {
            scc_id[u] = scc_count;
            for &v in &rev_successors[u] {
                if !visited2[v] {
                    visited2[v] = true;
                    stack.push(v);
                }
            }
        }
        scc_count += 1;
    }

    // Build SCC DAG and run Kahn + longest-path (no cycles by construction)
    let mut scc_edges: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for &(f, t) in &inter_edges {
        let sf = scc_id[f];
        let st = scc_id[t];
        if sf != st {
            scc_edges.insert((sf, st));
        }
    }

    let mut scc_in_degree = vec![0usize; scc_count];
    let mut scc_successors: Vec<Vec<usize>> = vec![vec![]; scc_count];
    for &(f, t) in &scc_edges {
        scc_successors[f].push(t);
        scc_in_degree[t] += 1;
    }

    let mut scc_ranks = vec![0usize; scc_count];
    let mut queue = std::collections::VecDeque::new();
    for (i, &deg) in scc_in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }
    let mut remaining = scc_in_degree.clone();
    let mut processed = vec![false; scc_count];
    loop {
        while let Some(idx) = queue.pop_front() {
            processed[idx] = true;
            for &succ in &scc_successors[idx] {
                if !processed[succ] && scc_ranks[succ] < scc_ranks[idx] + 1 {
                    scc_ranks[succ] = scc_ranks[idx] + 1;
                }
                remaining[succ] = remaining[succ].saturating_sub(1);
                if remaining[succ] == 0 && !processed[succ] {
                    queue.push_back(succ);
                }
            }
        }
        if let Some(i) = (0..scc_count).find(|&i| !processed[i]) {
            remaining[i] = 0;
            queue.push_back(i);
        } else {
            break;
        }
    }

    cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), scc_ranks[scc_id[i]]))
        .collect()
}

pub fn layout(chart: &FlowChart) -> LayoutResult {
    if chart.nodes.is_empty() {
        return LayoutResult {
            nodes: vec![],
            edges: vec![],
            subgraph_boxes: vec![],
            width: 0,
            height: 0,
        };
    }

    // Build a mapping from node id to index in chart.nodes
    let node_index: std::collections::HashMap<&str, usize> = chart
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let n = chart.nodes.len();

    // Phase 1: Cluster-aware rank assignment
    let cluster_rank_map = assign_cluster_ranks(chart);

    // node id -> cluster id (same logic as assign_cluster_ranks)
    let mut node_to_cluster: std::collections::HashMap<&str, String> =
        std::collections::HashMap::new();
    for sg in &chart.subgraphs {
        for nid in &sg.node_ids {
            node_to_cluster.insert(nid.as_str(), sg.id.clone());
        }
    }
    for node in &chart.nodes {
        node_to_cluster
            .entry(node.id.as_str())
            .or_insert_with(|| format!("__free__{}", node.id));
    }

    // Group node indices by cluster
    let mut cluster_members: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, node) in chart.nodes.iter().enumerate() {
        let cid = node_to_cluster[node.id.as_str()].clone();
        cluster_members.entry(cid).or_default().push(i);
    }

    // Per-cluster internal rank via Kahn + longest-path (intra-cluster edges only)
    let mut internal_ranks = vec![0usize; n];
    let mut max_internal_depth = 0usize;

    for (cid, members) in &cluster_members {
        if members.len() == 1 {
            internal_ranks[members[0]] = 0;
            continue;
        }

        // Build declaration-position map: node global index → position in subgraph.node_ids
        let decl_pos: std::collections::HashMap<usize, usize> =
            if let Some(sg) = chart.subgraphs.iter().find(|s| s.id == cid.as_str()) {
                members
                    .iter()
                    .map(|&gi| {
                        let pos = sg
                            .node_ids
                            .iter()
                            .position(|nid| nid == chart.nodes[gi].id.as_str())
                            .unwrap_or(0);
                        (gi, pos)
                    })
                    .collect()
            } else {
                members
                    .iter()
                    .enumerate()
                    .map(|(li, &gi)| (gi, li))
                    .collect()
            };

        // Start from declaration order as the base rank
        for &gi in members.iter() {
            internal_ranks[gi] = decl_pos[&gi];
        }

        // Apply forward-edge constraints: only propagate rank for edges where
        // the "from" node appears before the "to" node in declaration order.
        // Backward edges (e.g. DLQ -.retry.-> CQ) are intentionally ignored.
        for _ in 0..members.len() {
            for edge in &chart.edges {
                if let (Some(&fi), Some(&ti)) = (
                    node_index.get(edge.from.as_str()),
                    node_index.get(edge.to.as_str()),
                ) && node_to_cluster.get(edge.from.as_str()) == Some(cid)
                    && node_to_cluster.get(edge.to.as_str()) == Some(cid)
                {
                    let from_decl = decl_pos.get(&fi).copied().unwrap_or(0);
                    let to_decl = decl_pos.get(&ti).copied().unwrap_or(0);
                    if from_decl <= to_decl && internal_ranks[ti] < internal_ranks[fi] + 1 {
                        internal_ranks[ti] = internal_ranks[fi] + 1;
                    }
                }
            }
        }

        let depth = members
            .iter()
            .map(|&gi| internal_ranks[gi])
            .max()
            .unwrap_or(0);
        if depth > max_internal_depth {
            max_internal_depth = depth;
        }
    }

    // Combine cluster rank + internal rank into global ranks
    let band_size = max_internal_depth + 1;
    let mut ranks = vec![0usize; n];
    for (i, node) in chart.nodes.iter().enumerate() {
        let cid = &node_to_cluster[node.id.as_str()];
        let cr = cluster_rank_map.get(cid).copied().unwrap_or(0);
        ranks[i] = cr * band_size + internal_ranks[i];
    }

    // Rebuild predecessors for Phase 2 barycenter
    let mut predecessors: Vec<Vec<usize>> = vec![vec![]; n];
    for edge in &chart.edges {
        if let (Some(&from_idx), Some(&to_idx)) = (
            node_index.get(edge.from.as_str()),
            node_index.get(edge.to.as_str()),
        ) {
            predecessors[to_idx].push(from_idx);
        }
    }

    // Phase 1.5: Assign secondary band index to each cluster.
    // Clusters with the same cluster_rank (e.g. due to SCC) get stacked vertically.
    // Only named subgraphs get separate bands; free-node clusters stay in band 0.
    let mut rank_sg_counter: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    let mut cluster_secondary_band: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for sg in &chart.subgraphs {
        let cr = cluster_rank_map.get(&sg.id).copied().unwrap_or(0);
        let cnt = rank_sg_counter.entry(cr).or_insert(0);
        cluster_secondary_band.insert(sg.id.clone(), *cnt);
        *cnt += 1;
    }
    for node in &chart.nodes {
        let cid = node_to_cluster[node.id.as_str()].clone();
        cluster_secondary_band.entry(cid).or_insert(0);
    }
    let max_secondary_band = *cluster_secondary_band.values().max().unwrap_or(&0);

    // Phase 2: Order within ranks (barycenter heuristic)
    let max_rank = *ranks.iter().max().unwrap_or(&0);
    let mut rank_groups: Vec<Vec<usize>> = vec![vec![]; max_rank + 1];
    for (i, &r) in ranks.iter().enumerate() {
        rank_groups[r].push(i);
    }

    // Rank 0: keep definition order (already pushed in node order)
    // For subsequent ranks: sort by barycenter of predecessors in previous rank

    // Build a position-within-rank map (updated as we assign order)
    let mut pos_in_rank: Vec<usize> = vec![0; n]; // position of node i within its rank

    // Initialize rank 0 positions
    for (pos, &idx) in rank_groups[0].iter().enumerate() {
        pos_in_rank[idx] = pos;
    }

    #[allow(clippy::needless_range_loop)]
    for r in 1..=max_rank {
        let group = &rank_groups[r];
        // Compute barycenter for each node: average position of predecessors in rank r-1
        let mut barycenters: Vec<(usize, f64)> = group
            .iter()
            .map(|&idx| {
                let preds_in_prev: Vec<usize> = predecessors[idx]
                    .iter()
                    .filter(|&&p| ranks[p] == r - 1)
                    .map(|&p| pos_in_rank[p])
                    .collect();
                let bc = if preds_in_prev.is_empty() {
                    // No predecessors in previous rank: use current index as tie-breaker
                    idx as f64
                } else {
                    preds_in_prev.iter().sum::<usize>() as f64 / preds_in_prev.len() as f64
                };
                (idx, bc)
            })
            .collect();

        barycenters.sort_by(|a, b| {
            let cid_a = node_to_cluster
                .get(chart.nodes[a.0].id.as_str())
                .map(|s| s.as_str())
                .unwrap_or("");
            let cid_b = node_to_cluster
                .get(chart.nodes[b.0].id.as_str())
                .map(|s| s.as_str())
                .unwrap_or("");
            let cr_a = cluster_rank_map.get(cid_a).copied().unwrap_or(0);
            let cr_b = cluster_rank_map.get(cid_b).copied().unwrap_or(0);
            let sb_a = cluster_secondary_band.get(cid_a).copied().unwrap_or(0);
            let sb_b = cluster_secondary_band.get(cid_b).copied().unwrap_or(0);
            cr_a.cmp(&cr_b)
                .then(sb_a.cmp(&sb_b))
                .then(a.1.partial_cmp(&b.1).unwrap())
                .then(a.0.cmp(&b.0))
        });

        rank_groups[r] = barycenters.iter().map(|(idx, _)| *idx).collect();

        for (pos, &idx) in rank_groups[r].iter().enumerate() {
            pos_in_rank[idx] = pos;
        }
    }

    // Determine whether rank axis is horizontal (LR/RL) or vertical (TD/BT)
    let is_lr = matches!(chart.direction, Direction::LeftRight | Direction::RightLeft);

    // Phase 3: Coordinate assignment
    // For each rank compute node dimensions and max height
    let dims: Vec<(usize, usize)> = chart
        .nodes
        .iter()
        .map(|n| node_dimensions(&n.label, &n.shape, is_lr))
        .collect();

    // For LR/RL: ranks go left→right (x-axis), within-rank nodes go top→bottom (y-axis).
    // For TD/BT: ranks go top→bottom (y-axis), within-rank nodes go left→right (x-axis).
    //
    // We compute a "primary" (rank axis) and "secondary" (within-rank axis) coordinate,
    // then map them to (x, y) at the end.

    // "Secondary" size of each node: width for TD, height for LR
    let node_secondary_size: Vec<usize> = dims
        .iter()
        .map(|&(w, h)| if is_lr { h } else { w })
        .collect();

    // "Primary" size of each node: height for TD, width for LR
    let node_primary_size: Vec<usize> = dims
        .iter()
        .map(|&(w, h)| if is_lr { w } else { h })
        .collect();

    // Spacing along secondary axis (between nodes in same rank)
    let secondary_spacing = if is_lr { V_SPACING } else { H_SPACING };
    // Spacing along primary axis (between ranks)
    let primary_spacing = if is_lr { H_SPACING } else { V_SPACING };

    // Total secondary extent of each rank
    let rank_secondary_extents: Vec<usize> = rank_groups
        .iter()
        .map(|group| {
            if group.is_empty() {
                return 0;
            }
            let total: usize = group.iter().map(|&idx| node_secondary_size[idx]).sum();
            let gaps = if group.len() > 1 {
                (group.len() - 1) * secondary_spacing
            } else {
                0
            };
            total + gaps
        })
        .collect();

    // When subgraphs are present, reserve top margin on the secondary axis so
    // the subgraph box top border (label row) has room above the first node row.
    let sg_top_margin: usize = if chart.subgraphs.is_empty() { 0 } else { 2 };

    // Primary offsets per rank
    let rank_max_primary: Vec<usize> = rank_groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|&idx| node_primary_size[idx])
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut rank_primary_offsets: Vec<usize> = vec![0; max_rank + 1];
    let mut current_primary = 0;
    for r in 0..=max_rank {
        rank_primary_offsets[r] = current_primary;
        current_primary += rank_max_primary[r] + primary_spacing;
    }

    // When multiple named subgraphs share the same cluster rank (SCC cycle case),
    // stack them in separate vertical bands instead of centering all nodes together.
    let node_primary;
    let node_secondary;
    let final_secondary_extent;

    if max_secondary_band > 0 {
        // Compute max secondary extent per band across all ranks
        let mut band_max_extents: Vec<usize> = vec![0; max_secondary_band + 1];
        for group in &rank_groups {
            let mut band_totals: std::collections::HashMap<usize, (usize, usize)> =
                std::collections::HashMap::new();
            for &idx in group {
                let cid = &node_to_cluster[chart.nodes[idx].id.as_str()];
                let sb = cluster_secondary_band
                    .get(cid.as_str())
                    .copied()
                    .unwrap_or(0);
                let e = band_totals.entry(sb).or_insert((0, 0));
                e.0 += node_secondary_size[idx];
                e.1 += 1;
            }
            for (sb, (total, count)) in band_totals {
                let extent = total + count.saturating_sub(1) * secondary_spacing;
                band_max_extents[sb] = band_max_extents[sb].max(extent);
            }
        }

        // Stack bands with sg_top_margin above each band and secondary_spacing between bands
        let mut band_offsets: Vec<usize> = vec![0; max_secondary_band + 1];
        let mut current_y = 0usize;
        for sb in 0..=max_secondary_band {
            band_offsets[sb] = current_y;
            current_y += band_max_extents[sb] + sg_top_margin + secondary_spacing;
        }
        final_secondary_extent = current_y.saturating_sub(secondary_spacing);

        let mut np = vec![0usize; n];
        let mut ns = vec![0usize; n];
        for (r, group) in rank_groups.iter().enumerate() {
            let mut band_cursors: Vec<usize> = (0..=max_secondary_band)
                .map(|sb| band_offsets[sb] + sg_top_margin)
                .collect();
            for &idx in group {
                let cid = &node_to_cluster[chart.nodes[idx].id.as_str()];
                let sb = cluster_secondary_band
                    .get(cid.as_str())
                    .copied()
                    .unwrap_or(0);
                np[idx] = rank_primary_offsets[r];
                ns[idx] = band_cursors[sb];
                band_cursors[sb] += node_secondary_size[idx] + secondary_spacing;
            }
        }
        node_primary = np;
        node_secondary = ns;
    } else {
        // Original centering: no co-ranked subgraphs
        let mut max_secondary_extent = *rank_secondary_extents.iter().max().unwrap_or(&0);
        max_secondary_extent += sg_top_margin;
        final_secondary_extent = max_secondary_extent;

        let mut np = vec![0usize; n];
        let mut ns = vec![0usize; n];
        for (r, group) in rank_groups.iter().enumerate() {
            let rank_extent = rank_secondary_extents[r];
            let secondary_offset = (max_secondary_extent - rank_extent) / 2;
            let mut cur_secondary = secondary_offset + sg_top_margin;
            for &idx in group {
                np[idx] = rank_primary_offsets[r];
                ns[idx] = cur_secondary;
                cur_secondary += node_secondary_size[idx] + secondary_spacing;
            }
        }
        node_primary = np;
        node_secondary = ns;
    }

    // Map primary/secondary → x/y based on direction.
    // For BT: reverse primary axis so highest rank is at top.
    // For RL: reverse primary axis so highest rank is at left.
    let total_primary = if max_rank + 1 > 0 {
        rank_primary_offsets[max_rank] + rank_max_primary[max_rank]
    } else {
        0
    };

    let node_x: Vec<usize>;
    let node_y: Vec<usize>;
    let node_width: Vec<usize>;
    let node_height: Vec<usize>;

    match chart.direction {
        Direction::TopDown => {
            // primary = y (rank axis), secondary = x (within-rank axis)
            node_x = node_secondary.clone();
            node_y = node_primary.clone();
            node_width = dims.iter().map(|&(w, _)| w).collect();
            node_height = dims.iter().map(|&(_, h)| h).collect();
        }
        Direction::BottomTop => {
            // primary = y but reversed: y = total_primary - primary_offset - node_height
            node_x = node_secondary.clone();
            node_y = (0..n)
                .map(|i| total_primary.saturating_sub(node_primary[i] + node_primary_size[i]))
                .collect();
            node_width = dims.iter().map(|&(w, _)| w).collect();
            node_height = dims.iter().map(|&(_, h)| h).collect();
        }
        Direction::LeftRight => {
            // primary = x (rank axis), secondary = y (within-rank axis)
            node_x = node_primary.clone();
            node_y = node_secondary.clone();
            node_width = dims.iter().map(|&(w, _)| w).collect();
            node_height = dims.iter().map(|&(_, h)| h).collect();
        }
        Direction::RightLeft => {
            // primary = x but reversed
            node_x = (0..n)
                .map(|i| total_primary.saturating_sub(node_primary[i] + node_primary_size[i]))
                .collect();
            node_y = node_secondary.clone();
            node_width = dims.iter().map(|&(w, _)| w).collect();
            node_height = dims.iter().map(|&(_, h)| h).collect();
        }
    }

    // Build positioned nodes
    let positioned_nodes: Vec<PositionedNode> = chart
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| PositionedNode {
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape.clone(),
            x: node_x[i],
            y: node_y[i],
            width: node_width[i],
            height: node_height[i],
            compact: is_lr && node.shape == NodeShape::Diamond,
            node_style: node.node_style.clone(),
        })
        .collect();

    // Build positioned edges
    let total_height = match chart.direction {
        Direction::TopDown => total_primary,
        Direction::BottomTop => total_primary,
        Direction::LeftRight | Direction::RightLeft => final_secondary_extent,
    };

    let total_width = match chart.direction {
        Direction::TopDown | Direction::BottomTop => final_secondary_extent,
        Direction::LeftRight | Direction::RightLeft => total_primary,
    };

    let positioned_edges: Vec<PositionedEdge> = chart
        .edges
        .iter()
        .filter_map(|edge| {
            let from_idx = *node_index.get(edge.from.as_str())?;
            let to_idx = *node_index.get(edge.to.as_str())?;
            let from_node = &positioned_nodes[from_idx];
            let to_node = &positioned_nodes[to_idx];

            let (start, end) = match chart.direction {
                Direction::TopDown => {
                    // center-bottom → center-top
                    let s = (
                        from_node.x + from_node.width / 2,
                        from_node.y + from_node.height,
                    );
                    let e = (to_node.x + to_node.width / 2, to_node.y);
                    (s, e)
                }
                Direction::BottomTop => {
                    // center-top of source → center-bottom of target (reversed ranks)
                    let s = (from_node.x + from_node.width / 2, from_node.y);
                    let e = (to_node.x + to_node.width / 2, to_node.y + to_node.height);
                    (s, e)
                }
                Direction::LeftRight => {
                    // center-right → center-left
                    let s = (
                        from_node.x + from_node.width,
                        from_node.y + from_node.height / 2,
                    );
                    let e = (to_node.x, to_node.y + to_node.height / 2);
                    (s, e)
                }
                Direction::RightLeft => {
                    // center-left → center-right
                    let s = (from_node.x, from_node.y + from_node.height / 2);
                    let e = (to_node.x + to_node.width, to_node.y + to_node.height / 2);
                    (s, e)
                }
            };

            let points = route_edge(start, end, !is_lr);

            Some(PositionedEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                label: edge.label.clone(),
                style: edge.style.clone(),
                points,
                edge_style: edge.edge_style.clone(),
            })
        })
        .collect();

    // Compute bounding boxes for subgraphs
    const H_PAD: usize = 3;
    const V_PAD: usize = 2;

    // Build a lookup from node id to PositionedNode for fast access
    let node_pos_map: std::collections::HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let subgraph_boxes: Vec<SubgraphBox> = chart
        .subgraphs
        .iter()
        .filter_map(|sg| {
            // Collect all positioned member nodes
            let members: Vec<&PositionedNode> = sg
                .node_ids
                .iter()
                .filter_map(|id| node_pos_map.get(id.as_str()).copied())
                .collect();

            if members.is_empty() {
                return None;
            }

            let min_x = members.iter().map(|n| n.x).min().unwrap();
            let min_y = members.iter().map(|n| n.y).min().unwrap();
            let max_x_right = members.iter().map(|n| n.x + n.width).max().unwrap();
            let max_y_bottom = members.iter().map(|n| n.y + n.height).max().unwrap();

            let box_x = min_x.saturating_sub(H_PAD);
            let box_y = min_y.saturating_sub(V_PAD);
            // Minimum width so the label always fits in the top border: "┌ label ┐"
            let min_w = sg.label.len() + 4;
            let box_w = (max_x_right + H_PAD - box_x).max(min_w);
            let box_h = max_y_bottom + V_PAD - box_y;

            Some(SubgraphBox {
                label: sg.label.clone(),
                x: box_x,
                y: box_y,
                width: box_w,
                height: box_h,
                border_color: None,
            })
        })
        .collect();

    let result_width = subgraph_boxes
        .iter()
        .map(|b| b.x + b.width)
        .max()
        .unwrap_or(0)
        .max(total_width);
    let result_height = subgraph_boxes
        .iter()
        .map(|b| b.y + b.height)
        .max()
        .unwrap_or(0)
        .max(total_height);

    LayoutResult {
        nodes: positioned_nodes,
        edges: positioned_edges,
        subgraph_boxes,
        width: result_width,
        height: result_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape};

    fn make_node(id: &str, label: &str) -> Node {
        Node {
            id: id.to_string(),
            label: label.to_string(),
            shape: NodeShape::Rect,
            node_style: None,
        }
    }

    fn make_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            style: EdgeStyle::Arrow,
            edge_style: None,
        }
    }

    fn simple_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::TopDown,
            nodes,
            edges,
            subgraphs: vec![],
        }
    }

    /// A->B->C: each node should be at a greater y than the previous, all same x
    #[test]
    fn test_linear_chain_ranks() {
        let chart = simple_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // y strictly increases along the chain
        assert!(a.y < b.y, "A.y ({}) should be less than B.y ({})", a.y, b.y);
        assert!(b.y < c.y, "B.y ({}) should be less than C.y ({})", b.y, c.y);

        // All at same x (single column, centered identically)
        assert_eq!(a.x, b.x, "A and B should have same x");
        assert_eq!(b.x, c.x, "B and C should have same x");
    }

    /// A->B, A->C: B and C are at the same y (same rank), different x
    #[test]
    fn test_branching_layout() {
        let chart = simple_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![make_edge("A", "B"), make_edge("A", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // A is rank 0, B and C are rank 1 → same y
        assert!(a.y < b.y, "A.y ({}) should be less than B.y ({})", a.y, b.y);
        assert_eq!(b.y, c.y, "B.y ({}) should equal C.y ({})", b.y, c.y);

        // B and C at different x
        assert_ne!(b.x, c.x, "B.x ({}) should differ from C.x ({})", b.x, c.x);
    }

    /// Single node: positioned at (0,0), correct dimensions
    #[test]
    fn test_single_node() {
        let chart = simple_chart(vec![make_node("N", "Hello")], vec![]);
        let result = layout(&chart);
        assert_eq!(result.nodes.len(), 1);
        let node = &result.nodes[0];

        assert_eq!(node.x, 0);
        assert_eq!(node.y, 0);
        // label "Hello" has len 5, so width = 5 + 4 = 9
        assert_eq!(node.width, 9);
        assert_eq!(node.height, 3);
    }

    /// A straight vertical edge (same column) should have the same x for both endpoints
    #[test]
    fn test_edge_points_straight() {
        let chart = simple_chart(
            vec![make_node("A", "A"), make_node("B", "B")],
            vec![make_edge("A", "B")],
        );
        let result = layout(&chart);
        assert_eq!(result.edges.len(), 1);
        let edge = &result.edges[0];
        // Both nodes are in a single column (linear chain), so edge is straight
        assert_eq!(edge.points.len(), 2, "Straight edge should have 2 points");
        assert_eq!(
            edge.points[0].0, edge.points[1].0,
            "Straight edge x coords must match"
        );
    }

    /// Diamond node: width >= label_len + 4, height >= 4
    #[test]
    fn test_diamond_dimensions() {
        let label = "Yes";
        let node = Node {
            id: "D".to_string(),
            label: label.to_string(),
            shape: NodeShape::Diamond,
            node_style: None,
        };
        let chart = simple_chart(vec![node], vec![]);
        let result = layout(&chart);
        let n = &result.nodes[0];
        assert!(
            n.width >= label.len() + 4,
            "Diamond width {} should be >= {}",
            n.width,
            label.len() + 4
        );
        assert!(n.height >= 4, "Diamond height {} should be >= 4", n.height);
    }

    /// All nodes must fit within the reported width and height
    #[test]
    fn test_layout_result_dimensions() {
        let chart = simple_chart(
            vec![
                make_node("A", "Alpha"),
                make_node("B", "Beta"),
                make_node("C", "Gamma"),
                make_node("D", "Delta"),
            ],
            vec![
                make_edge("A", "B"),
                make_edge("A", "C"),
                make_edge("B", "D"),
                make_edge("C", "D"),
            ],
        );
        let result = layout(&chart);

        for node in &result.nodes {
            assert!(
                node.x + node.width <= result.width || result.width == 0,
                "Node '{}' right edge {} exceeds result.width {}",
                node.id,
                node.x + node.width,
                result.width
            );
            assert!(
                node.y + node.height <= result.height,
                "Node '{}' bottom edge {} exceeds result.height {}",
                node.id,
                node.y + node.height,
                result.height
            );
        }
    }

    fn lr_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::LeftRight,
            nodes,
            edges,
            subgraphs: vec![],
        }
    }

    fn bt_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::BottomTop,
            nodes,
            edges,
            subgraphs: vec![],
        }
    }

    fn rl_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::RightLeft,
            nodes,
            edges,
            subgraphs: vec![],
        }
    }

    /// LR: A->B->C chain — x strictly increases along the chain, y stays same
    #[test]
    fn test_lr_linear_chain() {
        let chart = lr_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // x strictly increases (ranks go left→right)
        assert!(
            a.x < b.x,
            "LR: A.x ({}) should be less than B.x ({})",
            a.x,
            b.x
        );
        assert!(
            b.x < c.x,
            "LR: B.x ({}) should be less than C.x ({})",
            b.x,
            c.x
        );

        // All at same y (single row)
        assert_eq!(a.y, b.y, "LR: A and B should have same y");
        assert_eq!(b.y, c.y, "LR: B and C should have same y");
    }

    /// LR branching: A->B, A->C — B and C at same x (same rank), different y
    #[test]
    fn test_lr_branching() {
        let chart = lr_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![make_edge("A", "B"), make_edge("A", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // A rank 0, B/C rank 1 → B and C at same x
        assert!(
            a.x < b.x,
            "LR: A.x ({}) should be less than B.x ({})",
            a.x,
            b.x
        );
        assert_eq!(b.x, c.x, "LR: B.x ({}) should equal C.x ({})", b.x, c.x);

        // B and C at different y
        assert_ne!(
            b.y, c.y,
            "LR: B.y ({}) should differ from C.y ({})",
            b.y, c.y
        );
    }

    /// LR edge: should use center-right of source and center-left of target
    #[test]
    fn test_lr_edge_ports() {
        let chart = lr_chart(
            vec![make_node("A", "A"), make_node("B", "B")],
            vec![make_edge("A", "B")],
        );
        let result = layout(&chart);
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        let edge = &result.edges[0];

        // First point should be center-right of A
        assert_eq!(
            edge.points[0],
            (a.x + a.width, a.y + a.height / 2),
            "LR edge start should be center-right of source"
        );
        // Last point should be center-left of B
        assert_eq!(
            *edge.points.last().unwrap(),
            (b.x, b.y + b.height / 2),
            "LR edge end should be center-left of target"
        );
    }

    /// BT: A->B->C chain — y strictly decreases along the chain (A at bottom, C at top)
    #[test]
    fn test_bt_linear_chain() {
        let chart = bt_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // y strictly decreases: A at bottom (larger y), C at top (smaller y)
        assert!(
            a.y > b.y,
            "BT: A.y ({}) should be greater than B.y ({})",
            a.y,
            b.y
        );
        assert!(
            b.y > c.y,
            "BT: B.y ({}) should be greater than C.y ({})",
            b.y,
            c.y
        );

        // All at same x
        assert_eq!(a.x, b.x, "BT: A and B should have same x");
        assert_eq!(b.x, c.x, "BT: B and C should have same x");
    }

    /// RL: A->B->C chain — x strictly decreases along the chain (A at right, C at left)
    #[test]
    fn test_rl_linear_chain() {
        let chart = rl_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // x strictly decreases: A at right, C at left
        assert!(
            a.x > b.x,
            "RL: A.x ({}) should be greater than B.x ({})",
            a.x,
            b.x
        );
        assert!(
            b.x > c.x,
            "RL: B.x ({}) should be greater than C.x ({})",
            b.x,
            c.x
        );

        // All at same y
        assert_eq!(a.y, b.y, "RL: A and B should have same y");
        assert_eq!(b.y, c.y, "RL: B and C should have same y");
    }

    /// Two subgraphs with inter-cluster edge: Pkg (Send, Dispatch, Deliver) and MQ (DQ, CQ, DLQ).
    /// The layout should arrange nodes such that all nodes from one subgraph occupy a contiguous
    /// x-range that does not overlap with the other subgraph's x-range.
    #[test]
    fn test_subgraph_members_contiguous_ranks() {
        use crate::mermaid::Subgraph;

        let nodes = vec![
            Node {
                id: "Send".into(),
                label: "Send".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "Dispatch".into(),
                label: "Dispatch".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "Deliver".into(),
                label: "Deliver".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "DQ".into(),
                label: "DQ".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "CQ".into(),
                label: "CQ".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "DLQ".into(),
                label: "DLQ".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "Send".into(),
                to: "Dispatch".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "Dispatch".into(),
                to: "Deliver".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "DQ".into(),
                to: "CQ".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "CQ".into(),
                to: "DLQ".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "Send".into(),
                to: "DQ".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
        ];
        let subgraphs = vec![
            Subgraph {
                id: "Pkg".into(),
                label: "packages/notifications".into(),
                node_ids: vec!["Send".into(), "Dispatch".into(), "Deliver".into()],
            },
            Subgraph {
                id: "MQ".into(),
                label: "RabbitMQ".into(),
                node_ids: vec!["DQ".into(), "CQ".into(), "DLQ".into()],
            },
        ];
        let chart = FlowChart {
            direction: Direction::LeftRight,
            nodes,
            edges,
            subgraphs,
        };
        let result = layout(&chart);

        let pkg_ids = ["Send", "Dispatch", "Deliver"];
        let mq_ids = ["DQ", "CQ", "DLQ"];
        let find_x = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().x;
        let pkg_xs: Vec<usize> = pkg_ids.iter().map(|id| find_x(id)).collect();
        let mq_xs: Vec<usize> = mq_ids.iter().map(|id| find_x(id)).collect();
        let pkg_max = pkg_xs.iter().max().unwrap();
        let mq_min = mq_xs.iter().min().unwrap();
        let pkg_min = pkg_xs.iter().min().unwrap();
        let mq_max = mq_xs.iter().max().unwrap();
        assert!(
            pkg_max < mq_min || mq_max < pkg_min,
            "Pkg and MQ x-ranges must not overlap. Pkg xs: {:?}, MQ xs: {:?}",
            pkg_xs,
            mq_xs
        );
    }

    #[test]
    fn test_subgraph_secondary_axis_grouping() {
        // Two subgraphs each with 2 nodes, inter-cluster edge A1->B1.
        // After compound layout: SGA members must be in the same rank column (same x),
        // SGB members must be in the same rank column, and the two columns must differ.
        use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
        let nodes = vec![
            Node {
                id: "A1".into(),
                label: "A1".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "A2".into(),
                label: "A2".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "B1".into(),
                label: "B1".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "B2".into(),
                label: "B2".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
        ];
        let edges = vec![Edge {
            from: "A1".into(),
            to: "B1".into(),
            label: None,
            style: EdgeStyle::Arrow,
            edge_style: None,
        }];
        let subgraphs = vec![
            Subgraph {
                id: "SGA".into(),
                label: "Group A".into(),
                node_ids: vec!["A1".into(), "A2".into()],
            },
            Subgraph {
                id: "SGB".into(),
                label: "Group B".into(),
                node_ids: vec!["B1".into(), "B2".into()],
            },
        ];
        let chart = FlowChart {
            direction: Direction::LeftRight,
            nodes,
            edges,
            subgraphs,
        };
        let result = layout(&chart);

        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a1 = find("A1");
        let a2 = find("A2");
        let b1 = find("B1");
        let b2 = find("B2");

        // With declaration-order internal ranks, A1 and A2 are at consecutive x positions.
        // Both SGA columns must appear before both SGB columns (x-range non-overlap).
        let a_max_x = a1.x.max(a2.x);
        let b_min_x = b1.x.min(b2.x);
        let a_min_x = a1.x.min(a2.x);
        let b_max_x = b1.x.max(b2.x);
        assert!(
            a_max_x < b_min_x || b_max_x < a_min_x,
            "SGA and SGB x-ranges must not overlap. SGA xs: [{},{}], SGB xs: [{},{}]",
            a_min_x,
            a_max_x,
            b_min_x,
            b_max_x
        );
        // SGA (rank 0) must come before SGB (rank 1) in LR mode
        assert!(
            a_min_x < b_min_x,
            "SGA columns must be left of SGB columns in LR mode"
        );
    }

    #[test]
    fn test_subgraph_box_compact() {
        use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
        let nodes = vec![
            Node {
                id: "S".into(),
                label: "Send".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "D".into(),
                label: "Dispatch".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "Q".into(),
                label: "Queue".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "C".into(),
                label: "Consume".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "S".into(),
                to: "D".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "S".into(),
                to: "Q".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "Q".into(),
                to: "C".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
        ];
        let subgraphs = vec![
            Subgraph {
                id: "SGA".into(),
                label: "App".into(),
                node_ids: vec!["S".into(), "D".into()],
            },
            Subgraph {
                id: "SGB".into(),
                label: "Broker".into(),
                node_ids: vec!["Q".into(), "C".into()],
            },
        ];
        let chart = FlowChart {
            direction: Direction::LeftRight,
            nodes,
            edges,
            subgraphs,
        };
        let result = layout(&chart);
        for sg_box in &result.subgraph_boxes {
            assert!(
                sg_box.width <= result.width,
                "SubgraphBox '{}' width {} > diagram width {}",
                sg_box.label,
                sg_box.width,
                result.width
            );
            assert!(
                sg_box.height <= result.height,
                "SubgraphBox '{}' height {} > diagram height {}",
                sg_box.label,
                sg_box.height,
                result.height
            );
        }
    }

    #[test]
    fn test_free_nodes_with_subgraphs() {
        use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
        let nodes = vec![
            Node {
                id: "Free".into(),
                label: "Free".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "A".into(),
                label: "A".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "B".into(),
                label: "B".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "C".into(),
                label: "C".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "D".into(),
                label: "D".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "A".into(),
                to: "B".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "C".into(),
                to: "D".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "A".into(),
                to: "C".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
        ];
        let subgraphs = vec![
            Subgraph {
                id: "SGA".into(),
                label: "G1".into(),
                node_ids: vec!["A".into(), "B".into()],
            },
            Subgraph {
                id: "SGB".into(),
                label: "G2".into(),
                node_ids: vec!["C".into(), "D".into()],
            },
        ];
        let chart = FlowChart {
            direction: Direction::LeftRight,
            nodes,
            edges,
            subgraphs,
        };
        let result = layout(&chart);
        let find_x = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().x;
        let a_x = find_x("A");
        let b_x = find_x("B");
        let c_x = find_x("C");
        let d_x = find_x("D");
        let sga_max = a_x.max(b_x);
        let sgb_min = c_x.min(d_x);
        let sga_min = a_x.min(b_x);
        let sgb_max = c_x.max(d_x);
        assert!(
            sga_max < sgb_min || sgb_max < sga_min,
            "SGA and SGB x-ranges must not overlap. SGA: [{},{}], SGB: [{},{}]",
            sga_min,
            sga_max,
            sgb_min,
            sgb_max
        );
        assert_eq!(result.nodes.len(), 5);
    }

    #[test]
    fn test_single_subgraph_no_regression() {
        use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
        let nodes = vec![
            Node {
                id: "A".into(),
                label: "A".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "B".into(),
                label: "B".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
            Node {
                id: "C".into(),
                label: "C".into(),
                shape: NodeShape::Rect,
                node_style: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "A".into(),
                to: "B".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
            Edge {
                from: "B".into(),
                to: "C".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
            },
        ];
        let subgraphs = vec![Subgraph {
            id: "SG".into(),
            label: "All".into(),
            node_ids: vec!["A".into(), "B".into(), "C".into()],
        }];
        let chart = FlowChart {
            direction: Direction::LeftRight,
            nodes,
            edges,
            subgraphs,
        };
        let result = layout(&chart);
        let find_x = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().x;
        assert!(find_x("A") < find_x("B"), "A.x should be less than B.x");
        assert!(find_x("B") < find_x("C"), "B.x should be less than C.x");
    }
}
