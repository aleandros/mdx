use super::{EntityLine, EntityLineKind, ErDiagram};
use crate::mermaid::{Edge, EdgeStyle, FlowChart, Node, NodeShape, display_width};

pub fn to_flowchart(diagram: &mut ErDiagram, max_box_width: usize) -> FlowChart {
    for entity in diagram.entities.iter_mut() {
        layout_entity(entity, max_box_width);
    }

    let nodes: Vec<Node> = diagram
        .entities
        .iter()
        .map(|e| Node {
            id: e.name.clone(),
            label: e.name.clone(),
            shape: NodeShape::EntityBox,
            node_style: e.node_style.clone(),
            entity: Some(e.clone()),
        })
        .collect();

    let edges: Vec<Edge> = diagram
        .relationships
        .iter()
        .map(|r| Edge {
            from: r.left.clone(),
            to: r.right.clone(),
            label: r.label.clone(),
            style: if r.identifying {
                EdgeStyle::Arrow
            } else {
                EdgeStyle::Dotted
            },
            edge_style: None,
            er_meta: Some(super::ErEdgeMeta {
                left_card: r.left_card,
                right_card: r.right_card,
                identifying: r.identifying,
            }),
        })
        .collect();

    FlowChart {
        direction: diagram.direction.clone(),
        nodes,
        edges,
        subgraphs: Vec::new(),
    }
}

fn key_str(k: super::KeyKind) -> &'static str {
    match k {
        super::KeyKind::None => "",
        super::KeyKind::Pk => "PK",
        super::KeyKind::Fk => "FK",
        super::KeyKind::PkFk => "PK,FK",
    }
}

fn layout_entity(entity: &mut super::Entity, max_box_width: usize) {
    let key_w = entity
        .attributes
        .iter()
        .map(|a| key_str(a.key).len())
        .max()
        .unwrap_or(0);
    let ty_w = entity
        .attributes
        .iter()
        .map(|a| display_width(&a.ty))
        .max()
        .unwrap_or(0);
    let name_w = entity
        .attributes
        .iter()
        .map(|a| display_width(&a.name))
        .max()
        .unwrap_or(0);

    let header_text = format!(" {} ", entity.name);

    // ` KEY TY NAME ` widths: leading space + key + space + ty + space + name + trailing space
    let attr_prefix_w = 1 + key_w + 1 + ty_w + 1 + name_w + 1;
    let inner_max = max_box_width.saturating_sub(2);
    let inline_comment_budget = inner_max.saturating_sub(attr_prefix_w);
    // Continuation rows align under the NAME column. NAME starts at:
    let continuation_indent = 1 + key_w + 1 + ty_w + 1;
    // Available width for wrapped comment text on a continuation row:
    let continuation_budget = inner_max
        .saturating_sub(continuation_indent + 1) // +1 for trailing space pad
        .max(1);

    let mut row_lines: Vec<EntityLine> = Vec::new();
    let mut max_row_w = 0usize;

    for a in &entity.attributes {
        // `{:<w$}` pads by char count, not columns; pad by display width.
        let base = format!(
            " {} {} {} ",
            pad_to(key_str(a.key), key_w),
            pad_to(&a.ty, ty_w),
            pad_to(&a.name, name_w),
        );
        match &a.comment {
            None => {
                max_row_w = max_row_w.max(display_width(&base));
                row_lines.push(EntityLine {
                    kind: EntityLineKind::AttrRow,
                    text: base,
                });
            }
            Some(c) => {
                if display_width(c) <= inline_comment_budget && inline_comment_budget > 0 {
                    let combined = format!("{}{} ", base, c);
                    max_row_w = max_row_w.max(display_width(&combined));
                    row_lines.push(EntityLine {
                        kind: EntityLineKind::AttrRow,
                        text: combined,
                    });
                } else {
                    max_row_w = max_row_w.max(display_width(&base));
                    row_lines.push(EntityLine {
                        kind: EntityLineKind::AttrRow,
                        text: base,
                    });
                    let pad = " ".repeat(continuation_indent);
                    for chunk in wrap_words(c, continuation_budget) {
                        let line = format!("{}{} ", pad, chunk);
                        max_row_w = max_row_w.max(display_width(&line));
                        row_lines.push(EntityLine {
                            kind: EntityLineKind::CommentRow,
                            text: line,
                        });
                    }
                }
            }
        }
    }

    let header_w = display_width(&header_text);
    let inner_w = header_w.max(max_row_w).min(inner_max.max(header_w));

    let width = inner_w + 2;

    let mut lines = vec![EntityLine {
        kind: EntityLineKind::Header,
        text: pad_to(&header_text, inner_w),
    }];
    if !entity.attributes.is_empty() {
        lines.push(EntityLine {
            kind: EntityLineKind::Separator,
            text: "-".repeat(inner_w),
        });
        for mut r in row_lines {
            r.text = pad_to(&r.text, inner_w);
            lines.push(r);
        }
    }

    entity.height = lines.len() + 2; // top + bottom borders
    entity.rendered_lines = lines;
    entity.width = width;
}

