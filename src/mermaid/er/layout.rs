use super::{EntityLine, EntityLineKind, ErDiagram};
use crate::mermaid::{Edge, EdgeStyle, FlowChart, Node, NodeShape};

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
        .map(|a| a.ty.len())
        .max()
        .unwrap_or(0);
    let name_w = entity
        .attributes
        .iter()
        .map(|a| a.name.len())
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
        let base = format!(
            " {:<kw$} {:<tw$} {:<nw$} ",
            key_str(a.key),
            a.ty,
            a.name,
            kw = key_w,
            tw = ty_w,
            nw = name_w,
        );
        match &a.comment {
            None => {
                max_row_w = max_row_w.max(base.len());
                row_lines.push(EntityLine {
                    kind: EntityLineKind::AttrRow,
                    text: base,
                });
            }
            Some(c) => {
                if c.len() <= inline_comment_budget && inline_comment_budget > 0 {
                    let combined = format!("{}{} ", base, c);
                    max_row_w = max_row_w.max(combined.len());
                    row_lines.push(EntityLine {
                        kind: EntityLineKind::AttrRow,
                        text: combined,
                    });
                } else {
                    max_row_w = max_row_w.max(base.len());
                    row_lines.push(EntityLine {
                        kind: EntityLineKind::AttrRow,
                        text: base,
                    });
                    let pad = " ".repeat(continuation_indent);
                    for chunk in wrap_words(c, continuation_budget) {
                        let line = format!("{}{} ", pad, chunk);
                        max_row_w = max_row_w.max(line.len());
                        row_lines.push(EntityLine {
                            kind: EntityLineKind::CommentRow,
                            text: line,
                        });
                    }
                }
            }
        }
    }

    let inner_w = std::iter::once(header_text.len())
        .chain(std::iter::once(max_row_w))
        .max()
        .unwrap_or(0)
        .min(inner_max.max(header_text.len()));

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

fn wrap_words(text: &str, max_w: usize) -> Vec<String> {
    if max_w == 0 {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            if word.len() > max_w {
                let mut w = word;
                while w.len() > max_w {
                    let (head, tail) = w.split_at(max_w);
                    out.push(head.to_string());
                    w = tail;
                }
                current = w.to_string();
            } else {
                current = word.to_string();
            }
        } else if current.len() + 1 + word.len() <= max_w {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(std::mem::take(&mut current));
            if word.len() > max_w {
                let mut w = word;
                while w.len() > max_w {
                    let (head, tail) = w.split_at(max_w);
                    out.push(head.to_string());
                    w = tail;
                }
                current = w.to_string();
            } else {
                current = word.to_string();
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - s.len()))
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
