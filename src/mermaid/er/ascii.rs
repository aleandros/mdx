use crate::mermaid::er::{Cardinality, EntityLineKind};
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

fn left_glyph(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ExactlyOne => "||",
        Cardinality::ZeroOrOne => "o|",
        Cardinality::ZeroOrMany => "}o",
        Cardinality::OneOrMany => "}|",
    }
}

fn right_glyph(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ExactlyOne => "||",
        Cardinality::ZeroOrOne => "|o",
        Cardinality::ZeroOrMany => "o{",
        Cardinality::OneOrMany => "|{",
    }
}

pub fn paint_cardinality(canvas_lines: &mut [String], edge: &PositionedEdge) {
    let Some(meta) = edge.er_meta.as_ref() else {
        return;
    };
    if edge.points.len() < 2 {
        return;
    }
    let start = edge.points[0];
    let end = *edge.points.last().unwrap();

    let l_chars: Vec<char> = left_glyph(meta.left_card).chars().collect();
    let r_chars: Vec<char> = right_glyph(meta.right_card).chars().collect();

    paint_glyph_at(canvas_lines, start, edge.points[1], &l_chars);
    paint_glyph_at(
        canvas_lines,
        end,
        edge.points[edge.points.len() - 2],
        &r_chars,
    );
}

fn paint_glyph_at(
    canvas_lines: &mut [String],
    endpoint: (usize, usize),
    next: (usize, usize),
    glyph: &[char],
) {
    if glyph.len() != 2 {
        return;
    }
    // Convention: glyph[0] is the lower-coord cell, glyph[1] is the higher-coord
    // cell along the segment direction at the endpoint. Callers (left_glyph /
    // right_glyph) produce strings in canvas-order (left→right or top→bottom).
    let (c0, c1) = (glyph[0], glyph[1]);
    let (x, y) = endpoint;
    let dx = next.0 as isize - endpoint.0 as isize;
    let dy = next.1 as isize - endpoint.1 as isize;
    if dx.abs() > dy.abs() {
        if dx > 0 {
            // endpoint is the lower-coord cell; next is to the right.
            set_cell(canvas_lines, x, y, c0);
            set_cell(canvas_lines, x + 1, y, c1);
        } else if dx < 0 && x >= 1 {
            // endpoint is the higher-coord cell; next is to the left.
            set_cell(canvas_lines, x - 1, y, c0);
            set_cell(canvas_lines, x, y, c1);
        }
    } else if dy != 0 {
        if dy > 0 {
            set_cell(canvas_lines, x, y, c0);
            set_cell(canvas_lines, x, y + 1, c1);
        } else if y >= 1 {
            set_cell(canvas_lines, x, y - 1, c0);
            set_cell(canvas_lines, x, y, c1);
        }
    }
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
    fn test_relationship_renders_crow_foot_glyphs() {
        use crate::mermaid::er::{Cardinality, Relationship};
        let mut diag = ErDiagram {
            direction: Direction::LeftRight,
            direction_explicit: false,
            entities: vec![make_entity("A"), make_entity("B")],
            relationships: vec![Relationship {
                left: "A".into(),
                right: "B".into(),
                left_card: Cardinality::ExactlyOne,
                right_card: Cardinality::ZeroOrMany,
                identifying: true,
                label: None,
            }],
        };
        let chart = to_flowchart(&mut diag, 40);
        let layout = crate::mermaid::layout::layout(&chart);
        let lines = crate::mermaid::ascii::render(&layout);
        let joined = lines.join("\n");
        assert!(
            joined.contains("||"),
            "missing one-and-only-one glyph:\n{}",
            joined
        );
        assert!(
            joined.contains("o{"),
            "missing zero-or-many glyph:\n{}",
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
