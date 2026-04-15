use super::layout::SequenceLayout;
use crate::mermaid::ascii::Canvas;

pub fn render(layout: &SequenceLayout) -> Vec<String> {
    if layout.width == 0 || layout.height == 0 {
        return vec![];
    }

    let canvas = Canvas::new(layout.width, layout.height);
    let mut lines = canvas.to_lines();
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}
