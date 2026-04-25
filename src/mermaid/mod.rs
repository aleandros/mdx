pub mod ascii;
pub mod color;
// The `er` module is scaffolded; subsequent tasks fill in functionality.
#[allow(dead_code)]
pub mod er;
pub mod layout;
pub mod parse;
pub mod sequence;

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    TopDown,
    BottomTop,
    LeftRight,
    RightLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeShape {
    Rect,
    Rounded,
    Diamond,
    Circle,
    /// ER entity (table-like multi-row box). Constructed in a later task.
    #[allow(dead_code)]
    EntityBox,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeStyle {
    Arrow,
    Line,
    Dotted,
    Thick,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeStyle {
    pub fill: Option<crate::render::Color>,
    pub stroke: Option<crate::render::Color>,
    pub color: Option<crate::render::Color>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MermaidEdgeStyle {
    pub stroke: Option<crate::render::Color>,
    pub label_color: Option<crate::render::Color>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    pub node_style: Option<NodeStyle>,
    /// Set only for ER entity boxes; None for flowchart nodes.
    pub entity: Option<er::Entity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
    pub edge_style: Option<MermaidEdgeStyle>,
    /// Set only for ER edges; None for flowchart edges.
    pub er_meta: Option<er::ErEdgeMeta>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subgraph {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowChart {
    pub direction: Direction,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
}

/// Returns (styled_lines, node_count, edge_count)
pub fn render_mermaid(
    content: &str,
    theme: &crate::theme::Theme,
) -> anyhow::Result<(Vec<crate::render::StyledLine>, usize, usize)> {
    let first_line = content
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .unwrap_or("");

    if first_line == "sequenceDiagram" {
        let mut diagram = sequence::parse::parse_sequence(content)?;
        let participant_count = diagram.participants.len();
        let event_count = diagram.events.len();

        // Resolve colors to nearest theme match
        for p in &mut diagram.participants {
            if let Some(ref mut style) = p.style {
                resolve_node_style(style, theme);
            }
        }
        resolve_event_styles(&mut diagram.events, theme);

        let mut laid_out = sequence::layout::layout(&diagram);

        // Apply theme default colors to unstyled participants and messages.
        for p in &mut laid_out.participants {
            if p.style.is_none() {
                p.style = Some(NodeStyle {
                    fill: None,
                    stroke: Some(color::resolve_color(&theme.diagram_node_border, theme)),
                    color: Some(color::resolve_color(&theme.diagram_node_text, theme)),
                });
            }
        }
        for msg in &mut laid_out.messages {
            if msg.edge_style.is_none() {
                msg.edge_style = Some(MermaidEdgeStyle {
                    stroke: Some(color::resolve_color(&theme.diagram_edge_stroke, theme)),
                    label_color: Some(color::resolve_color(&theme.diagram_edge_label, theme)),
                });
            }
        }

        let lines = sequence::ascii::render_styled(&laid_out);
        Ok((lines, participant_count, event_count))
    } else {
        let mut chart = parse::parse_flowchart(content)?;
        let node_count = chart.nodes.len();
        let edge_count = chart.edges.len();

        // Resolve colors to nearest theme match
        for node in &mut chart.nodes {
            if let Some(ref mut style) = node.node_style {
                resolve_node_style(style, theme);
            }
        }
        for edge in &mut chart.edges {
            if let Some(ref mut style) = edge.edge_style {
                resolve_edge_style(style, theme);
            }
        }

        let mut positioned = layout::layout(&chart);

        // Apply theme defaults to elements without explicit styles.
        for node in &mut positioned.nodes {
            if node.node_style.is_none() {
                node.node_style = Some(NodeStyle {
                    fill: None,
                    stroke: Some(color::resolve_color(&theme.diagram_node_border, theme)),
                    color: Some(color::resolve_color(&theme.diagram_node_text, theme)),
                });
            }
        }
        for edge in &mut positioned.edges {
            if edge.edge_style.is_none() {
                edge.edge_style = Some(MermaidEdgeStyle {
                    stroke: None, // edge lines stay in terminal default color
                    label_color: Some(color::resolve_color(&theme.diagram_edge_label, theme)),
                });
            }
        }
        for sg in &mut positioned.subgraph_boxes {
            sg.border_color = Some(color::resolve_color(&theme.diagram_border, theme));
        }

        let lines = ascii::render_styled(&positioned);
        Ok((lines, node_count, edge_count))
    }
}

fn resolve_node_style(style: &mut NodeStyle, theme: &crate::theme::Theme) {
    if let Some(ref c) = style.fill {
        style.fill = Some(color::resolve_color(c, theme));
    }
    if let Some(ref c) = style.stroke {
        style.stroke = Some(color::resolve_color(c, theme));
    }
    if let Some(ref c) = style.color {
        style.color = Some(color::resolve_color(c, theme));
    }
}

fn resolve_edge_style(style: &mut MermaidEdgeStyle, theme: &crate::theme::Theme) {
    if let Some(ref c) = style.stroke {
        style.stroke = Some(color::resolve_color(c, theme));
    }
    if let Some(ref c) = style.label_color {
        style.label_color = Some(color::resolve_color(c, theme));
    }
}

fn resolve_event_styles(events: &mut [sequence::Event], theme: &crate::theme::Theme) {
    for event in events.iter_mut() {
        match event {
            sequence::Event::Message {
                edge_style: Some(style),
                ..
            } => {
                resolve_edge_style(style, theme);
            }
            sequence::Event::Fragment { sections, .. } => {
                for section in sections.iter_mut() {
                    resolve_event_styles(&mut section.events, theme);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn test_render_mermaid_with_theme_returns_styled_lines() {
        let input = "graph TD\n    A[Start] --> B[End]\n    style A stroke:#ff0000\n";
        let theme = Theme::default_theme();
        let (lines, node_count, edge_count) = render_mermaid(input, theme).unwrap();
        assert_eq!(node_count, 2);
        assert_eq!(edge_count, 1);
        let has_color = lines
            .iter()
            .any(|line| line.spans.iter().any(|s| s.style.fg.is_some()));
        assert!(has_color, "Styled node should produce colored spans");
    }

    #[test]
    fn test_render_mermaid_uses_theme_defaults() {
        // Unstyled diagrams now use theme default colors for node borders/text.
        let input = "graph TD\n    A --> B\n";
        let theme = Theme::default_theme();
        let (lines, _, _) = render_mermaid(input, theme).unwrap();
        let has_color = lines
            .iter()
            .any(|line| line.spans.iter().any(|s| s.style.fg.is_some()));
        assert!(
            has_color,
            "Unstyled diagram should use theme default colors"
        );
    }

    #[test]
    fn test_render_styled_flowchart_end_to_end() {
        let input = "graph TD\n    A[Start] --> B[End]\n    style A stroke:#ff0000\n    classDef blue fill:#0000ff\n    class B blue\n    linkStyle 0 stroke:#00ff00\n";
        let theme = Theme::default_theme();
        let (lines, node_count, edge_count) = render_mermaid(input, theme).unwrap();
        assert_eq!(node_count, 2);
        assert_eq!(edge_count, 1);
        assert!(!lines.is_empty());
        // Verify colors are present and are theme colors (not raw input colors)
        let all_colors: Vec<_> = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter_map(|s| s.style.fg.clone())
            .collect();
        assert!(!all_colors.is_empty(), "Should have colored spans");
        // All colors should be from the theme palette, not raw #ff0000
        for color in &all_colors {
            if let crate::render::Color::Rgb(r, g, b) = color {
                assert!(
                    !(*r == 255 && *g == 0 && *b == 0),
                    "Raw red should be resolved to nearest theme color, not passed through"
                );
            }
        }
    }

    #[test]
    fn test_render_styled_sequence_end_to_end() {
        let input = "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Hello\n    style A stroke:#ff0000\n    linkStyle 0 stroke:#00ff00\n";
        let theme = Theme::default_theme();
        let (lines, participant_count, event_count) = render_mermaid(input, theme).unwrap();
        assert_eq!(participant_count, 2);
        assert_eq!(event_count, 1);
        assert!(!lines.is_empty());
    }
}