/// Greedy word wrap measured in display columns. A single word wider than
/// `max_w` is split at character boundaries (never inside a UTF-8 sequence).
fn wrap_words(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let word_w = display_width(word);
        if !current.is_empty() {
            if display_width(&current) + 1 + word_w <= max_w {
                current.push(' ');
                current.push_str(word);
                continue;
            }
            out.push(std::mem::take(&mut current));
        }
        if word_w <= max_w {
            current = word.to_string();
            continue;
        }
        // Over-long word: chop into column-bounded pieces.
        let mut piece = String::new();
        let mut piece_w = 0usize;
        for ch in word.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if piece_w + cw > max_w && !piece.is_empty() {
                out.push(std::mem::take(&mut piece));
                piece_w = 0;
            }
            piece.push(ch);
            piece_w += cw;
        }
        current = piece;
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn pad_to(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::Direction;
    use crate::mermaid::er::{Cardinality, Entity, ErDiagram, Relationship};

    fn empty_entity(name: &str) -> Entity {
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
    fn test_to_flowchart_empty_entity_box_dimensions() {
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![empty_entity("Foo")],
            relationships: Vec::new(),
        };
        let chart = to_flowchart(&mut diag, 50);
        assert_eq!(chart.nodes.len(), 1);
        let node = &chart.nodes[0];
        assert_eq!(node.shape, crate::mermaid::NodeShape::EntityBox);
        let entity = node.entity.as_ref().unwrap();
        // Width = name + borders + padding (at least name.len() + 4)
        assert!(entity.width >= "Foo".len() + 4);
        assert!(entity.height >= 3); // top border + name row + bottom border
    }

    #[test]
    fn test_to_flowchart_relationship_becomes_edge() {
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![empty_entity("A"), empty_entity("B")],
            relationships: vec![Relationship {
                left: "A".into(),
                right: "B".into(),
                left_card: Cardinality::ExactlyOne,
                right_card: Cardinality::ZeroOrMany,
                identifying: true,
                label: Some("has".into()),
            }],
        };
        let chart = to_flowchart(&mut diag, 50);
        assert_eq!(chart.edges.len(), 1);
        let edge = &chart.edges[0];
        assert_eq!(edge.from, "A");
        assert_eq!(edge.to, "B");
        assert_eq!(edge.label.as_deref(), Some("has"));
        let meta = edge.er_meta.as_ref().unwrap();
        assert_eq!(meta.left_card, Cardinality::ExactlyOne);
        assert_eq!(meta.right_card, Cardinality::ZeroOrMany);
        assert!(meta.identifying);
    }

    #[test]
    fn test_to_flowchart_attribute_columns_aligned() {
        use crate::mermaid::er::{Attribute, EntityLineKind, KeyKind};
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![Entity {
                name: "Foo".into(),
                attributes: vec![
                    Attribute {
                        ty: "string".into(),
                        name: "id".into(),
                        key: KeyKind::Pk,
                        comment: None,
                    },
                    Attribute {
                        ty: "int".into(),
                        name: "ttlMillis".into(),
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
        let _ = to_flowchart(&mut diag, 50);
        let entity = diag.entities[0].clone();
        let attr_rows: Vec<&str> = entity
            .rendered_lines
            .iter()
            .filter(|l| l.kind == EntityLineKind::AttrRow)
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(attr_rows.len(), 2);
        let r0 = attr_rows[0];
        let r1 = attr_rows[1];
        let ty_col_0 = r0.find("string").unwrap();
        let ty_col_1 = r1.find("int").unwrap();
        assert_eq!(
            ty_col_0, ty_col_1,
            "type column not aligned: `{}` vs `{}`",
            r0, r1
        );
        let name_col_0 = r0.find("id").unwrap();
        let name_col_1 = r1.find("ttlMillis").unwrap();
        assert_eq!(name_col_0, name_col_1, "name column not aligned");
        assert!(r0.contains("PK"));
        assert!(!r1.contains("PK"));
    }

    #[test]
    fn test_to_flowchart_short_comment_inlined() {
        use crate::mermaid::er::{Attribute, EntityLineKind, KeyKind};
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![Entity {
                name: "Foo".into(),
                attributes: vec![Attribute {
                    ty: "string".into(),
                    name: "id".into(),
                    key: KeyKind::Pk,
                    comment: Some("primary".into()),
                }],
                rendered_lines: Vec::new(),
                width: 0,
                height: 0,
                node_style: None,
            }],
            relationships: Vec::new(),
        };
        to_flowchart(&mut diag, 50);
        let lines = &diag.entities[0].rendered_lines;
        let attr = lines
            .iter()
            .find(|l| l.kind == EntityLineKind::AttrRow)
            .unwrap();
        assert!(
            attr.text.contains("primary"),
            "expected inlined comment, got `{}`",
            attr.text
        );
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.kind == EntityLineKind::CommentRow)
                .count(),
            0
        );
    }

    #[test]
    fn test_to_flowchart_long_comment_wraps_to_subsequent_rows() {
        use crate::mermaid::er::{Attribute, EntityLineKind, KeyKind};
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![Entity {
                name: "Foo".into(),
                attributes: vec![Attribute {
                    ty: "int".into(),
                    name: "ttlMs".into(),
                    key: KeyKind::None,
                    comment: Some(
                        "max age before discard, default ten days, applied at send time".into(),
                    ),
                }],
                rendered_lines: Vec::new(),
                width: 0,
                height: 0,
                node_style: None,
            }],
            relationships: Vec::new(),
        };
        to_flowchart(&mut diag, 40);
        let lines = &diag.entities[0].rendered_lines;
        let comment_rows: Vec<&str> = lines
            .iter()
            .filter(|l| l.kind == EntityLineKind::CommentRow)
            .map(|l| l.text.as_str())
            .collect();
        assert!(
            comment_rows.len() >= 2,
            "expected wrapping, got {} rows",
            comment_rows.len()
        );
        assert!(
            diag.entities[0].width <= 40,
            "box width {} exceeds max 40",
            diag.entities[0].width
        );
    }

    #[test]
    fn test_to_flowchart_propagates_node_style() {
        use crate::mermaid::NodeStyle;
        use crate::render::Color;
        let style = NodeStyle {
            fill: None,
            stroke: Some(Color::Red),
            color: Some(Color::Blue),
        };
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![Entity {
                name: "Foo".into(),
                attributes: Vec::new(),
                rendered_lines: Vec::new(),
                width: 0,
                height: 0,
                node_style: Some(style.clone()),
            }],
            relationships: Vec::new(),
        };
        let chart = to_flowchart(&mut diag, 50);
        assert_eq!(chart.nodes[0].node_style, Some(style));
    }

    #[test]
    fn test_to_flowchart_non_identifying_uses_dotted_style() {
        let mut diag = ErDiagram {
            direction: Direction::TopDown,
            direction_explicit: false,
            entities: vec![empty_entity("A"), empty_entity("B")],
            relationships: vec![Relationship {
                left: "A".into(),
                right: "B".into(),
                left_card: Cardinality::ExactlyOne,
                right_card: Cardinality::ExactlyOne,
                identifying: false,
                label: None,
            }],
        };
        let chart = to_flowchart(&mut diag, 50);
        assert_eq!(chart.edges[0].style, crate::mermaid::EdgeStyle::Dotted);
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use crate::mermaid::er::{Attribute, Entity, KeyKind};

    fn entity_with(name: &str, attrs: Vec<(&str, &str, Option<&str>)>) -> Entity {
        Entity {
            name: name.to_string(),
            attributes: attrs
                .into_iter()
                .map(|(ty, name, comment)| Attribute {
                    ty: ty.to_string(),
                    name: name.to_string(),
                    key: KeyKind::None,
                    comment: comment.map(str::to_string),
                })
                .collect(),
            rendered_lines: Vec::new(),
            width: 0,
            height: 0,
            node_style: None,
        }
    }

    /// Issue #3: every rendered row of an entity has the same display width,
    /// so CJK attribute names keep the columns and the right border aligned.
    #[test]
    fn entity_rows_align_by_display_width() {
        let mut e = entity_with(
            "用户",
            vec![
                ("string", "名字", None),
                ("int", "age", Some("年龄 in years")),
            ],
        );
        layout_entity(&mut e, 50);
        let widths: Vec<usize> = e
            .rendered_lines
            .iter()
            .map(|l| display_width(&l.text))
            .collect();
        assert!(
            widths.iter().all(|&w| w == widths[0]),
            "rows differ in display width: {widths:?} {:?}",
            e.rendered_lines
        );
        assert_eq!(e.width, widths[0] + 2);
        let names: Vec<usize> = e
            .rendered_lines
            .iter()
            .filter(|l| l.kind == EntityLineKind::AttrRow)
            .map(|l| {
                display_width(
                    l.text
                        .split_once("名字")
                        .or(l.text.split_once("age"))
                        .unwrap()
                        .0,
                )
            })
            .collect();
        assert_eq!(
            names[0], names[1],
            "name column misaligned: {:?}",
            e.rendered_lines
        );
    }

    /// A long non-ASCII word is split at character boundaries by display
    /// width; the old byte-based split panicked here.
    #[test]
    fn wrap_words_splits_wide_words_without_panicking() {
        let chunks = wrap_words("数据库数据库数据库 ok", 8);
        assert!(chunks.iter().all(|c| display_width(c) <= 8), "{chunks:?}");
        assert_eq!(chunks.concat().replace(' ', ""), "数据库数据库数据库ok");
        let chunks = wrap_words("Ünïcödé-sehr-lang", 5);
        assert!(chunks.iter().all(|c| display_width(c) <= 5), "{chunks:?}");
    }

    #[test]
    fn pad_to_uses_display_width() {
        assert_eq!(display_width(&pad_to("数据", 6)), 6);
        assert_eq!(pad_to("数据", 2), "数据");
    }
}

#[cfg(test)]
pub fn layout_entity_for_test(entity: &mut super::Entity, max_box_width: usize) {
    layout_entity(entity, max_box_width)
}
