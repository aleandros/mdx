use crate::mermaid::er::{Cardinality, EntityLineKind};
use crate::mermaid::layout::{PositionedEdge, PositionedNode};
use crate::render::{Color, SpanStyle, StyledLine, StyledSpan};

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

/// Recolors an entity's bounding-box region within `rows` cell-by-cell.
/// Border cells (`+`, `-`, `|` along the four edges of the box AND the
/// in-box separator row) take `node_style.stroke` (or `fill` as fallback).
/// Inner text cells take `node_style.color`. Cells outside the box keep
/// whatever color they already had.
///
/// The caller must have already painted the plain glyph content of the entity
/// into the rows (via `paint_entity` on a plain buffer that was then promoted
/// to single default-styled spans, or equivalent) so this function can read
/// the existing characters.
#[allow(dead_code)]
pub fn paint_entity_styled(rows: &mut [StyledLine], node: &crate::mermaid::layout::PositionedNode) {
    let Some(entity) = node.entity.as_ref() else {
        return;
    };
    let Some(style) = node.node_style.as_ref() else {
        return;
    };
    let stroke = style.stroke.clone().or_else(|| style.fill.clone());
    let text_color = style.color.clone();

    let w = node.width;
    let h = node.height;
    if w < 2 || h < 2 {
        return;
    }

    for dy in 0..h {
        let y = node.y + dy;
        if y >= rows.len() {
            continue;
        }
        let row_text = row_text_string(&rows[y]);
        let row_chars: Vec<char> = row_text.chars().collect();

        let mut new_spans: Vec<StyledSpan> = Vec::new();
        let mut current_text = String::new();
        let mut current_fg: Option<Color> = None;
        let mut started = false;

        for (x, ch) in row_chars.iter().enumerate() {
            let in_box_x = x >= node.x && x < node.x + w;
            let cell_fg =
                if in_box_x && (dy == 0 || dy == h - 1 || x == node.x || x == node.x + w - 1) {
                    // Outer-edge cell of the box.
                    stroke.clone()
                } else if in_box_x && dy >= 1 && dy < h - 1 {
                    // Inner content cell. If this row corresponds to a Separator
                    // entry in rendered_lines, treat it as a border (stroke).
                    let inner_y = dy.saturating_sub(1);
                    let kind = entity.rendered_lines.get(inner_y).map(|l| l.kind);
                    if matches!(kind, Some(crate::mermaid::er::EntityLineKind::Separator)) {
                        stroke.clone()
                    } else {
                        text_color.clone()
                    }
                } else {
                    // Outside the box: preserve existing color.
                    find_existing_fg(&rows[y], x)
                };

            if !started {
                current_fg = cell_fg.clone();
                current_text.push(*ch);
                started = true;
            } else if cell_fg == current_fg {
                current_text.push(*ch);
            } else {
                new_spans.push(StyledSpan {
                    text: std::mem::take(&mut current_text),
                    style: SpanStyle {
                        fg: current_fg.clone(),
                        ..Default::default()
                    },
                });
                current_fg = cell_fg;
                current_text.push(*ch);
            }
        }
        if started && !current_text.is_empty() {
            new_spans.push(StyledSpan {
                text: current_text,
                style: SpanStyle {
                    fg: current_fg,
                    ..Default::default()
                },
            });
        }
        rows[y] = StyledLine { spans: new_spans };
    }
}

#[allow(dead_code)]
fn row_text_string(line: &StyledLine) -> String {
    line.spans.iter().map(|s| s.text.as_str()).collect()
}

