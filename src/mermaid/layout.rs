use super::{FlowChart, NodeShape};

const H_SPACING: usize = 4;
const V_SPACING: usize = 2;

#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub id: String,
    pub label: String,
    pub shape: super::NodeShape,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedEdge {
    pub from: String,
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

fn node_dimensions(label: &str, shape: &NodeShape) -> (usize, usize) {
    match shape {
        NodeShape::Rect | NodeShape::Rounded | NodeShape::Circle => (label.len() + 4, 3),
        NodeShape::Diamond => {
            let inner_w = label.len() + 2;
            let half = (inner_w + 1) / 2;
            (inner_w + 2, half * 2 + 1)
        }
    }
}

fn route_edge(start: (usize, usize), end: (usize, usize)) -> Vec<(usize, usize)> {
    if start.0 == end.0 {
        vec![start, end]
    } else {
        let mid_y = (start.1 + end.1) / 2;
        vec![start, (start.0, mid_y), (end.0, mid_y), end]
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

    for i in 0..n {
        if in_degree[i] == 0 {
            queue.push_back(i);
        }
    }

    // We need a remaining in-degree counter to process topologically
    let mut remaining_in = in_degree.clone();

    while let Some(idx) = queue.pop_front() {
        for &succ in &successors[idx] {
            if ranks[succ] < ranks[idx] + 1 {
                ranks[succ] = ranks[idx] + 1;
            }
            remaining_in[succ] -= 1;
            if remaining_in[succ] == 0 {
                queue.push_back(succ);
            }
        }
    }

    // Nodes not reached (cycles) already default to rank 0

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

    // Phase 3: Coordinate assignment
    // For each rank compute node dimensions and max height
    let dims: Vec<(usize, usize)> = chart
        .nodes
        .iter()
        .map(|n| node_dimensions(&n.label, &n.shape))
        .collect();

    // Compute the pixel width of each rank (sum of node widths + H_SPACING between them)
    let rank_widths: Vec<usize> = rank_groups
        .iter()
        .map(|group| {
            if group.is_empty() {
                return 0;
            }
            let total_node_width: usize = group.iter().map(|&idx| dims[idx].0).sum();
            let gaps = if group.len() > 1 {
                (group.len() - 1) * H_SPACING
            } else {
                0
            };
            total_node_width + gaps
        })
        .collect();

    let max_rank_width = *rank_widths.iter().max().unwrap_or(&0);

    // Compute y offsets per rank
    let mut rank_y_offsets: Vec<usize> = vec![0; max_rank + 1];
    let rank_max_heights: Vec<usize> = rank_groups
        .iter()
        .map(|group| group.iter().map(|&idx| dims[idx].1).max().unwrap_or(0))
        .collect();

    let mut current_y = 0;
    for r in 0..=max_rank {
        rank_y_offsets[r] = current_y;
        current_y += rank_max_heights[r] + V_SPACING;
    }

    // Assign x, y to each node
    let mut node_x = vec![0usize; n];
    let mut node_y = vec![0usize; n];

    for (r, group) in rank_groups.iter().enumerate() {
        // Center this rank horizontally relative to the widest rank
        let rank_w = rank_widths[r];
        let x_offset = (max_rank_width - rank_w) / 2;

        let mut cur_x = x_offset;
        for &idx in group {
            node_x[idx] = cur_x;
            node_y[idx] = rank_y_offsets[r];
            cur_x += dims[idx].0 + H_SPACING;
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
            width: dims[i].0,
            height: dims[i].1,
        })
        .collect();

    // Build positioned edges
    let total_height = if max_rank + 1 > 0 {
        rank_y_offsets[max_rank] + rank_max_heights[max_rank]
    } else {
        0
    };

    let positioned_edges: Vec<PositionedEdge> = chart
        .edges
        .iter()
        .filter_map(|edge| {
            let from_idx = *node_index.get(edge.from.as_str())?;
            let to_idx = *node_index.get(edge.to.as_str())?;
            let from_node = &positioned_nodes[from_idx];
            let to_node = &positioned_nodes[to_idx];

            // center-bottom of source
            let start = (from_node.x + from_node.width / 2, from_node.y + from_node.height);
            // center-top of target
            let end = (to_node.x + to_node.width / 2, to_node.y);

            let points = route_edge(start, end);

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
        width: max_rank_width,
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
}
