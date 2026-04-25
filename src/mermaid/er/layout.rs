use super::ErDiagram;
use crate::mermaid::FlowChart;

pub fn to_flowchart(_diagram: &mut ErDiagram, _max_box_width: usize) -> FlowChart {
    FlowChart {
        direction: crate::mermaid::Direction::TopDown,
        nodes: Vec::new(),
        edges: Vec::new(),
        subgraphs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {}
