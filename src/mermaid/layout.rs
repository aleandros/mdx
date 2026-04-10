use super::FlowChart;

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

pub fn layout(_chart: &FlowChart) -> LayoutResult {
    todo!()
}
