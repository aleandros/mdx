pub mod ascii;
pub mod layout;
pub mod parse;
pub mod sequence;

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    TopDown,
    BottomTop,
    LeftRight,
    RightLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeShape {
    Rect,
    Rounded,
    Diamond,
    Circle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeStyle {
    Arrow,
    Line,
    Dotted,
    Thick,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowChart {
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Returns (ascii_lines, node_count, edge_count)
pub fn render_mermaid(content: &str) -> anyhow::Result<(Vec<String>, usize, usize)> {
    let first_line = content
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .unwrap_or("");

    if first_line == "sequenceDiagram" {
        let diagram = sequence::parse::parse_sequence(content)?;
        let participant_count = diagram.participants.len();
        let event_count = diagram.events.len();
        let laid_out = sequence::layout::layout(&diagram);
        let lines = sequence::ascii::render(&laid_out);
        Ok((lines, participant_count, event_count))
    } else {
        let chart = parse::parse_flowchart(content)?;
        let node_count = chart.nodes.len();
        let edge_count = chart.edges.len();
        let positioned = layout::layout(&chart);
        let lines = ascii::render(&positioned);
        Ok((lines, node_count, edge_count))
    }
}
