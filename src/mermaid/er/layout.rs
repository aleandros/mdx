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
            node_style: None,
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

fn layout_entity(entity: &mut super::Entity, _max_box_width: usize) {
    // Header row: " Name " (1 padding cell on each side of the name)
    let header = format!(" {} ", entity.name);
    let inner_w = header.len();
    let width = inner_w + 2; // 1 border on each side
    let height = if entity.attributes.is_empty() {
        3 // top border + header + bottom border
    } else {
        // Attribute rows arrive in Task 9; reserve placeholder space.
        3 + 1 + entity.attributes.len()
    };

    let mut lines = vec![EntityLine {
        kind: EntityLineKind::Header,
        text: header,
    }];
    if !entity.attributes.is_empty() {
        lines.push(EntityLine {
            kind: EntityLineKind::Separator,
            text: "-".repeat(inner_w),
        });
        for a in &entity.attributes {
            // Placeholder text; full row format lands in Task 9.
            lines.push(EntityLine {
                kind: EntityLineKind::AttrRow,
                text: format!(" {} {} ", a.ty, a.name),
            });
        }
    }

    entity.rendered_lines = lines;
    entity.width = width;
    entity.height = height;
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