#[allow(dead_code)]
fn find_existing_fg(line: &StyledLine, x: usize) -> Option<Color> {
    let mut col = 0usize;
    for span in &line.spans {
        let len = span.text.chars().count();
        if x < col + len {
            return span.style.fg.clone();
        }
        col += len;
    }
    None
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
            node_style: None,
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
                node_style: None,
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

    #[test]
    fn test_paint_entity_styled_borders_use_stroke_color() {
        use crate::mermaid::NodeStyle;
        use crate::mermaid::layout::PositionedNode;
        use crate::render::{Color, StyledLine, StyledSpan};

        let mut entity = make_entity("Foo");
        crate::mermaid::er::layout::layout_entity_for_test(&mut entity, 30);
        let node = PositionedNode {
            id: "Foo".into(),
            label: "Foo".into(),
            shape: crate::mermaid::NodeShape::EntityBox,
            x: 0,
            y: 0,
            width: entity.width,
            height: entity.height,
            compact: false,
            node_style: Some(NodeStyle {
                fill: None,
                stroke: Some(Color::Red),
                color: Some(Color::Blue),
            }),
            entity: Some(entity.clone()),
        };
        let mut rows: Vec<StyledLine> = (0..node.height)
            .map(|_| StyledLine {
                spans: vec![StyledSpan {
                    text: " ".repeat(node.width),
                    style: Default::default(),
                }],
            })
            .collect();
        // First paint plain glyphs so the cell content (`+`, `-`, `|`, "Foo") exists.
        let mut plain: Vec<String> = (0..node.height).map(|_| " ".repeat(node.width)).collect();
        crate::mermaid::er::ascii::paint_entity(&mut plain, &node);
        // Promote plain rows back into styled rows as a single default-styled span each.
        for (i, line) in plain.into_iter().enumerate() {
            rows[i] = StyledLine {
                spans: vec![StyledSpan {
                    text: line,
                    style: Default::default(),
                }],
            };
        }
        // Now recolor.
        crate::mermaid::er::ascii::paint_entity_styled(&mut rows, &node);

        // Top border row contains '+' / '-' cells with fg = Red.
        let top: String = rows[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert!(top.contains('+') && top.contains('-'), "top row: `{top}`");
        let dash_span = rows[0]
            .spans
            .iter()
            .find(|s| s.text.contains('-'))
            .expect("expected dash span on border row");
        assert_eq!(dash_span.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_paint_entity_styled_text_uses_color() {
        use crate::mermaid::NodeStyle;
        use crate::mermaid::layout::PositionedNode;
        use crate::render::{Color, StyledLine, StyledSpan};

        let mut entity = make_entity("Foo");
        crate::mermaid::er::layout::layout_entity_for_test(&mut entity, 30);
        let node = PositionedNode {
            id: "Foo".into(),
            label: "Foo".into(),
            shape: crate::mermaid::NodeShape::EntityBox,
            x: 0,
            y: 0,
            width: entity.width,
            height: entity.height,
            compact: false,
            node_style: Some(NodeStyle {
                fill: None,
                stroke: Some(Color::Red),
                color: Some(Color::Blue),
            }),
            entity: Some(entity.clone()),
        };
        let mut rows: Vec<StyledLine> = (0..node.height)
            .map(|_| StyledLine {
                spans: vec![StyledSpan {
                    text: " ".repeat(node.width),
                    style: Default::default(),
                }],
            })
            .collect();
        let mut plain: Vec<String> = (0..node.height).map(|_| " ".repeat(node.width)).collect();
        crate::mermaid::er::ascii::paint_entity(&mut plain, &node);
        for (i, line) in plain.into_iter().enumerate() {
            rows[i] = StyledLine {
                spans: vec![StyledSpan {
                    text: line,
                    style: Default::default(),
                }],
            };
        }
        crate::mermaid::er::ascii::paint_entity_styled(&mut rows, &node);

        // Header row (y=1) contains "Foo" — the spans should mark "Foo" with fg = Blue.
        let foo_span = rows[1]
            .spans
            .iter()
            .find(|s| s.text.contains("Foo"))
            .expect("expected Foo span on header row");
        assert_eq!(foo_span.style.fg, Some(Color::Blue));
    }
}
