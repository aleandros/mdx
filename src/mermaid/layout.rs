use super::{Direction, FlowChart, NodeShape, display_width};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

const H_SPACING: usize = 6;
const V_SPACING: usize = 4;
/// Secondary-axis gap kept on either side of a virtual (edge-routing) node.
/// Smaller than the node spacing so long edges cost little room.
const VIRTUAL_GAP: usize = 2;
// Every inter-rank gap is divided into lanes, counted in cells from the
// source-side rank border (`o0` is the cell touching the source rank):
//
//   TD/BT   o0 stubs out of the source · [box border] · [side lane] · labels ·
//           forward bends · [in lane] · [box border] · arrowheads
//   LR/RL   o0 stubs · o1 side lane · [box border] · forward bends at s/2 ·
//           in lane at s-2 · [box border at s-3] · arrowheads at s-1;
//           labels are written across the gap
//
// The base gap (`V_SPACING` / `H_SPACING`) holds stubs, labels, bends and
// arrowheads. A TD/BT gap grows by one row for each optional lane it needs:
// the side lane carries back edges leaving, same-rank edges and self-loops;
// the in lane carries back edges arriving at the next rank. When the chart
// has subgraphs, the cells where a box border can fall (`V_PAD` / `H_PAD`
// from the nodes) are reserved too, so no edge runs along a border. An LR/RL
// gap grows to fit the widest label written across it.

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
    /// Set only for ER entity boxes; carries pre-rendered attribute rows.
    pub entity: Option<super::er::Entity>,
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
    /// Top-left cell where the label is written, chosen by the layout so it
    /// sits in free gap space. `None` lets the painter pick a spot from the
    /// points alone.
    pub label_pos: Option<(usize, usize)>,
    pub edge_style: Option<super::MermaidEdgeStyle>,
    /// Set only for ER relationship edges; carries cardinality + identifying flag.
    pub er_meta: Option<super::er::ErEdgeMeta>,
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

