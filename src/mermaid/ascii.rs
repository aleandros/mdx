use super::layout::{LayoutResult, PositionedEdge, PositionedNode};
use super::{EdgeStyle, NodeShape};

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

pub(crate) struct Canvas {
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

fn clear_node_area(canvas: &mut Canvas, node: &PositionedNode) {
    for row in 0..node.height {
        for col in 0..node.width {
            canvas.set(node.x + col, node.y + row, ' ');
        }
    }
}

fn draw_rect(canvas: &mut Canvas, node: &PositionedNode) {
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;
    clear_node_area(canvas, node);

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
    clear_node_area(canvas, node);

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
    clear_node_area(canvas, node);
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

fn draw_compact_diamond(canvas: &mut Canvas, node: &PositionedNode) {
    let x = node.x;
    let y = node.y;
    let w = node.width;
    let h = node.height;
    clear_node_area(canvas, node);

    // Top row: /───\
    canvas.set(x, y, '/');
    for i in 1..w - 1 {
        canvas.set(x + i, y, '─');
    }
    canvas.set(x + w - 1, y, '\\');

    // Middle rows with label
    for row in 1..h - 1 {
        canvas.set(x, y + row, '<');
        canvas.set(x + w - 1, y + row, '>');
    }

    // Bottom row: \───/
    canvas.set(x, y + h - 1, '\\');
    for i in 1..w - 1 {
        canvas.set(x + i, y + h - 1, '─');
    }
    canvas.set(x + w - 1, y + h - 1, '/');

    // Label centered on middle row
    let mid_row = h / 2;
    let label_x = x + (w - node.label.len()) / 2;
    canvas.draw_text(label_x, y + mid_row, &node.label);
}

fn draw_node(canvas: &mut Canvas, node: &PositionedNode) {
    match node.shape {
        NodeShape::Rect => draw_rect(canvas, node),
        NodeShape::Rounded | NodeShape::Circle => draw_rounded(canvas, node),
        NodeShape::Diamond => {
            if node.compact {
                draw_compact_diamond(canvas, node);
            } else {
                draw_diamond(canvas, node);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Edge drawing — connectivity-grid approach
// ---------------------------------------------------------------------------
//
// Instead of drawing edge characters directly (which causes overwrite
// problems at corners and T-junctions), we build a connectivity bitmask
// grid, then convert to the correct box-drawing character for each cell.

const DIR_UP: u8 = 1;
const DIR_DOWN: u8 = 2;
const DIR_LEFT: u8 = 4;
const DIR_RIGHT: u8 = 8;

fn connectivity_to_char(bits: u8) -> char {
    match bits {
        0 => ' ',
        // Single or straight
        0b0001..=0b0011 => '│',
        0b0100 | 0b1000 | 0b1100 => '─',
        // Corners
        0b1001 => '└',
        0b1010 => '┌',
        0b0101 => '┘',
        0b0110 => '┐',
        // T-junctions
        0b1011 => '├',
        0b0111 => '┤',
        0b1101 => '┴',
        0b1110 => '┬',
        // Cross
        0b1111 => '┼',
        _ => '│',
    }
}

#[allow(clippy::needless_range_loop)]
fn mark_segment(conn: &mut [Vec<u8>], p0: (usize, usize), p1: (usize, usize), w: usize, h: usize) {
    if p0.0 == p1.0 {
        let x = p0.0;
        if x >= w {
            return;
        }
        let (y_start, y_end) = if p0.1 <= p1.1 {
            (p0.1, p1.1)
        } else {
            (p1.1, p0.1)
        };
        for y in y_start..=y_end {
            if y >= h {
                continue;
            }
            if y == y_start && y_start != y_end {
                conn[y][x] |= DIR_DOWN;
            } else if y == y_end && y_start != y_end {
                conn[y][x] |= DIR_UP;
            } else {
                conn[y][x] |= DIR_UP | DIR_DOWN;
            }
        }
    } else if p0.1 == p1.1 {
        let y = p0.1;
        if y >= h {
            return;
        }
        let (x_start, x_end) = if p0.0 <= p1.0 {
            (p0.0, p1.0)
        } else {
            (p1.0, p0.0)
        };
        for x in x_start..=x_end {
            if x >= w {
                continue;
            }
            if x == x_start && x_start != x_end {
                conn[y][x] |= DIR_RIGHT;
            } else if x == x_end && x_start != x_end {
                conn[y][x] |= DIR_LEFT;
            } else {
                conn[y][x] |= DIR_LEFT | DIR_RIGHT;
            }
        }
    }
}

fn mark_edge_connectivity(conn: &mut [Vec<u8>], points: &[(usize, usize)], w: usize, h: usize) {
    for seg in 0..points.len().saturating_sub(1) {
        mark_segment(conn, points[seg], points[seg + 1], w, h);
    }
}

fn apply_edge_style(canvas: &mut Canvas, conn: &[Vec<u8>], edge: &PositionedEdge) {
    let (h_char, v_char) = match edge.style {
        EdgeStyle::Dotted => ('.', ':'),
        EdgeStyle::Thick => ('═', '║'),
        _ => return,
    };

    for seg in 0..edge.points.len().saturating_sub(1) {
        let (x0, y0) = edge.points[seg];
        let (x1, y1) = edge.points[seg + 1];

        if x0 == x1 {
            let (y_start, y_end) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
            for y in y_start..=y_end {
                if y < conn.len()
                    && x0 < conn[0].len()
                    && (conn[y][x0] & (DIR_LEFT | DIR_RIGHT)) == 0
                {
                    canvas.set(x0, y, v_char);
                }
            }
        } else if y0 == y1 {
            let (x_start, x_end) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
            for x in x_start..=x_end {
                if y0 < conn.len() && x < conn[0].len() && (conn[y0][x] & (DIR_UP | DIR_DOWN)) == 0
                {
                    canvas.set(x, y0, h_char);
                }
            }
        }
    }
}

fn draw_arrowhead(canvas: &mut Canvas, edge: &PositionedEdge) {
    let draw_arrow = matches!(
        edge.style,
        EdgeStyle::Arrow | EdgeStyle::Dotted | EdgeStyle::Thick
    );
    if !draw_arrow || edge.points.len() < 2 {
        return;
    }

    let last = edge.points[edge.points.len() - 1];
    let prev = edge.points[edge.points.len() - 2];

    if last.0 == prev.0 {
        // Vertical last segment — place arrowhead one cell before target border
        if last.1.abs_diff(prev.1) >= 2 {
            if last.1 > prev.1 {
                canvas.set(last.0, last.1 - 1, '▼');
            } else {
                canvas.set(last.0, last.1 + 1, '▲');
            }
        }
    } else if last.1 == prev.1 {
        // Horizontal last segment
        if last.0.abs_diff(prev.0) >= 2 {
            if last.0 > prev.0 {
                canvas.set(last.0 - 1, last.1, '►');
            } else {
                canvas.set(last.0 + 1, last.1, '◄');
            }
        }
    }
}

fn draw_edge_label(canvas: &mut Canvas, edge: &PositionedEdge) {
    let label = match &edge.label {
        Some(l) if !l.is_empty() => l,
        _ => return,
    };
    let points = &edge.points;

    if points.len() >= 4 {
        // L-bend: place label on the unique middle segment (not the shared initial one)
        let (x1, y1) = points[1];
        let (x2, y2) = points[2];

        if y1 == y2 {
            // Horizontal middle segment — center label above it
            let x_min = x1.min(x2);
            let x_max = x1.max(x2);
            let seg_len = x_max - x_min;
            let label_x = if seg_len >= label.len() {
                x_min + (seg_len - label.len()) / 2
            } else {
                x_min
            };
            canvas.draw_text(label_x, y1.saturating_sub(1), label);
        } else {
            // Vertical middle segment — label to the right at midpoint
            let y_mid = (y1.min(y2) + y1.max(y2)) / 2;
            canvas.draw_text(x1 + 1, y_mid, label);
        }
    } else if points.len() >= 2 {
        // Straight edge
        let (x0, y0) = points[0];
        let (x1, y1) = points[1];

        if x0 == x1 {
            let y_mid = (y0.min(y1) + y0.max(y1)) / 2;
            canvas.draw_text(x0 + 1, y_mid, label);
        } else {
            let x_min = x0.min(x1);
            let x_max = x0.max(x1);
            let x_mid = (x_min + x_max) / 2;
            let label_x = x_mid.saturating_sub(label.len() / 2);
            canvas.draw_text(label_x, if y0 > 0 { y0 - 1 } else { y0 }, label);
        }
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

    if canvas_width == 0 || canvas_height == 0 {
        return vec![];
    }

    let mut canvas = Canvas::new(canvas_width, canvas_height);

    // 1. Build connectivity grid from all edge segments
    let mut conn = vec![vec![0u8; canvas_width]; canvas_height];
    for edge in &layout.edges {
        mark_edge_connectivity(&mut conn, &edge.points, canvas_width, canvas_height);
    }

    // 2. Draw box-drawing chars from connectivity
    for (y, row) in conn.iter().enumerate() {
        for (x, &bits) in row.iter().enumerate() {
            if bits != 0 {
                canvas.set(x, y, connectivity_to_char(bits));
            }
        }
    }

    // 3. Apply edge-specific styles (dotted / thick) on straight segments
    for edge in &layout.edges {
        apply_edge_style(&mut canvas, &conn, edge);
    }

    // 4. Draw arrowheads (before nodes, so nodes can overwrite border overlap)
    for edge in &layout.edges {
        draw_arrowhead(&mut canvas, edge);
    }

    // 5. Draw edge labels
    for edge in &layout.edges {
        draw_edge_label(&mut canvas, edge);
    }

    // 6. Draw nodes on top of edges
    for node in &layout.nodes {
        draw_node(&mut canvas, node);
    }

    // Trim trailing empty lines
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
            compact: false,
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
        assert!(
            lines.len() >= 3,
            "Expected at least 3 lines, got {}",
            lines.len()
        );

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
        assert!(
            all_text.contains("Yes"),
            "Diamond should contain label 'Yes'"
        );
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

    #[test]
    fn test_edge_label_not_on_node_border() {
        use crate::mermaid::layout::PositionedEdge;

        // Node A at (0,0) height=3 → bottom border at y=2
        // Node B at (0,10) → top border at y=10
        // Edge from (3, 3) to (3, 10): vertical segment through rows 3..9
        // Midpoint of first (and only) segment: y=(3+10)/2=6
        // Label should appear at row 6 — strictly between y=2 (node A bottom) and y=10 (node B top)
        let node_a = make_positioned_node("A", "Hi", NodeShape::Rect, 0, 0, 6, 3);
        let node_b = make_positioned_node("B", "Lo", NodeShape::Rect, 0, 10, 6, 3);

        let edge = PositionedEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            label: Some("yes".to_string()),
            style: EdgeStyle::Arrow,
            points: vec![(3, 3), (3, 10)],
        };

        let layout = LayoutResult {
            nodes: vec![node_a, node_b],
            edges: vec![edge],
            width: 10,
            height: 14,
        };
        let lines = render(&layout);

        // Find rows that contain "yes"
        let label_rows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("yes"))
            .map(|(i, _)| i)
            .collect();

        assert!(
            !label_rows.is_empty(),
            "Label 'yes' should appear in rendered output"
        );

        for &row in &label_rows {
            // Label must NOT be on node A's rows (0..=2) or node B's rows (10..=12)
            assert!(
                row > 2 && row < 10,
                "Label 'yes' at row {} should be between node borders (rows 3..9)",
                row
            );
        }
    }
}
