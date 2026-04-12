use super::{Direction, FlowChart, NodeShape};

const H_SPACING: usize = 4;
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
}

#[derive(Debug)]
pub struct LayoutResult {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
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
    } else {
        // LR/RL: horizontal first, then vertical
        let mid_x = (start.0 + end.0) / 2;
        vec![start, (mid_x, start.1), (mid_x, end.1), end]
    }
}

pub fn layout(chart: &FlowChart) -> LayoutResult {
    if chart.nodes.is_empty() {
        return LayoutResult {
            nodes: vec![],
            edges: vec![],
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

    // Phase 1: Rank assignment (Kahn's algorithm + longest path)
    let mut in_degree = vec![0usize; n];
    let mut successors: Vec<Vec<usize>> = vec![vec![]; n];
    let mut predecessors: Vec<Vec<usize>> = vec![vec![]; n];

    for edge in &chart.edges {
        if let (Some(&from_idx), Some(&to_idx)) =
            (node_index.get(edge.from.as_str()), node_index.get(edge.to.as_str()))
        {
            successors[from_idx].push(to_idx);
            predecessors[to_idx].push(from_idx);
            in_degree[to_idx] += 1;
        }
    }

    let mut ranks = vec![0usize; n];
    let mut queue = std::collections::VecDeque::new();

    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    // We need a remaining in-degree counter to process topologically
    let mut remaining_in = in_degree.clone();
    let mut processed = vec![false; n];

    // Process nodes, breaking cycles by forcing unprocessed nodes as sources
    loop {
        while let Some(idx) = queue.pop_front() {
            processed[idx] = true;
            for &succ in &successors[idx] {
                // Only update rank for unprocessed nodes (skip back-edges)
                if !processed[succ] && ranks[succ] < ranks[idx] + 1 {
                    ranks[succ] = ranks[idx] + 1;
                }
                remaining_in[succ] = remaining_in[succ].saturating_sub(1);
                if remaining_in[succ] == 0 && !processed[succ] {
                    queue.push_back(succ);
                }
            }
        }

        // If cycle nodes remain, force the first unprocessed node as source
        if let Some(i) = (0..n).find(|&i| !processed[i]) {
            remaining_in[i] = 0;
            queue.push_back(i);
        } else {
            break;
        }
    }

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

        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));

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

    let max_secondary_extent = *rank_secondary_extents.iter().max().unwrap_or(&0);

    // Primary offsets per rank
    let rank_max_primary: Vec<usize> = rank_groups
        .iter()
        .map(|group| group.iter().map(|&idx| node_primary_size[idx]).max().unwrap_or(0))
        .collect();

    let mut rank_primary_offsets: Vec<usize> = vec![0; max_rank + 1];
    let mut current_primary = 0;
    for r in 0..=max_rank {
        rank_primary_offsets[r] = current_primary;
        current_primary += rank_max_primary[r] + primary_spacing;
    }

    // Assign primary/secondary coordinate to each node
    let mut node_primary = vec![0usize; n];
    let mut node_secondary = vec![0usize; n];

    for (r, group) in rank_groups.iter().enumerate() {
        // Center this rank along the secondary axis
        let rank_extent = rank_secondary_extents[r];
        let secondary_offset = (max_secondary_extent - rank_extent) / 2;

        let mut cur_secondary = secondary_offset;
        for &idx in group {
            node_primary[idx] = rank_primary_offsets[r];
            node_secondary[idx] = cur_secondary;
            cur_secondary += node_secondary_size[idx] + secondary_spacing;
        }
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
        })
        .collect();

    // Build positioned edges
    let total_height = match chart.direction {
        Direction::TopDown => total_primary,
        Direction::BottomTop => total_primary,
        Direction::LeftRight | Direction::RightLeft => max_secondary_extent,
    };

    let total_width = match chart.direction {
        Direction::TopDown | Direction::BottomTop => max_secondary_extent,
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
                    let s = (from_node.x + from_node.width / 2, from_node.y + from_node.height);
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
                    let s = (from_node.x + from_node.width, from_node.y + from_node.height / 2);
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
            })
        })
        .collect();

    LayoutResult {
        nodes: positioned_nodes,
        edges: positioned_edges,
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
        }
    }

    fn make_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            style: EdgeStyle::Arrow,
        }
    }

    fn simple_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::TopDown,
            nodes,
            edges,
        }
    }

    /// A->B->C: each node should be at a greater y than the previous, all same x
    #[test]
    fn test_linear_chain_ranks() {
        let chart = simple_chart(
            vec![make_node("A", "A"), make_node("B", "B"), make_node("C", "C")],
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
            vec![make_node("A", "A"), make_node("B", "B"), make_node("C", "C")],
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
        }
    }

    fn bt_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::BottomTop,
            nodes,
            edges,
        }
    }

    fn rl_chart(nodes: Vec<Node>, edges: Vec<Edge>) -> FlowChart {
        FlowChart {
            direction: Direction::RightLeft,
            nodes,
            edges,
        }
    }

    /// LR: A->B->C chain — x strictly increases along the chain, y stays same
    #[test]
    fn test_lr_linear_chain() {
        let chart = lr_chart(
            vec![make_node("A", "A"), make_node("B", "B"), make_node("C", "C")],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // x strictly increases (ranks go left→right)
        assert!(a.x < b.x, "LR: A.x ({}) should be less than B.x ({})", a.x, b.x);
        assert!(b.x < c.x, "LR: B.x ({}) should be less than C.x ({})", b.x, c.x);

        // All at same y (single row)
        assert_eq!(a.y, b.y, "LR: A and B should have same y");
        assert_eq!(b.y, c.y, "LR: B and C should have same y");
    }

    /// LR branching: A->B, A->C — B and C at same x (same rank), different y
    #[test]
    fn test_lr_branching() {
        let chart = lr_chart(
            vec![make_node("A", "A"), make_node("B", "B"), make_node("C", "C")],
            vec![make_edge("A", "B"), make_edge("A", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // A rank 0, B/C rank 1 → B and C at same x
        assert!(a.x < b.x, "LR: A.x ({}) should be less than B.x ({})", a.x, b.x);
        assert_eq!(b.x, c.x, "LR: B.x ({}) should equal C.x ({})", b.x, c.x);

        // B and C at different y
        assert_ne!(b.y, c.y, "LR: B.y ({}) should differ from C.y ({})", b.y, c.y);
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
            vec![make_node("A", "A"), make_node("B", "B"), make_node("C", "C")],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // y strictly decreases: A at bottom (larger y), C at top (smaller y)
        assert!(a.y > b.y, "BT: A.y ({}) should be greater than B.y ({})", a.y, b.y);
        assert!(b.y > c.y, "BT: B.y ({}) should be greater than C.y ({})", b.y, c.y);

        // All at same x
        assert_eq!(a.x, b.x, "BT: A and B should have same x");
        assert_eq!(b.x, c.x, "BT: B and C should have same x");
    }

    /// RL: A->B->C chain — x strictly decreases along the chain (A at right, C at left)
    #[test]
    fn test_rl_linear_chain() {
        let chart = rl_chart(
            vec![make_node("A", "A"), make_node("B", "B"), make_node("C", "C")],
            vec![make_edge("A", "B"), make_edge("B", "C")],
        );
        let result = layout(&chart);
        let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
        let a = find("A");
        let b = find("B");
        let c = find("C");

        // x strictly decreases: A at right, C at left
        assert!(a.x > b.x, "RL: A.x ({}) should be greater than B.x ({})", a.x, b.x);
        assert!(b.x > c.x, "RL: B.x ({}) should be greater than C.x ({})", b.x, c.x);

        // All at same y
        assert_eq!(a.y, b.y, "RL: A and B should have same y");
        assert_eq!(b.y, c.y, "RL: B and C should have same y");
    }
}