fn node_dimensions(
    label: &str,
    shape: &NodeShape,
    compact_diamond: bool,
    entity: Option<&super::er::Entity>,
) -> (usize, usize) {
    let label_w = display_width(label);
    match shape {
        NodeShape::EntityBox => {
            if let Some(e) = entity {
                (e.width, e.height)
            } else {
                (label_w + 4, 3)
            }
        }
        NodeShape::Rect | NodeShape::Rounded | NodeShape::Circle => (label_w + 4, 3),
        NodeShape::Diamond => {
            if compact_diamond {
                // Compact 3-row hexagon for LR/RL
                (label_w + 4, 3)
            } else {
                let inner_w = label_w + 2;
                let half = inner_w.div_ceil(2);
                (inner_w + 2, half * 2 + 1)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ranking
// ---------------------------------------------------------------------------

/// Kosaraju's algorithm over `nc` vertices. Returns the component id of each
/// vertex and the number of components.
fn strongly_connected_components(
    nc: usize,
    edges: &BTreeSet<(usize, usize)>,
) -> (Vec<usize>, usize) {
    let mut successors: Vec<Vec<usize>> = vec![vec![]; nc];
    let mut rev_successors: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(f, t) in edges {
        successors[f].push(t);
        rev_successors[t].push(f);
    }

    // Phase 1: iterative DFS, record finish order
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

    // Phase 2: DFS on the reverse graph in reverse finish order → SCCs
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
    (scc_id, scc_count)
}

/// Removes the edges that close cycles so the ranking graph becomes acyclic.
///
/// Within each strongly connected component, a depth-first walk starting from
/// the earliest-declared member (subgraphs first, then free nodes, visiting
/// successors in declaration order) marks the edges that point back to a
/// cluster still on the DFS stack; those are dropped from the ranking graph so
/// the component unrolls into a chain. `A-->B-->C-->A` ranks A, B, C on
/// successive ranks and `C-->A` is later drawn as a back edge, which matches
/// what Mermaid does.
///
/// Components that contain two or more named subgraphs are left alone: they
/// keep the SCC grouping so bidirectional inter-subgraph edges still place
/// the subgraphs side by side on one cluster rank.
fn drop_free_node_back_edges(nc: usize, edges: &mut BTreeSet<(usize, usize)>, is_free: &[bool]) {
    let (scc_id, scc_count) = strongly_connected_components(nc, edges);
    let mut members: Vec<Vec<usize>> = vec![vec![]; scc_count];
    for v in 0..nc {
        members[scc_id[v]].push(v);
    }
    // BTreeSet iteration is sorted by (from, to), so successor lists come out
    // in declaration order.
    let mut successors: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(f, t) in edges.iter() {
        successors[f].push(t);
    }

    let mut to_remove: Vec<(usize, usize)> = Vec::new();
    for comp in &members {
        let subgraphs = comp.iter().filter(|&&v| !is_free[v]).count();
        if comp.len() < 2 || subgraphs >= 2 {
            continue;
        }
        // 0 = unvisited, 1 = on the DFS stack, 2 = finished
        let mut state = vec![0u8; nc];
        let start = comp[0];
        state[start] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some((u, ni)) = stack.last_mut() {
            let u = *u;
            if *ni < successors[u].len() {
                let v = successors[u][*ni];
                *ni += 1;
                if scc_id[v] != scc_id[u] {
                    continue;
                }
                match state[v] {
                    0 => {
                        state[v] = 1;
                        stack.push((v, 0));
                    }
                    1 => to_remove.push((u, v)),
                    _ => {}
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }
    for e in to_remove {
        edges.remove(&e);
    }
}

/// Returns a rank for each cluster ID. Subgraphs are clusters; free nodes each get
/// a synthetic singleton cluster ID `"__free__<node_id>"`.
///
/// Cycles among free nodes are broken by declaration order (see
/// [`drop_free_node_back_edges`]). Remaining cycles, which always involve a
/// named subgraph, are condensed with Kosaraju's SCC so the cyclic clusters
/// share one rank. The condensed DAG is ranked by longest path.
fn assign_cluster_ranks(chart: &FlowChart) -> HashMap<String, usize> {
    let mut node_to_cluster: HashMap<&str, String> = HashMap::new();
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

    let mut seen = HashSet::new();
    let mut cluster_ids: Vec<String> = Vec::new();
    for sg in &chart.subgraphs {
        if seen.insert(sg.id.clone()) {
            cluster_ids.push(sg.id.clone());
        }
    }
    let subgraph_cluster_count = cluster_ids.len();
    for node in &chart.nodes {
        let cid = node_to_cluster[node.id.as_str()].clone();
        if seen.insert(cid.clone()) {
            cluster_ids.push(cid);
        }
    }

    let cluster_index: HashMap<String, usize> = cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let nc = cluster_ids.len();
    let is_free: Vec<bool> = (0..nc).map(|i| i >= subgraph_cluster_count).collect();

    let mut inter_edges: BTreeSet<(usize, usize)> = BTreeSet::new();
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

    drop_free_node_back_edges(nc, &mut inter_edges, &is_free);
    let (scc_id, scc_count) = strongly_connected_components(nc, &inter_edges);

    // Build SCC DAG and run Kahn + longest-path (only multi-subgraph cycles
    // remain, condensed into one SCC each)
    let mut scc_edges: BTreeSet<(usize, usize)> = BTreeSet::new();
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
    let mut queue = VecDeque::new();
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

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Lays out `seq` along one axis: returns the offset of each item and the
/// total extent. Real nodes are separated by `spacing`; a gap next to a
/// virtual node shrinks to `VIRTUAL_GAP`.
fn place_sequence(
    seq: &[usize],
    size: &[usize],
    is_virtual: &dyn Fn(usize) -> bool,
    spacing: usize,
) -> (Vec<usize>, usize) {
    let mut offsets = Vec::with_capacity(seq.len());
    let mut cur = 0usize;
    for (k, &idx) in seq.iter().enumerate() {
        offsets.push(cur);
        cur += size[idx];
        if let Some(&next) = seq.get(k + 1) {
            cur += if is_virtual(idx) || is_virtual(next) {
                VIRTUAL_GAP
            } else {
                spacing
            };
        }
    }
    (offsets, cur)
}

/// Drops repeated points and merges runs of collinear axis-aligned segments.
fn simplify_path(points: Vec<(isize, isize)>) -> Vec<(isize, isize)> {
    let mut out: Vec<(isize, isize)> = Vec::with_capacity(points.len());
    for p in points {
        if out.last() == Some(&p) {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let same_col = a.0 == b.0 && b.0 == p.0;
            let same_row = a.1 == b.1 && b.1 == p.1;
            if same_col || same_row {
                // Only merge when the direction is preserved (no doubling back).
                let forward = if same_col {
                    (b.1 - a.1).signum() == (p.1 - b.1).signum()
                } else {
                    (b.0 - a.0).signum() == (p.0 - b.0).signum()
                };
                if forward {
                    out.pop();
                }
            }
        }
        out.push(p);
    }
    out
}

/// A point in (secondary, primary) or (x, y) space before the final shift.
type Pt = (isize, isize);
/// A routed edge: its polyline and, when labelled, the label's anchor cell.
type RoutedEdge = (Vec<Pt>, Option<Pt>);

/// How an edge is drawn relative to the rank axis.
enum EdgeRoute {
    /// Target rank is greater than source rank; `virtuals` are the routing
    /// nodes inserted on each skipped rank (empty for adjacent ranks).
    Forward { virtuals: Vec<usize> },
    /// Target rank is lower than the source rank: drawn around the side of
    /// the diagram.
    Back,
    /// Both endpoints share a rank: drawn through the gap after that rank.
    SameRank,
    /// `A --> A`.
    SelfLoop,
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

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
    let node_index: HashMap<&str, usize> = chart
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let n = chart.nodes.len();

    // Phase 1: Cluster-aware rank assignment
    let cluster_rank_map = assign_cluster_ranks(chart);

    // node id -> cluster id (same logic as assign_cluster_ranks)
    let mut node_to_cluster: HashMap<&str, String> = HashMap::new();
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
    let mut cluster_members: HashMap<String, Vec<usize>> = HashMap::new();
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
        let decl_pos: HashMap<usize, usize> =
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

        // Longest path over intra-cluster edges, like free nodes: every
        // member starts at rank 0 and forward edges push their targets down.
        for &gi in members.iter() {
            internal_ranks[gi] = 0;
        }

        // Only propagate rank for edges where the "from" node appears before
        // the "to" node in declaration order; backward edges (e.g.
        // DLQ -.retry.-> CQ) are ignored so the propagation terminates.
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
    // `cluster rank * band_size` leaves unused rank values whenever a cluster
    // is shallower than the deepest one (a free node next to a 3-deep
    // subgraph sits 3 ranks before its first member). Compact them so adjacent
    // nodes are on adjacent ranks and no edge is split for an empty rank.
    {
        let used: BTreeSet<usize> = ranks.iter().copied().collect();
        let dense: HashMap<usize, usize> =
            used.into_iter().enumerate().map(|(d, r)| (r, d)).collect();
        for r in &mut ranks {
            *r = dense[r];
        }
    }

    // Phase 1.5: Assign secondary band index to each cluster.
    // Clusters with the same cluster_rank (e.g. due to SCC) get stacked vertically.
    // Only named subgraphs get separate bands; free-node clusters stay in band 0.
    let mut rank_sg_counter: HashMap<usize, usize> = HashMap::new();
    let mut cluster_secondary_band: HashMap<String, usize> = HashMap::new();
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

    // Phase 1.75: Classify edges and insert virtual nodes.
    //
    // Layout "items" are the real nodes (indices 0..n) followed by virtual
    // nodes (n..). A forward edge that skips ranks gets one 1x1 virtual node
    // per skipped rank; the virtual takes part in ordering and reserves a
    // column (TD/BT) or row (LR/RL) so the edge can cross the rank without
    // touching any node there.
    let mut item_rank: Vec<usize> = ranks.clone();
    let mut item_band: Vec<usize> = chart
        .nodes
        .iter()
        .map(|node| {
            let cid = &node_to_cluster[node.id.as_str()];
            cluster_secondary_band.get(cid).copied().unwrap_or(0)
        })
        .collect();
    let mut item_preds: Vec<Vec<usize>> = vec![vec![]; n];
    // When the chart has subgraphs, long edges between different clusters are
    // routed through a dedicated band past every box, so their lanes never
    // run inside (or along the border of) a box they do not belong to.
    let routing_band = max_secondary_band + 1;
    let mut routing_band_used = false;
    let mut routes: Vec<Option<(usize, usize, EdgeRoute)>> = Vec::with_capacity(chart.edges.len());
    for edge in &chart.edges {
        let (Some(&fi), Some(&ti)) = (
            node_index.get(edge.from.as_str()),
            node_index.get(edge.to.as_str()),
        ) else {
            routes.push(None);
            continue;
        };
        let route = if fi == ti {
            EdgeRoute::SelfLoop
        } else {
            let (rf, rt) = (ranks[fi], ranks[ti]);
            if rt > rf {
                let same_cluster = node_to_cluster[chart.nodes[fi].id.as_str()]
                    == node_to_cluster[chart.nodes[ti].id.as_str()];
                let band = if chart.subgraphs.is_empty() || same_cluster {
                    item_band[fi]
                } else {
                    routing_band
                };
                let mut prev = fi;
                let mut virtuals = Vec::with_capacity(rt - rf - 1);
                for r in rf + 1..rt {
                    let v = item_rank.len();
                    item_rank.push(r);
                    item_band.push(band);
                    item_preds.push(vec![prev]);
                    virtuals.push(v);
                    prev = v;
                    routing_band_used |= band == routing_band;
                }
                item_preds[ti].push(prev);
                EdgeRoute::Forward { virtuals }
            } else if rt == rf {
                EdgeRoute::SameRank
            } else {
                EdgeRoute::Back
            }
        };
        routes.push(Some((fi, ti, route)));
    }
    let total_items = item_rank.len();
    let is_virtual = |idx: usize| idx >= n;
    let max_secondary_band = if routing_band_used {
        routing_band
    } else {
        max_secondary_band
    };

    // Phase 2: Order within ranks (barycenter heuristic)
    let max_rank = *item_rank.iter().max().unwrap_or(&0);
    let mut rank_groups: Vec<Vec<usize>> = vec![vec![]; max_rank + 1];
    for (i, &r) in item_rank.iter().enumerate() {
        rank_groups[r].push(i);
    }

    // Rank 0: keep definition order (already pushed in node order)
    // For subsequent ranks: sort by barycenter of predecessors in previous rank

    // Build a position-within-rank map (updated as we assign order)
    let mut pos_in_rank: Vec<usize> = vec![0; total_items];

    // Initialize rank 0 positions
    for (pos, &idx) in rank_groups[0].iter().enumerate() {
        pos_in_rank[idx] = pos;
    }

    #[allow(clippy::needless_range_loop)]
    for r in 1..=max_rank {
        let group = &rank_groups[r];
        // Compute barycenter for each item: average position of predecessors in rank r-1
        let mut barycenters: Vec<(usize, f64)> = group
            .iter()
            .map(|&idx| {
                let preds_in_prev: Vec<usize> = item_preds[idx]
                    .iter()
                    .filter(|&&p| item_rank[p] == r - 1)
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

        // Every item in a rank shares the same cluster rank (rank = cluster
        // rank * band_size + internal rank), so the band is the first key.
        barycenters.sort_by(|a, b| {
            item_band[a.0]
                .cmp(&item_band[b.0])
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
    let reversed = matches!(chart.direction, Direction::BottomTop | Direction::RightLeft);

    // Phase 3: Coordinate assignment
    // For each rank compute node dimensions and max height
    let dims: Vec<(usize, usize)> = chart
        .nodes
        .iter()
        .map(|n| node_dimensions(&n.label, &n.shape, is_lr, n.entity.as_ref()))
        .collect();

    // For LR/RL: ranks go left→right (x-axis), within-rank nodes go top→bottom (y-axis).
    // For TD/BT: ranks go top→bottom (y-axis), within-rank nodes go left→right (x-axis).
    //
    // We compute a "primary" (rank axis) and "secondary" (within-rank axis) coordinate,
    // then map them to (x, y) at the end.

    // "Secondary" size of each item: width for TD, height for LR; 1 for virtuals
    let item_secondary_size: Vec<usize> = (0..total_items)
        .map(|i| {
            if is_virtual(i) {
                1
            } else if is_lr {
                dims[i].1
            } else {
                dims[i].0
            }
        })
        .collect();

    // "Primary" size of each item: height for TD, width for LR; 1 for virtuals
    let item_primary_size: Vec<usize> = (0..total_items)
        .map(|i| {
            if is_virtual(i) {
                1
            } else if is_lr {
                dims[i].0
            } else {
                dims[i].1
            }
        })
        .collect();

    // Spacing along secondary axis (between nodes in same rank)
    let secondary_spacing = if is_lr { V_SPACING } else { H_SPACING };
    // Spacing along primary axis (between ranks)
    let primary_spacing = if is_lr { H_SPACING } else { V_SPACING };

    // When subgraphs are present, reserve top margin on the secondary axis so
    // the subgraph box top border (label row) has room above the first node row.
    let sg_top_margin: usize = if chart.subgraphs.is_empty() { 0 } else { 2 };

    // Primary offsets per rank
    let rank_max_primary: Vec<usize> = rank_groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|&idx| item_primary_size[idx])
                .max()
                .unwrap_or(0)
        })
        .collect();

    // Size each inter-rank gap from the lanes it has to carry (see the lane
    // table at the top of the file). `gap_side[r]` / `gap_in[r]` describe the
    // gap after rank r; the pseudo-gaps before rank 0 and after the last rank
    // only ever hold side/in lanes and are not sized here.
    let gap_count = max_rank; // gaps between consecutive ranks
    let mut gap_side = vec![false; gap_count + 1];
    let mut gap_in = vec![false; gap_count + 1];
    let mut gap_label_w = vec![0usize; gap_count + 1];
    for route in routes.iter().flatten() {
        let (fi, ti, route) = route;
        let (rf, rt) = (ranks[*fi], ranks[*ti]);
        match route {
            EdgeRoute::Forward { .. } => {}
            EdgeRoute::Back => {
                gap_side[rf] = true;
                if rt > 0 {
                    gap_in[rt - 1] = true;
                }
            }
            EdgeRoute::SameRank | EdgeRoute::SelfLoop => gap_side[rf] = true,
        }
    }
    if is_lr {
        for (ei, route) in routes.iter().enumerate() {
            if let Some((fi, _, EdgeRoute::Forward { .. })) = route
                && let Some(label) = chart.edges[ei].label.as_deref()
                && !label.is_empty()
            {
                // Keep the label clear of the cells at both ends of the gap:
                // one line cell each side, plus the two-cell cardinality
                // glyphs on ER edges or the subgraph box padding.
                let margin = if chart.edges[ei].er_meta.is_some() || !chart.subgraphs.is_empty() {
                    3
                } else {
                    1
                };
                let rf = ranks[*fi];
                gap_label_w[rf] = gap_label_w[rf].max(display_width(label) + 2 * margin);
            }
        }
    }
    // Cells reserved for subgraph box borders on each side of a gap.
    let has_sg = usize::from(!chart.subgraphs.is_empty());
    let gap_size: Vec<usize> = (0..=gap_count)
        .map(|r| {
            if is_lr {
                (primary_spacing + 2 * has_sg).max(gap_label_w[r])
            } else {
                primary_spacing + 2 * has_sg + usize::from(gap_side[r]) + usize::from(gap_in[r])
            }
        })
        .collect();

    let mut rank_primary_offsets: Vec<usize> = vec![0; max_rank + 1];
    let mut current_primary = 0;
    for r in 0..=max_rank {
        rank_primary_offsets[r] = current_primary;
        current_primary += rank_max_primary[r] + gap_size[r];
    }

    // When multiple named subgraphs share the same cluster rank (SCC cycle case),
    // stack them in separate vertical bands instead of centering all nodes together.
    let mut item_primary = vec![0usize; total_items];
    let mut item_secondary = vec![0usize; total_items];
    let final_secondary_extent;

    if max_secondary_band > 0 {
        // Per (rank, band) sequences, in rank order.
        let band_sequences: Vec<Vec<Vec<usize>>> = rank_groups
            .iter()
            .map(|group| {
                let mut per_band: Vec<Vec<usize>> = vec![vec![]; max_secondary_band + 1];
                for &idx in group {
                    per_band[item_band[idx]].push(idx);
                }
                per_band
            })
            .collect();

        // Compute max secondary extent per band across all ranks
        let mut band_max_extents: Vec<usize> = vec![0; max_secondary_band + 1];
        for per_band in &band_sequences {
            for (sb, seq) in per_band.iter().enumerate() {
                let (_, extent) =
                    place_sequence(seq, &item_secondary_size, &is_virtual, secondary_spacing);
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

        for (r, per_band) in band_sequences.iter().enumerate() {
            for (sb, seq) in per_band.iter().enumerate() {
                let (offsets, extent) =
                    place_sequence(seq, &item_secondary_size, &is_virtual, secondary_spacing);
                // Centre each rank's run within its band.
                let centre = (band_max_extents[sb] - extent) / 2;
                for (&idx, &off) in seq.iter().zip(&offsets) {
                    item_primary[idx] = rank_primary_offsets[r];
                    item_secondary[idx] = band_offsets[sb] + sg_top_margin + centre + off;
                }
            }
        }
    } else {
        // Original centering: no co-ranked subgraphs
        let placed: Vec<(Vec<usize>, usize)> = rank_groups
            .iter()
            .map(|group| {
                place_sequence(group, &item_secondary_size, &is_virtual, secondary_spacing)
            })
            .collect();
        let mut max_secondary_extent = placed.iter().map(|(_, e)| *e).max().unwrap_or(0);
        max_secondary_extent += sg_top_margin;
        final_secondary_extent = max_secondary_extent;

        for (r, group) in rank_groups.iter().enumerate() {
            let (offsets, rank_extent) = &placed[r];
            let secondary_offset = (max_secondary_extent - rank_extent) / 2;
            for (&idx, &off) in group.iter().zip(offsets) {
                item_primary[idx] = rank_primary_offsets[r];
                item_secondary[idx] = secondary_offset + sg_top_margin + off;
            }
        }
    }

    // Map primary/secondary → x/y based on direction.
    // For BT: reverse primary axis so highest rank is at top.
    // For RL: reverse primary axis so highest rank is at left.
    let total_primary = rank_primary_offsets[max_rank] + rank_max_primary[max_rank];

    // Screen-space primary coordinate of each item's start cell.
    let screen_primary: Vec<usize> = (0..total_items)
        .map(|i| {
            if reversed {
                total_primary.saturating_sub(item_primary[i] + item_primary_size[i])
            } else {
                item_primary[i]
            }
        })
        .collect();

    let (item_x, item_y): (Vec<usize>, Vec<usize>) = if is_lr {
        (screen_primary.clone(), item_secondary.clone())
    } else {
        (item_secondary.clone(), screen_primary.clone())
    };

    // Build positioned nodes
    let positioned_nodes: Vec<PositionedNode> = chart
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| PositionedNode {
            id: node.id.clone(),
            label: node.label.clone(),
            shape: node.shape.clone(),
            x: item_x[i],
            y: item_y[i],
            width: dims[i].0,
            height: dims[i].1,
            compact: is_lr && node.shape == NodeShape::Diamond,
            node_style: node.node_style.clone(),
            entity: node.entity.clone(),
        })
        .collect();

    // Compute bounding boxes for subgraphs. Coordinates may go negative here
    // (a box around rank-0 nodes needs rows above them); everything is shifted
    // into place after edge routing.
    const H_PAD: isize = 3;
    const V_PAD: isize = 2;

    // Build a lookup from node id to PositionedNode for fast access
    let node_pos_map: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    // (label, x, y, width, height) with signed origin.
    let raw_boxes: Vec<(String, isize, isize, usize, usize)> = chart
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

            let min_x = members.iter().map(|n| n.x).min().unwrap() as isize;
            let min_y = members.iter().map(|n| n.y).min().unwrap() as isize;
            let max_x_right = members.iter().map(|n| n.x + n.width).max().unwrap() as isize;
            let max_y_bottom = members.iter().map(|n| n.y + n.height).max().unwrap() as isize;

            let box_x = min_x - H_PAD;
            let box_y = min_y - V_PAD;
            // Minimum width so the label always fits in the top border: "┌ label ┐"
            let min_w = display_width(&sg.label) + 4;
            let box_w = ((max_x_right + H_PAD - box_x) as usize).max(min_w);
            let box_h = (max_y_bottom + V_PAD - box_y) as usize;

            Some((sg.label.clone(), box_x, box_y, box_w, box_h))
        })
        .collect();

    // Phase 4: Edge routing, in (secondary, primary) screen space.
    //
    // Screen-space extent of each rank along the primary axis.
    let rank_lo: Vec<usize> = (0..=max_rank)
        .map(|r| {
            if reversed {
                total_primary - rank_primary_offsets[r] - rank_max_primary[r]
            } else {
                rank_primary_offsets[r]
            }
        })
        .collect();
    let rank_hi: Vec<usize> = (0..=max_rank)
        .map(|r| rank_lo[r] + rank_max_primary[r])
        .collect();

    // Cell `k` (counted from the source side) of the gap after rank `r`, in
    // screen coordinates. Past the last rank the gap is open-ended.
    let gap_cell_after = |r: usize, k: usize| -> isize {
        if r < gap_count {
            let lo = rank_hi[r].min(rank_hi[r + 1]) as isize;
            if reversed {
                lo + gap_size[r] as isize - 1 - k as isize
            } else {
                lo + k as isize
            }
        } else if reversed {
            rank_lo[r] as isize - 1 - k as isize
        } else {
            (rank_hi[r] + k) as isize
        }
    };
    // Cell `k` counted from the target side of the gap before rank `r`.
    let gap_cell_before = |r: usize, k: usize| -> isize {
        if r > 0 {
            gap_cell_after(r - 1, gap_size[r - 1] - 1 - k)
        } else if reversed {
            (rank_hi[0] + k) as isize
        } else {
            rank_lo[0] as isize - 1 - k as isize
        }
    };
    // Lane where forward edges bend, in the gap after rank r.
    let forward_lane = |r: usize| -> isize {
        let k = if is_lr {
            gap_size[r] / 2
        } else {
            2 + has_sg + usize::from(gap_side[r])
        };
        gap_cell_after(r, k)
    };
    // Row where TD/BT labels of forward edges leaving rank r are written.
    let label_lane =
        |r: usize| -> isize { gap_cell_after(r, 1 + has_sg + usize::from(gap_side[r])) };
    // Lane used by back edges leaving rank r, same-rank edges and self-loops.
    let out_lane = |r: usize| -> isize { gap_cell_after(r, if is_lr { 1 } else { 1 + has_sg }) };
    // Lane used by back edges arriving at rank r, just before the arrowheads
    // (and the box border, in TD/BT).
    let in_lane = |r: usize| -> isize { gap_cell_before(r, if is_lr { 1 } else { 1 + has_sg }) };
    let sec_center =
        |i: usize| -> isize { (item_secondary[i] + item_secondary_size[i] / 2) as isize };
    let sec_hi = |i: usize| -> isize { (item_secondary[i] + item_secondary_size[i]) as isize };
    // First cell outside the node on the out side (bottom in TD).
    let out_port = |i: usize| -> isize {
        if reversed {
            screen_primary[i] as isize - 1
        } else {
            (screen_primary[i] + item_primary_size[i]) as isize
        }
    };
    // Border cell on the in side (top in TD).
    let in_port = |i: usize| -> isize {
        if reversed {
            (screen_primary[i] + item_primary_size[i] - 1) as isize
        } else {
            screen_primary[i] as isize
        }
    };
    // Border cell on the out side (bottom border in TD).
    let out_border = |i: usize| -> isize {
        if reversed {
            screen_primary[i] as isize
        } else {
            (screen_primary[i] + item_primary_size[i] - 1) as isize
        }
    };
    let to_xy = |(s, p): (isize, isize)| -> (isize, isize) { if is_lr { (p, s) } else { (s, p) } };

    // Obstacles for back edges: (secondary range, primary range) of every item
    // and subgraph box.
    let mut obstacles: Vec<((isize, isize), (isize, isize))> = (0..total_items)
        .map(|i| {
            let s0 = item_secondary[i] as isize;
            let p0 = screen_primary[i] as isize;
            (
                (s0, s0 + item_secondary_size[i] as isize),
                (p0, p0 + item_primary_size[i] as isize),
            )
        })
        .collect();
    for &(_, bx, by, bw, bh) in &raw_boxes {
        let (bw, bh) = (bw as isize, bh as isize);
        let (s0, s1, p0, p1) = if is_lr {
            (by, by + bh, bx, bx + bw)
        } else {
            (bx, bx + bw, by, by + bh)
        };
        obstacles.push(((s0, s1), (p0, p1)));
    }

    let label_width = |ei: usize| -> isize {
        chart.edges[ei]
            .label
            .as_deref()
            .filter(|l| !l.is_empty())
            .map(display_width)
            .unwrap_or(0) as isize
    };

    // Routed paths and label anchors, both in (secondary, primary) space.
    let mut routed: Vec<Option<RoutedEdge>> = vec![None; routes.len()];

    // Self-loops hug the side of their node, so they go first and become
    // obstacles for back edges, which run further out.
    for (ei, route) in routes.iter().enumerate() {
        let Some((fi, _, EdgeRoute::SelfLoop)) = route else {
            continue;
        };
        let fi = *fi;
        let g = out_lane(ranks[fi]);
        let side = sec_hi(fi) + 2;
        let mid = (screen_primary[fi] + item_primary_size[fi] / 2) as isize;
        let lw = label_width(ei);
        // Reserve the loop and its label (written just outside the loop).
        let reserve_hi = side + 1 + if lw > 0 { lw + 1 } else { 0 };
        obstacles.push(((side, reserve_hi), (g.min(mid), g.max(mid) + 1)));
        let label = (lw > 0).then_some((side + 2, mid));
        routed[ei] = Some((
            vec![
                (sec_center(fi), out_port(fi)),
                (sec_center(fi), g),
                (side, g),
                (side, mid),
                (sec_hi(fi) - 1, mid),
            ],
            label,
        ));
    }

    // Running secondary offset for back-edge side columns; each back edge
    // reserves its column plus room for its label.
    let mut side_cursor: isize = 0;
    for (ei, route) in routes.iter().enumerate() {
        let Some((fi, ti, route)) = route else {
            continue;
        };
        let (fi, ti) = (*fi, *ti);
        let rf = ranks[fi];
        let rt = ranks[ti];
        let lw = label_width(ei);
        let mut pts: Vec<(isize, isize)> = Vec::new();
        let mut label: Option<(isize, isize)> = None;
        match route {
            EdgeRoute::Forward { virtuals } => {
                let mut cols: Vec<isize> = Vec::with_capacity(virtuals.len() + 2);
                cols.push(sec_center(fi));
                cols.extend(virtuals.iter().map(|&v| sec_center(v)));
                cols.push(sec_center(ti));
                // Snap near-aligned hops to straight lines, pulling the
                // source side onto the target's column so every edge into a
                // node arrives at the same cell. Only in TD/BT: a one-column
                // offset still lands on the wide top border, while in LR/RL a
                // one-row offset would put the arrowhead on a box corner
                // instead of the middle row.
                let snap: isize = if is_lr { 0 } else { 1 };
                for k in (1..cols.len()).rev() {
                    if (cols[k] - cols[k - 1]).abs() <= snap {
                        cols[k - 1] = cols[k];
                    }
                }
                pts.push((cols[0], out_port(fi)));
                for (k, r) in (rf..rt).enumerate() {
                    let g = forward_lane(r);
                    pts.push((cols[k], g));
                    pts.push((cols[k + 1], g));
                }
                pts.push((cols[cols.len() - 1], in_port(ti)));
                if lw > 0 {
                    let straight = cols[0] == cols[1];
                    if is_lr {
                        // Written across the first gap, centred; on a straight
                        // edge that is on the wire, on a jogged edge it sits at
                        // the jog's midpoint row.
                        let gap_lo = rank_hi[rf].min(rank_hi[rf + 1]) as isize;
                        let sec = if straight {
                            cols[0]
                        } else {
                            (cols[0] + cols[1]) / 2
                        };
                        label = Some((sec, gap_lo + (gap_size[rf] as isize - lw) / 2));
                    } else if straight {
                        // Beside the vertical line, on the label row.
                        label = Some((cols[0] + 1, label_lane(rf)));
                    } else {
                        // Centred over the first horizontal segment when it
                        // fits, else flush with its left end.
                        let (a, b) = (cols[0].min(cols[1]), cols[0].max(cols[1]));
                        let seg = b - a;
                        let sec = if seg >= lw { a + (seg - lw) / 2 } else { a };
                        label = Some((sec, label_lane(rf)));
                    }
                }
            }
            EdgeRoute::Back => {
                let g_out = out_lane(rf);
                let g_in = in_lane(rt);
                let (span_lo, span_hi) = (g_out.min(g_in), g_out.max(g_in));
                let side_base = obstacles
                    .iter()
                    .filter(|(_, (p0, p1))| *p0 <= span_hi && span_lo < *p1)
                    .map(|((_, s1), _)| *s1)
                    .max()
                    .unwrap_or(0);
                let side = side_base + 2 + side_cursor;
                // TD/BT labels sit beside the side column and need width;
                // LR/RL labels sit above the side row.
                side_cursor += 2 + if is_lr {
                    0
                } else {
                    lw + if lw > 0 { 1 } else { 0 }
                };
                pts.push((sec_center(fi), out_port(fi)));
                pts.push((sec_center(fi), g_out));
                pts.push((side, g_out));
                pts.push((side, g_in));
                pts.push((sec_center(ti), g_in));
                pts.push((sec_center(ti), in_port(ti)));
                if lw > 0 {
                    label = Some(if is_lr {
                        (side - 1, (span_lo + span_hi - lw) / 2)
                    } else {
                        (side + 2, (span_lo + span_hi) / 2)
                    });
                }
            }
            EdgeRoute::SameRank => {
                let g = out_lane(rf);
                pts.push((sec_center(fi), out_port(fi)));
                pts.push((sec_center(fi), g));
                pts.push((sec_center(ti), g));
                pts.push((sec_center(ti), out_border(ti)));
                if lw > 0 {
                    let (a, b) = (
                        sec_center(fi).min(sec_center(ti)),
                        sec_center(fi).max(sec_center(ti)),
                    );
                    // TD/BT: on the wire, centred between the two ports.
                    // LR/RL: beside the vertical run, at its midpoint.
                    label = Some(if is_lr {
                        ((a + b) / 2, g + 1)
                    } else {
                        ((a + b - lw) / 2, g)
                    });
                }
            }
            EdgeRoute::SelfLoop => continue,
        }
        routed[ei] = Some((pts, label));
    }
    let raw_edges: Vec<(usize, Vec<Pt>, Option<Pt>)> = routed
        .into_iter()
        .enumerate()
        .filter_map(|(ei, r)| {
            let (pts, label) = r?;
            let xy: Vec<(isize, isize)> = pts.into_iter().map(to_xy).collect();
            Some((ei, simplify_path(xy), label.map(to_xy)))
        })
        .collect();

    // Side routes may reach above/left of the origin; shift everything so all
    // coordinates are non-negative.
    let min_x = raw_edges
        .iter()
        .flat_map(|(_, p, l)| p.iter().chain(l.iter()).map(|q| q.0))
        .chain(raw_boxes.iter().map(|b| b.1))
        .min()
        .unwrap_or(0)
        .min(0);
    let min_y = raw_edges
        .iter()
        .flat_map(|(_, p, l)| p.iter().chain(l.iter()).map(|q| q.1))
        .chain(raw_boxes.iter().map(|b| b.2))
        .min()
        .unwrap_or(0)
        .min(0);
    let shift_x = (-min_x) as usize;
    let shift_y = (-min_y) as usize;

    let mut positioned_nodes = positioned_nodes;
    if shift_x > 0 || shift_y > 0 {
        for node in &mut positioned_nodes {
            node.x += shift_x;
            node.y += shift_y;
        }
    }
    let subgraph_boxes: Vec<SubgraphBox> = raw_boxes
        .into_iter()
        .map(|(label, x, y, width, height)| SubgraphBox {
            label,
            x: (x + shift_x as isize) as usize,
            y: (y + shift_y as isize) as usize,
            width,
            height,
            border_color: None,
        })
        .collect();

    let shift = |(x, y): (isize, isize)| -> (usize, usize) {
        (
            (x + shift_x as isize) as usize,
            (y + shift_y as isize) as usize,
        )
    };
    let positioned_edges: Vec<PositionedEdge> = raw_edges
        .into_iter()
        .map(|(ei, points, label_pos)| {
            let edge = &chart.edges[ei];
            PositionedEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                label: edge.label.clone(),
                style: edge.style.clone(),
                points: points.into_iter().map(shift).collect(),
                label_pos: label_pos.map(shift),
                edge_style: edge.edge_style.clone(),
                er_meta: edge.er_meta.clone(),
            }
        })
        .collect();

    let (mut total_width, mut total_height) = if is_lr {
        (total_primary + shift_x, final_secondary_extent + shift_y)
    } else {
        (final_secondary_extent + shift_x, total_primary + shift_y)
    };
    for b in &subgraph_boxes {
        total_width = total_width.max(b.x + b.width);
        total_height = total_height.max(b.y + b.height);
    }
    for e in &positioned_edges {
        for &(x, y) in &e.points {
            total_width = total_width.max(x + 1);
            total_height = total_height.max(y + 1);
        }
        if let (Some((x, y)), Some(label)) = (e.label_pos, e.label.as_deref()) {
            total_width = total_width.max(x + display_width(label));
            total_height = total_height.max(y + 1);
        }
    }

    LayoutResult {
        nodes: positioned_nodes,
        edges: positioned_edges,
        subgraph_boxes,
        width: total_width,
        height: total_height,
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
            entity: None,
        }
    }

    fn make_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            style: EdgeStyle::Arrow,
            edge_style: None,
            er_meta: None,
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
            entity: None,
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
                entity: None,
            },
            Node {
                id: "Dispatch".into(),
                label: "Dispatch".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "Deliver".into(),
                label: "Deliver".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "DQ".into(),
                label: "DQ".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "CQ".into(),
                label: "CQ".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "DLQ".into(),
                label: "DLQ".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "Send".into(),
                to: "Dispatch".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "Dispatch".into(),
                to: "Deliver".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "DQ".into(),
                to: "CQ".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "CQ".into(),
                to: "DLQ".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "Send".into(),
                to: "DQ".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
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
                entity: None,
            },
            Node {
                id: "A2".into(),
                label: "A2".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "B1".into(),
                label: "B1".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "B2".into(),
                label: "B2".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
        ];
        let edges = vec![Edge {
            from: "A1".into(),
            to: "B1".into(),
            label: None,
            style: EdgeStyle::Arrow,
            edge_style: None,
            er_meta: None,
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
                entity: None,
            },
            Node {
                id: "D".into(),
                label: "Dispatch".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "Q".into(),
                label: "Queue".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "C".into(),
                label: "Consume".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "S".into(),
                to: "D".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "S".into(),
                to: "Q".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "Q".into(),
                to: "C".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
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
                entity: None,
            },
            Node {
                id: "A".into(),
                label: "A".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "B".into(),
                label: "B".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "C".into(),
                label: "C".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "D".into(),
                label: "D".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "A".into(),
                to: "B".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "C".into(),
                to: "D".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "A".into(),
                to: "C".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
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

    /// Property-style sweep over pseudo-random DAGs (deterministic xorshift,
    /// no extra dependencies) in all four directions. Invariants that must
    /// hold for any input: no two node boxes overlap, every node lies within
    /// the reported bounds, and the layout is deterministic.
    #[test]
    fn random_dags_have_no_overlapping_nodes_and_fit_bounds() {
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for case in 0..40 {
            let n = 3 + (next() % 18) as usize;
            let nodes: Vec<Node> = (0..n)
                .map(|i| make_node(&format!("N{i}"), &format!("Node {i}")))
                .collect();
            let m = n + (next() % (2 * n as u64)) as usize;
            let mut edges = Vec::new();
            for _ in 0..m {
                let a = (next() % n as u64) as usize;
                let b = (next() % n as u64) as usize;
                if a == b {
                    continue;
                }
                // Forward edges only → acyclic by construction.
                let (a, b) = if a < b { (a, b) } else { (b, a) };
                edges.push(make_edge(&format!("N{a}"), &format!("N{b}")));
            }
            for dir in [
                Direction::TopDown,
                Direction::BottomTop,
                Direction::LeftRight,
                Direction::RightLeft,
            ] {
                let chart = FlowChart {
                    direction: dir.clone(),
                    nodes: nodes.clone(),
                    edges: edges.clone(),
                    subgraphs: vec![],
                };
                let r1 = layout(&chart);
                let r2 = layout(&chart);
                for (a, b) in r1.nodes.iter().zip(&r2.nodes) {
                    assert_eq!(
                        (a.x, a.y, a.width, a.height),
                        (b.x, b.y, b.width, b.height),
                        "case {case} {dir:?}: layout is not deterministic for {}",
                        a.id
                    );
                }
                for node in &r1.nodes {
                    assert!(
                        node.x + node.width <= r1.width && node.y + node.height <= r1.height,
                        "case {case} {dir:?}: {} at ({},{}) {}x{} exceeds bounds {}x{}",
                        node.id,
                        node.x,
                        node.y,
                        node.width,
                        node.height,
                        r1.width,
                        r1.height
                    );
                }
                for (i, a) in r1.nodes.iter().enumerate() {
                    for b in &r1.nodes[i + 1..] {
                        let overlap = a.x < b.x + b.width
                            && b.x < a.x + a.width
                            && a.y < b.y + b.height
                            && b.y < a.y + a.height;
                        assert!(
                            !overlap,
                            "case {case} {dir:?}: {} ({},{}) and {} ({},{}) overlap",
                            a.id, a.x, a.y, b.id, b.x, b.y
                        );
                    }
                }
            }
        }
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
                entity: None,
            },
            Node {
                id: "B".into(),
                label: "B".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
            Node {
                id: "C".into(),
                label: "C".into(),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            },
        ];
        let edges = vec![
            Edge {
                from: "A".into(),
                to: "B".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
            },
            Edge {
                from: "B".into(),
                to: "C".into(),
                label: None,
                style: EdgeStyle::Arrow,
                edge_style: None,
                er_meta: None,
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

    // -----------------------------------------------------------------------
    // Edge-routing invariants
    // -----------------------------------------------------------------------

    /// Every cell of every edge segment must lie outside the bounding box of
    /// any node that is not the edge's source or target (issue #2).
    fn assert_edges_avoid_foreign_nodes(result: &LayoutResult, context: &str) {
        for edge in &result.edges {
            for seg in edge.points.windows(2) {
                let ((x0, y0), (x1, y1)) = (seg[0], seg[1]);
                let (xa, xb) = (x0.min(x1), x0.max(x1));
                let (ya, yb) = (y0.min(y1), y0.max(y1));
                for node in &result.nodes {
                    if node.id == edge.from || node.id == edge.to {
                        continue;
                    }
                    let hit = xa < node.x + node.width
                        && node.x <= xb
                        && ya < node.y + node.height
                        && node.y <= yb;
                    assert!(
                        !hit,
                        "{context}: edge {}->{} segment {:?}-{:?} crosses node {} at ({},{}) {}x{}",
                        edge.from,
                        edge.to,
                        seg[0],
                        seg[1],
                        node.id,
                        node.x,
                        node.y,
                        node.width,
                        node.height
                    );
                }
            }
        }
    }

    fn all_directions() -> [Direction; 4] {
        [
            Direction::TopDown,
            Direction::BottomTop,
            Direction::LeftRight,
            Direction::RightLeft,
        ]
    }

    /// Issue #1: `A-->B-->C-->A` must unroll into a chain; the closing edge is
    /// drawn as a back edge that ends on A's border.
    #[test]
    fn free_node_cycle_unrolls_into_chain_with_back_edge() {
        let chart = simple_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
            ],
            vec![
                make_edge("A", "B"),
                make_edge("B", "C"),
                make_edge("C", "A"),
            ],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let (a, b, c) = (find("A"), find("B"), find("C"));
        assert!(a.y < b.y && b.y < c.y, "expected A above B above C");

        let back = result
            .edges
            .iter()
            .find(|e| e.from == "C" && e.to == "A")
            .expect("back edge present");
        assert!(
            back.points.len() >= 4,
            "back edge should route around the side, got {:?}",
            back.points
        );
        let last = *back.points.last().unwrap();
        assert_eq!(
            last,
            (a.x + a.width / 2, a.y),
            "back edge must end on A's top border"
        );
        let prev = back.points[back.points.len() - 2];
        assert!(
            prev.0 == last.0 && prev.1 + 2 <= last.1 || prev.1 >= last.1 + 2,
            "last segment must be long enough for an arrowhead: {prev:?} -> {last:?}"
        );
        // The side column lies to the right of every node.
        let side_x = back.points[2].0;
        for n in &result.nodes {
            assert!(
                side_x >= n.x + n.width,
                "side column {side_x} overlaps {}",
                n.id
            );
        }
        assert_edges_avoid_foreign_nodes(&result, "cycle");
    }

    /// A feedback loop inside a longer chain keeps the forward chain intact.
    #[test]
    fn retry_loop_does_not_flatten_chain() {
        let chart = simple_chart(
            vec![
                make_node("A", "A"),
                make_node("B", "B"),
                make_node("C", "C"),
                make_node("D", "D"),
            ],
            vec![
                make_edge("A", "B"),
                make_edge("B", "C"),
                make_edge("C", "B"),
                make_edge("C", "D"),
            ],
        );
        let result = layout(&chart);
        let y = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().y;
        assert!(y("A") < y("B") && y("B") < y("C") && y("C") < y("D"));
    }

    /// Cycles through named subgraphs keep the SCC grouping (the subgraphs
    /// stay side by side on one cluster rank).
    #[test]
    fn bidirectional_subgraph_edges_keep_subgraphs_side_by_side() {
        use crate::mermaid::Subgraph;
        let chart = FlowChart {
            direction: Direction::LeftRight,
            nodes: vec![make_node("A", "A"), make_node("B", "B")],
            edges: vec![make_edge("A", "B"), make_edge("B", "A")],
            subgraphs: vec![
                Subgraph {
                    id: "SA".into(),
                    label: "SA".into(),
                    node_ids: vec!["A".into()],
                },
                Subgraph {
                    id: "SB".into(),
                    label: "SB".into(),
                    node_ids: vec!["B".into()],
                },
            ],
        };
        let result = layout(&chart);
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        assert_eq!(a.x, b.x, "co-ranked subgraphs share the rank column");
        assert_ne!(a.y, b.y, "and are stacked in separate bands");
        assert_edges_avoid_foreign_nodes(&result, "subgraph cycle");
    }

    /// A self-loop is drawn beside its node and ends on the node's border.
    #[test]
    fn self_loop_is_routed_beside_node() {
        let chart = simple_chart(
            vec![make_node("A", "A"), make_node("B", "B")],
            vec![make_edge("A", "A"), make_edge("A", "B")],
        );
        let result = layout(&chart);
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let loop_edge = result
            .edges
            .iter()
            .find(|e| e.from == "A" && e.to == "A")
            .unwrap();
        assert!(loop_edge.points.len() >= 4, "{:?}", loop_edge.points);
        let last = *loop_edge.points.last().unwrap();
        assert_eq!(last, (a.x + a.width - 1, a.y + a.height / 2));
        assert!(
            loop_edge
                .points
                .iter()
                .all(|&(x, _)| x >= a.x + a.width / 2),
            "loop stays on the right-hand side: {:?}",
            loop_edge.points
        );
    }

    /// Issue #2: an edge that skips ranks is split at every intermediate rank
    /// and never crosses a node there.
    #[test]
    fn rank_skipping_edges_avoid_intermediate_nodes() {
        let chart = simple_chart(
            vec![
                make_node("A", "Node A"),
                make_node("B", "Node B"),
                make_node("C", "Node C"),
                make_node("D", "Node D"),
            ],
            vec![
                make_edge("A", "B"),
                make_edge("B", "C"),
                make_edge("C", "D"),
                make_edge("A", "D"),
                make_edge("A", "C"),
            ],
        );
        for dir in all_directions() {
            let chart = FlowChart {
                direction: dir.clone(),
                ..chart.clone()
            };
            let result = layout(&chart);
            assert_edges_avoid_foreign_nodes(&result, &format!("{dir:?}"));
            let long = result
                .edges
                .iter()
                .find(|e| e.from == "A" && e.to == "D")
                .unwrap();
            assert!(
                long.points.len() >= 4,
                "{dir:?}: A->D should bend around B and C, got {:?}",
                long.points
            );
        }
    }

    /// Ports: the first point is the cell just outside the source, the last
    /// point is on the target's border, so the arrowhead (one cell before the
    /// end) always touches the target. Checked in all four directions.
    #[test]
    fn edge_ports_touch_source_and_target_in_all_directions() {
        for dir in all_directions() {
            let chart = FlowChart {
                direction: dir.clone(),
                nodes: vec![make_node("A", "A"), make_node("B", "B")],
                edges: vec![make_edge("A", "B")],
                subgraphs: vec![],
            };
            let result = layout(&chart);
            let a = &result.nodes[0];
            let b = &result.nodes[1];
            let e = &result.edges[0];
            let first = e.points[0];
            let last = *e.points.last().unwrap();
            let (exp_first, exp_last) = match dir {
                Direction::TopDown => (
                    (a.x + a.width / 2, a.y + a.height),
                    (b.x + b.width / 2, b.y),
                ),
                Direction::BottomTop => (
                    (a.x + a.width / 2, a.y - 1),
                    (b.x + b.width / 2, b.y + b.height - 1),
                ),
                Direction::LeftRight => (
                    (a.x + a.width, a.y + a.height / 2),
                    (b.x, b.y + b.height / 2),
                ),
                Direction::RightLeft => (
                    (a.x - 1, a.y + a.height / 2),
                    (b.x + b.width - 1, b.y + b.height / 2),
                ),
            };
            assert_eq!(first, exp_first, "{dir:?}: start port");
            assert_eq!(last, exp_last, "{dir:?}: end port");
        }
    }

    /// Issue #3: node boxes are sized by display width, not byte length.
    #[test]
    fn node_dimensions_use_display_width() {
        let chart = simple_chart(
            vec![
                make_node("A", "数据库"),
                make_node("B", "Café"),
                make_node("C", "Ünïcödé ✓"),
            ],
            vec![],
        );
        let result = layout(&chart);
        let w = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().width;
        assert_eq!(w("A"), 6 + 4, "three CJK characters are six columns wide");
        assert_eq!(w("B"), 4 + 4);
        assert_eq!(w("C"), 9 + 4);
    }

    /// Property sweep over random graphs that may contain cycles, duplicate
    /// edges and self-loops: no edge segment crosses a foreign node, every
    /// point lies within the reported bounds, and the layout is deterministic.
    #[test]
    fn random_graphs_with_cycles_route_edges_around_nodes() {
        let mut seed: u64 = 0xD1B5_4A32_D192_ED03;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for case in 0..60 {
            let n = 2 + (next() % 12) as usize;
            let nodes: Vec<Node> = (0..n)
                .map(|i| make_node(&format!("N{i}"), &format!("Node {i}")))
                .collect();
            let m = n + (next() % (2 * n as u64)) as usize;
            let mut edges = Vec::new();
            for _ in 0..m {
                let a = (next() % n as u64) as usize;
                let b = (next() % n as u64) as usize;
                // Any direction, self-loops included.
                let mut e = make_edge(&format!("N{a}"), &format!("N{b}"));
                if next() % 4 == 0 {
                    e.label = Some(format!("e{a}{b}"));
                }
                edges.push(e);
            }
            for dir in all_directions() {
                let chart = FlowChart {
                    direction: dir.clone(),
                    nodes: nodes.clone(),
                    edges: edges.clone(),
                    subgraphs: vec![],
                };
                let r1 = layout(&chart);
                let r2 = layout(&chart);
                for (a, b) in r1.edges.iter().zip(&r2.edges) {
                    assert_eq!(a.points, b.points, "case {case} {dir:?}: not deterministic");
                }
                assert_edges_avoid_foreign_nodes(&r1, &format!("case {case} {dir:?}"));
                for e in &r1.edges {
                    for &(x, y) in &e.points {
                        assert!(
                            x < r1.width && y < r1.height,
                            "case {case} {dir:?}: point ({x},{y}) outside {}x{}",
                            r1.width,
                            r1.height
                        );
                    }
                }
                for (i, a) in r1.nodes.iter().enumerate() {
                    for b in &r1.nodes[i + 1..] {
                        let overlap = a.x < b.x + b.width
                            && b.x < a.x + a.width
                            && a.y < b.y + b.height
                            && b.y < a.y + a.height;
                        assert!(
                            !overlap,
                            "case {case} {dir:?}: {} and {} overlap",
                            a.id, b.id
                        );
                    }
                }
            }
        }
    }
}
