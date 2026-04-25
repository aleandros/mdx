use crate::mermaid::er::EntityLineKind;
use crate::mermaid::layout::{PositionedEdge, PositionedNode};

/// Paints the borders and inner content of an entity box at its positioned
/// coordinates onto `canvas_lines` (one String per row). Caller has already
/// allocated the canvas with sufficient height; rows are padded with spaces
/// as needed.
pub fn paint_entity(canvas_lines: &mut [String], node: &PositionedNode) {
    let Some(entity) = node.entity.as_ref() else {
        return;
    };

    let w = node.width;
    let h = node.height;
    if w < 2 || h < 2 {
        return;
    }

    // Top and bottom borders
    paint_horizontal(canvas_lines, node.x, node.y, w, '+', '-');
    paint_horizontal(canvas_lines, node.x, node.y + h - 1, w, '+', '-');

    // Side borders
    for dy in 1..(h - 1) {
        set_cell(canvas_lines, node.x, node.y + dy, '|');
        set_cell(canvas_lines, node.x + w - 1, node.y + dy, '|');
    }

    // Inner content rows
    let inner_top = node.y + 1;
    let inner_bottom = node.y + h - 1;
    for (i, line) in entity.rendered_lines.iter().enumerate() {
        let row_y = inner_top + i;
        if row_y >= inner_bottom {
            break;
        }
        let inner = match line.kind {
            EntityLineKind::Separator => "-".repeat(w.saturating_sub(2)),
            _ => line.text.clone(),
        };
        paint_text(canvas_lines, node.x + 1, row_y, &inner, w.saturating_sub(2));
    }
}

fn set_cell(canvas_lines: &mut [String], x: usize, y: usize, ch: char) {
    if y >= canvas_lines.len() {
        return;
    }
    let line = &mut canvas_lines[y];
    while line.chars().count() <= x {
        line.push(' ');
    }
    let mut chars: Vec<char> = line.chars().collect();
    chars[x] = ch;
    *line = chars.into_iter().collect();
}

fn paint_horizontal(
    canvas_lines: &mut [String],
    x: usize,
    y: usize,
    w: usize,
    corner: char,
    fill: char,
) {
    if y >= canvas_lines.len() || w < 2 {
        return;
    }
    set_cell(canvas_lines, x, y, corner);
    for dx in 1..(w - 1) {
        set_cell(canvas_lines, x + dx, y, fill);
    }
    set_cell(canvas_lines, x + w - 1, y, corner);
}

fn paint_text(canvas_lines: &mut [String], x: usize, y: usize, text: &str, max_w: usize) {
    if y >= canvas_lines.len() {
        return;
    }
    for (dx, ch) in text.chars().take(max_w).enumerate() {
        set_cell(canvas_lines, x + dx, y, ch);
    }
}

pub fn paint_cardinality(_canvas_lines: &mut [String], _edge: &PositionedEdge) {
    // Implemented in Task 12.
}

#[cfg(test)]
mod tests {
    use crate::mermaid::Direction;
    use crate::mermaid::er::layout::to_flowchart;
    use crate::mermaid::er::{Entity, ErDiagram};

    fn make_entity(name: &str) -> Entity {
        Entity {
            name: name.to_string(),
            attributes: Vec::new(),
            rendered_lines: Vec::new(),
            width: 0,
            height: 0,
        }
    }

    #[test]
    fn test_entity_box_styled_render_contains_name() {
        let mut diag = ErDiagram {
            direction: Direction::LeftRight,
            direction_explicit: false,
            entities: vec![make_entity("Foo")],
            relationships: Vec::new(),
        };
        let chart = to_flowchart(&mut diag, 40);
        let layout = crate::mermaid::layout::layout(&chart);
        let styled = crate::mermaid::ascii::render_styled(&layout);
        let joined: String = styled
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("Foo"),
            "name missing in styled:\n{}",
            joined
        );
        assert!(
            joined.contains("+-"),
            "border missing in styled:\n{}",
            joined
        );
        assert!(joined.contains("|"), "side missing in styled:\n{}", joined);
    }

    #[test]
    fn test_entity_box_renders_attributes_and_separator() {
        use crate::mermaid::er::{Attribute, KeyKind};
        let mut diag = ErDiagram {
            direction: Direction::LeftRight,
            direction_explicit: false,
            entities: vec![Entity {
                name: "Customer".to_string(),
                attributes: vec![
                    Attribute {
                        ty: "int".to_string(),
                        name: "id".to_string(),
                        key: KeyKind::Pk,
                        comment: None,
                    },
                    Attribute {
                        ty: "string".to_string(),
                        name: "name".to_string(),
                        key: KeyKind::None,
                        comment: None,
                    },
                ],
                rendered_lines: Vec::new(),
                width: 0,
                height: 0,
            }],
            relationships: Vec::new(),
        };
        let chart = to_flowchart(&mut diag, 40);
        let layout = crate::mermaid::layout::layout(&chart);
        let lines = crate::mermaid::ascii::render(&layout);
        let joined = lines.join("\n");
        assert!(joined.contains("Customer"), "header missing:\n{}", joined);
        assert!(joined.contains("PK"), "PK marker missing:\n{}", joined);
        assert!(joined.contains("id"), "id attr missing:\n{}", joined);
        assert!(joined.contains("name"), "name attr missing:\n{}", joined);
        // Separator row inside the box: a run of dashes between header and attrs.
        // Width is dynamic but at least 5 dashes will be present.
        assert!(
            joined.contains("-----"),
            "separator dashes missing:\n{}",
            joined
        );
    }

    #[test]
    fn test_entity_box_renders_name_and_borders() {
        let mut diag = ErDiagram {
            direction: Direction::LeftRight,
            direction_explicit: false,
            entities: vec![make_entity("Foo")],
            relationships: Vec::new(),
        };
        let chart = to_flowchart(&mut diag, 40);
        let layout = crate::mermaid::layout::layout(&chart);
        let lines = crate::mermaid::ascii::render(&layout);
        let joined = lines.join("\n");
        assert!(
            joined.contains("Foo"),
            "expected entity name in render:\n{}",
            joined
        );
        assert!(
            joined.contains("+-"),
            "expected box top corner:\n{}",
            joined
        );
        assert!(
            joined.contains("|"),
            "expected box side border:\n{}",
            joined
        );
    }
}
