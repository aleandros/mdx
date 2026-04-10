use super::layout::{LayoutResult, PositionedEdge, PositionedNode};
use super::{EdgeStyle, NodeShape};

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

pub struct Canvas {
    grid: Vec<Vec<char>>,
    width: usize,
    height: usize,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Canvas {
            grid: vec![vec![' '; width]; height],
            width,
            height,
        }
    }

    pub fn set(&mut self, x: usize, y: usize, ch: char) {
        if x < self.width && y < self.height {
            self.grid[y][x] = ch;
        }
    }

    pub fn draw_text(&mut self, x: usize, y: usize, text: &str) {
        for (i, ch) in text.chars().enumerate() {
            self.set(x + i, y, ch);
        }
    }

    pub fn to_lines(&self) -> Vec<String> {
        self.grid
            .iter()
            .map(|row| {
                let s: String = row.iter().collect();
                s.trim_end().to_string()
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Node drawing
// ---------------------------------------------------------------------------

fn draw_rect(canvas: &mut Canvas, node: &PositionedNode) {
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;

    // Top row
    canvas.set(x, y, '┌');
    for i in 1..w - 1 {
        canvas.set(x + i, y, '─');
    }
    canvas.set(x + w - 1, y, '┐');

    // Middle rows
    for row in 1..h - 1 {
        canvas.set(x, y + row, '│');
        canvas.set(x + w - 1, y + row, '│');
    }

    // Bottom row
    canvas.set(x, y + h - 1, '└');
    for i in 1..w - 1 {
        canvas.set(x + i, y + h - 1, '─');
    }
    canvas.set(x + w - 1, y + h - 1, '┘');

    // Label centered on middle row
    let mid_row = h / 2;
    let label_x = x + (w - node.label.len()) / 2;
    canvas.draw_text(label_x, y + mid_row, &node.label);
}

fn draw_rounded(canvas: &mut Canvas, node: &PositionedNode) {
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;

    canvas.set(x, y, '╭');
    for i in 1..w - 1 {
        canvas.set(x + i, y, '─');
    }
    canvas.set(x + w - 1, y, '╮');

    for row in 1..h - 1 {
        canvas.set(x, y + row, '│');
        canvas.set(x + w - 1, y + row, '│');
    }

    canvas.set(x, y + h - 1, '╰');
    for i in 1..w - 1 {
        canvas.set(x + i, y + h - 1, '─');
    }
    canvas.set(x + w - 1, y + h - 1, '╯');

    let mid_row = h / 2;
    let label_x = x + (w - node.label.len()) / 2;
    canvas.draw_text(label_x, y + mid_row, &node.label);
}

fn draw_diamond(canvas: &mut Canvas, node: &PositionedNode) {
    let cx = node.x + node.width / 2;
    let cy = node.y + node.height / 2;
    let half_h = node.height / 2;

    // Draw top half (rows from cy-half_h to cy)
    for row in 0..=half_h {
        let offset = row;
        let left = cx.saturating_sub(offset);
        let right = cx + offset;
        if row == 0 {
            // Top vertex: just one point is the apex — draw the two chars
            canvas.set(cx.saturating_sub(1), node.y, '/');
            canvas.set(cx, node.y, '\\');
        } else if row == half_h {
            // Middle row: label centered, sides
            canvas.set(left, cy, '/');
            canvas.set(right, cy, '\\');
            // Label
            let label_start = left + 1;
            canvas.draw_text(label_start, cy, &node.label);
        } else {
            canvas.set(left, node.y + row, '/');
            canvas.set(right, node.y + row, '\\');
        }
    }

    // Draw bottom half (rows from cy+1 to cy+half_h)
    for row in 1..=half_h {
        let offset = half_h - row;
        let left = cx.saturating_sub(offset);
        let right = cx + offset;
        let y = cy + row;
        if row == half_h {
            // Bottom vertex
            canvas.set(cx.saturating_sub(1), y, '\\');
            canvas.set(cx, y, '/');
        } else {
            canvas.set(left, y, '\\');
            canvas.set(right, y, '/');
        }
    }
}

fn draw_node(canvas: &mut Canvas, node: &PositionedNode) {
    match node.shape {
        NodeShape::Rect => draw_rect(canvas, node),
        NodeShape::Rounded | NodeShape::Circle => draw_rounded(canvas, node),
        NodeShape::Diamond => draw_diamond(canvas, node),
    }
}

// ---------------------------------------------------------------------------
// Edge drawing
// ---------------------------------------------------------------------------

fn draw_edge(canvas: &mut Canvas, edge: &PositionedEdge) {
    let points = &edge.points;
    if points.len() < 2 {
        return;
    }

    let (h_char, v_char) = match edge.style {
        EdgeStyle::Dotted => ('.', ':'),
        EdgeStyle::Thick => ('═', '║'),
        _ => ('─', '│'),
    };

    let draw_arrow = matches!(
        edge.style,
        EdgeStyle::Arrow | EdgeStyle::Dotted | EdgeStyle::Thick
    );

    for seg in 0..points.len() - 1 {
        let (x0, y0) = points[seg];
        let (x1, y1) = points[seg + 1];

        if x0 == x1 {
            // Vertical segment
            let (y_start, y_end) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
            for y in y_start..=y_end {
                canvas.set(x0, y, v_char);
            }
            // Arrow head at endpoint
            if draw_arrow && seg == points.len() - 2 {
                if y1 > y0 {
                    canvas.set(x1, y1, '▼');
                } else {
                    canvas.set(x1, y1, '▲');
                }
            }
        } else {
            // Horizontal segment
            let (x_start, x_end) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
            for x in x_start..=x_end {
                canvas.set(x, y0, h_char);
            }
            // Arrow head at endpoint
            if draw_arrow && seg == points.len() - 2 {
                if x1 > x0 {
                    canvas.set(x1, y1, '►');
                } else {
                    canvas.set(x1, y1, '◄');
                }
            }
        }
    }

    // Edge label near the first segment, offset by 2 from start x
    if let Some(label) = &edge.label
        && !label.is_empty()
    {
        let (x0, y0) = points[0];
        canvas.draw_text(x0 + 2, y0, label);
    }
}

// ---------------------------------------------------------------------------
// render
// ---------------------------------------------------------------------------

pub fn render(layout: &LayoutResult) -> Vec<String> {
    let extra_width = 10;
    let extra_height = 2;

    let canvas_width = layout.width + extra_width;
    let canvas_height = layout.height + extra_height;

    // Guard: avoid creating empty canvas when layout is empty
    if canvas_width == 0 || canvas_height == 0 {
        return vec![];
    }

    let mut canvas = Canvas::new(canvas_width, canvas_height);

    // Draw edges first so nodes draw on top
    for edge in &layout.edges {
        draw_edge(&mut canvas, edge);
    }

    for node in &layout.nodes {
        draw_node(&mut canvas, node);
    }

    // Collect lines, removing trailing empty lines
    let mut lines = canvas.to_lines();
    while lines.last().map(|l: &String| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::{EdgeStyle, NodeShape};

    fn make_positioned_node(
        id: &str,
        label: &str,
        shape: NodeShape,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> PositionedNode {
        PositionedNode {
            id: id.to_string(),
            label: label.to_string(),
            shape,
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn test_render_single_rect() {
        // "Hi" rect: label len=2, width=6, height=3
        let node = make_positioned_node("A", "Hi", NodeShape::Rect, 0, 0, 6, 3);
        let layout = LayoutResult {
            nodes: vec![node],
            edges: vec![],
            width: 6,
            height: 3,
        };
        let lines = render(&layout);

        // Should have at least 3 lines
        assert!(lines.len() >= 3, "Expected at least 3 lines, got {}", lines.len());

        // Top row must contain box-drawing top-left corner
        assert!(
            lines[0].contains('┌'),
            "Top row should contain '┌', got: {:?}",
            lines[0]
        );
        assert!(
            lines[0].contains('┐'),
            "Top row should contain '┐', got: {:?}",
            lines[0]
        );
        assert!(
            lines[0].contains('─'),
            "Top row should contain '─', got: {:?}",
            lines[0]
        );

        // Bottom row
        assert!(
            lines[2].contains('└'),
            "Bottom row should contain '└', got: {:?}",
            lines[2]
        );
        assert!(
            lines[2].contains('┘'),
            "Bottom row should contain '┘', got: {:?}",
            lines[2]
        );

        // Middle row has label and side borders
        assert!(
            lines[1].contains('│'),
            "Middle row should contain '│', got: {:?}",
            lines[1]
        );
        assert!(
            lines[1].contains("Hi"),
            "Middle row should contain label 'Hi', got: {:?}",
            lines[1]
        );
    }

    #[test]
    fn test_render_rounded() {
        // "Hi" rounded node
        let node = make_positioned_node("A", "Hi", NodeShape::Rounded, 0, 0, 6, 3);
        let layout = LayoutResult {
            nodes: vec![node],
            edges: vec![],
            width: 6,
            height: 3,
        };
        let lines = render(&layout);

        assert!(lines.len() >= 3);

        // Rounded corners
        assert!(
            lines[0].contains('╭'),
            "Top row should contain '╭', got: {:?}",
            lines[0]
        );
        assert!(
            lines[0].contains('╮'),
            "Top row should contain '╮', got: {:?}",
            lines[0]
        );
        assert!(
            lines[2].contains('╰'),
            "Bottom row should contain '╰', got: {:?}",
            lines[2]
        );
        assert!(
            lines[2].contains('╯'),
            "Bottom row should contain '╯', got: {:?}",
            lines[2]
        );

        // Label present
        assert!(
            lines[1].contains("Hi"),
            "Middle row should contain 'Hi', got: {:?}",
            lines[1]
        );
    }

    #[test]
    fn test_render_two_nodes_with_edge() {
        use crate::mermaid::layout::PositionedEdge;

        // Node A at (0,0) width=6 height=3
        // Node B at (0,7) width=6 height=3
        let node_a = make_positioned_node("A", "Hi", NodeShape::Rect, 0, 0, 6, 3);
        let node_b = make_positioned_node("B", "Lo", NodeShape::Rect, 0, 7, 6, 3);

        // Edge: straight vertical from bottom of A to top of B
        let edge = PositionedEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            style: EdgeStyle::Arrow,
            points: vec![(3, 3), (3, 7)],
        };

        let layout = LayoutResult {
            nodes: vec![node_a, node_b],
            edges: vec![edge],
            width: 6,
            height: 10,
        };
        let lines = render(&layout);

        // Both nodes should appear
        let all_text: String = lines.join("\n");
        assert!(all_text.contains('┌'), "Should contain rect corner '┌'");
        assert!(all_text.contains("Hi"), "Should contain label 'Hi'");
        assert!(all_text.contains("Lo"), "Should contain label 'Lo'");

        // Edge vertical character should appear
        assert!(
            all_text.contains('│') || all_text.contains('▼'),
            "Should contain edge char '│' or arrow '▼'"
        );
    }

    #[test]
    fn test_render_diamond() {
        // Diamond node with label "Yes"
        // label len=3, inner_w=5, half=(5+1)/2=3, width=7, height=7
        let node = make_positioned_node("D", "Yes", NodeShape::Diamond, 0, 0, 7, 7);
        let layout = LayoutResult {
            nodes: vec![node],
            edges: vec![],
            width: 7,
            height: 7,
        };
        let lines = render(&layout);

        let all_text: String = lines.join("\n");

        // Must contain slash characters for diamond shape
        assert!(all_text.contains('/'), "Diamond should contain '/'");
        assert!(all_text.contains('\\'), "Diamond should contain '\\'");

        // Label must be present
        assert!(all_text.contains("Yes"), "Diamond should contain label 'Yes'");
    }

    #[test]
    fn test_canvas_to_lines_trims_trailing_spaces() {
        let mut canvas = Canvas::new(10, 3);
        // Draw only a short text, rest should be trimmed
        canvas.draw_text(0, 1, "hi");

        let lines = canvas.to_lines();

        // Line 0: empty → ""
        assert_eq!(lines[0], "", "Empty row should trim to empty string");
        // Line 1: "hi" with trailing spaces trimmed
        assert_eq!(lines[1], "hi", "Row with text should trim trailing spaces");
        // Line 2: empty → ""
        assert_eq!(lines[2], "", "Empty row should trim to empty string");
    }
}
