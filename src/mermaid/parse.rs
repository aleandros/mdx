use std::collections::HashMap;

use anyhow::{Context, bail};

use super::color::{parse_edge_style_props, parse_node_style_props};
use super::{
    Direction, Edge, EdgeStyle, FlowChart, MermaidEdgeStyle, Node, NodeShape, NodeStyle, Subgraph,
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn parse_flowchart(input: &str) -> anyhow::Result<FlowChart> {
    let mut lines = input.lines().peekable();

    // -----------------------------------------------------------------------
    // First non-blank, non-comment line must be the graph declaration
    // -----------------------------------------------------------------------
    let direction = loop {
        match lines.next() {
            None => bail!("Empty input: no graph declaration found"),
            Some(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("%%") {
                    continue;
                }
                break parse_direction_line(trimmed)?;
            }
        }
    };

    // -----------------------------------------------------------------------
    // Process remaining lines
    // -----------------------------------------------------------------------
    // node_order: insertion-order list of ids
    // node_map:   id → Node (label/shape may be updated by explicit notation)
    let mut node_order: Vec<String> = Vec::new();
    let mut node_map: HashMap<String, Node> = HashMap::new();
    let mut edges: Vec<Edge> = Vec::new();

    let mut class_defs: HashMap<String, NodeStyle> = HashMap::new();
    let mut class_assignments: Vec<(Vec<String>, String)> = Vec::new();
    let mut link_styles: Vec<(usize, MermaidEdgeStyle)> = Vec::new();
    let mut node_styles: Vec<(String, NodeStyle)> = Vec::new();

    // Subgraph tracking
    let mut subgraphs: Vec<Subgraph> = Vec::new();
    // Stack of (id, label, member_node_ids) for nested subgraphs
    let mut subgraph_stack: Vec<(String, String, Vec<String>)> = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        // subgraph declarations: start a new subgraph context
        if let Some(rest) = trimmed.strip_prefix("subgraph") {
            let rest = rest.trim();
            let (sg_id, sg_label) = parse_subgraph_header(rest);
            subgraph_stack.push((sg_id, sg_label, Vec::new()));
            continue;
        }

        // end: close the current subgraph context
        if trimmed == "end" {
            if let Some((sg_id, sg_label, member_ids)) = subgraph_stack.pop() {
                subgraphs.push(Subgraph {
                    id: sg_id,
                    label: sg_label,
                    node_ids: member_ids,
                });
            }
            continue;
        }

        // style A fill:#f9f,stroke:#333,color:#000
        if let Some(rest) = trimmed.strip_prefix("style ") {
            if let Some((id, props)) = rest.split_once(' ') {
                let style = parse_node_style_props(props);
                node_styles.push((id.to_string(), style));
            }
            continue;
        }

        // classDef className fill:#f9f,stroke:#333
        if let Some(rest) = trimmed.strip_prefix("classDef ") {
            if let Some((name, props)) = rest.split_once(' ') {
                let style = parse_node_style_props(props);
                class_defs.insert(name.to_string(), style);
            }
            continue;
        }

        // class A,B className
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some((ids_str, class_name)) = rest.rsplit_once(' ') {
                let ids: Vec<String> = ids_str.split(',').map(|s| s.trim().to_string()).collect();
                class_assignments.push((ids, class_name.to_string()));
            }
            continue;
        }

        // linkStyle 0 stroke:#ff3
        if let Some(rest) = trimmed.strip_prefix("linkStyle ") {
            if let Some((idx_str, props)) = rest.split_once(' ')
                && let Ok(idx) = idx_str.parse::<usize>()
            {
                let style = parse_edge_style_props(props);
                link_styles.push((idx, style));
            }
            continue;
        }

        // Track node_order length before parsing to detect newly added nodes
        let before_len = node_order.len();
        parse_statement(trimmed, &mut node_order, &mut node_map, &mut edges)
            .with_context(|| format!("Failed to parse line: {:?}", trimmed))?;

        // Any newly added nodes belong to the current subgraph (if any).
        // Also handle already-existing nodes listed inside a subgraph block.
        if !subgraph_stack.is_empty() {
            let new_ids: Vec<String> = node_order[before_len..].to_vec();
            if let Some(top) = subgraph_stack.last_mut() {
                if !new_ids.is_empty() {
                    top.2.extend(new_ids);
                } else {
                    // Node already existed — extract its ID from the line and add to membership.
                    // This handles the pattern where edges appear before subgraph blocks,
                    // so nodes are pre-created but still need to be assigned to the subgraph.
                    let first_id: String = trimmed
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !first_id.is_empty()
                        && node_map.contains_key(&first_id)
                        && !top.2.contains(&first_id)
                    {
                        top.2.push(first_id);
                    }
                }
            }
        }
    }

    // Apply class assignments to nodes
    for (ids, class_name) in &class_assignments {
        if let Some(class_style) = class_defs.get(class_name) {
            for id in ids {
                if let Some(node) = node_map.get_mut(id) {
                    node.node_style = Some(class_style.clone());
                }
            }
        }
    }

    // Apply inline style directives (override class styles)
    for (id, style) in &node_styles {
        if let Some(node) = node_map.get_mut(id) {
            let existing = node.node_style.get_or_insert_with(NodeStyle::default);
            if style.fill.is_some() {
                existing.fill = style.fill.clone();
            }
            if style.stroke.is_some() {
                existing.stroke = style.stroke.clone();
            }
            if style.color.is_some() {
                existing.color = style.color.clone();
            }
        }
    }

    // Apply link styles to edges
    for (idx, style) in &link_styles {
        if let Some(edge) = edges.get_mut(*idx) {
            edge.edge_style = Some(style.clone());
        }
    }

    let nodes: Vec<Node> = node_order.iter().map(|id| node_map[id].clone()).collect();

    Ok(FlowChart {
        direction,
        nodes,
        edges,
        subgraphs,
    })
}

// ---------------------------------------------------------------------------
// Direction line
// ---------------------------------------------------------------------------

fn parse_direction_line(line: &str) -> anyhow::Result<Direction> {
    // Accept "graph TD", "flowchart TD", etc.
    let mut parts = line.split_whitespace();
    let keyword = parts.next().unwrap_or("");
    if keyword != "graph" && keyword != "flowchart" {
        bail!("Expected 'graph' or 'flowchart', got {:?}", keyword);
    }
    let dir = parts.next().unwrap_or("");
    match dir {
        "TD" | "TB" => Ok(Direction::TopDown),
        "BT" => Ok(Direction::BottomTop),
        "LR" => Ok(Direction::LeftRight),
        "RL" => Ok(Direction::RightLeft),
        other => bail!("Unknown direction {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Statement parser — handles one logical line which may be a chain
//
// Grammar (simplified):
//   statement  = node_term (edge node_term)*
//   node_term  = ID [ shape_notation ]
//   edge       = '-->' | '---' | '-.->' | '==>' followed by optional '|label|'
// ---------------------------------------------------------------------------

fn parse_statement(
    line: &str,
    node_order: &mut Vec<String>,
    node_map: &mut HashMap<String, Node>,
    edges: &mut Vec<Edge>,
) -> anyhow::Result<()> {
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;

    // Parse first node term
    let mut prev_node = parse_node_term(&chars, &mut pos)?;
    register_node(&prev_node, node_order, node_map);

    // Loop: parse optional edge + next node term
    loop {
        skip_whitespace(&chars, &mut pos);
        if pos >= chars.len() {
            break;
        }

        // Try to parse an edge; if we can't, stop
        let (style, label) = match parse_edge(&chars, &mut pos) {
            Ok(e) => e,
            Err(_) => break,
        };

        skip_whitespace(&chars, &mut pos);
        let next_node = parse_node_term(&chars, &mut pos)?;
        register_node(&next_node, node_order, node_map);

        edges.push(Edge {
            from: prev_node.id.clone(),
            to: next_node.id.clone(),
            label,
            style,
            edge_style: None,
            er_meta: None,
        });

        prev_node = next_node;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Register a node, respecting that explicit labels override implicit ones
// ---------------------------------------------------------------------------

fn register_node(node: &Node, node_order: &mut Vec<String>, node_map: &mut HashMap<String, Node>) {
    let has_explicit_label = node.shape != NodeShape::Rect || node.label != node.id;

    if let Some(existing) = node_map.get_mut(&node.id) {
        // Only update if new definition carries an explicit label/shape
        if has_explicit_label {
            existing.label = node.label.clone();
            existing.shape = node.shape.clone();
        }
    } else {
        node_order.push(node.id.clone());
        node_map.insert(node.id.clone(), node.clone());
    }
}

// ---------------------------------------------------------------------------
// Node term: ID followed by optional shape brackets
// ---------------------------------------------------------------------------

fn parse_node_term(chars: &[char], pos: &mut usize) -> anyhow::Result<Node> {
    let id = parse_identifier(chars, pos)?;

    skip_whitespace(chars, pos);

    // Determine shape from the next character (if any)
    if *pos >= chars.len() {
        return Ok(Node {
            id: id.clone(),
            label: id,
            shape: NodeShape::Rect,
            node_style: None,
            entity: None,
        });
    }

    match chars[*pos] {
        '[' => {
            *pos += 1;
            let raw = collect_until(chars, pos, ']')?;
            Ok(Node {
                id,
                label: clean_label(&raw),
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            })
        }
        '(' => {
            // Could be `(label)` (rounded) or `((label))` (circle)
            *pos += 1;
            if *pos < chars.len() && chars[*pos] == '(' {
                // Circle: ((label))
                *pos += 1;
                let raw = collect_until(chars, pos, ')')?;
                // consume second ')'
                if *pos < chars.len() && chars[*pos] == ')' {
                    *pos += 1;
                }
                Ok(Node {
                    id,
                    label: clean_label(&raw),
                    shape: NodeShape::Circle,
                    node_style: None,
                    entity: None,
                })
            } else {
                // Rounded: (label)
                let raw = collect_until(chars, pos, ')')?;
                Ok(Node {
                    id,
                    label: clean_label(&raw),
                    shape: NodeShape::Rounded,
                    node_style: None,
                    entity: None,
                })
            }
        }
        '{' => {
            *pos += 1;
            let raw = collect_until(chars, pos, '}')?;
            Ok(Node {
                id,
                label: clean_label(&raw),
                shape: NodeShape::Diamond,
                node_style: None,
                entity: None,
            })
        }
        _ => {
            // Bare node — label equals id
            Ok(Node {
                id: id.clone(),
                label: id,
                shape: NodeShape::Rect,
                node_style: None,
                entity: None,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Edge parser — returns (EdgeStyle, Option<label>)
// Order of checks matters:
//   1. `-.->` before `---` (otherwise `---` matches start of `-.->`)
//   2. `-->` before `---`
//   3. `==>` (thick)
//   4. `---` (plain line)
// ---------------------------------------------------------------------------

fn parse_edge(chars: &[char], pos: &mut usize) -> anyhow::Result<(EdgeStyle, Option<String>)> {
    let remaining: String = chars[*pos..].iter().collect();

    let (style, consumed) = if remaining.starts_with("-.-") {
        // Dotted: `-.->` has arrowhead (4 chars); `-.-` alone has none (3 chars).
        // Distinguish by whether char 3 is `>`.
        let n = if remaining.chars().nth(3) == Some('>') {
            4
        } else {
            3
        };
        (EdgeStyle::Dotted, n)
    } else if let Some(stripped) = remaining.strip_prefix("-.") {
        // Extended dotted with embedded text: `-.label.->` (e.g. `-.failure.->`)
        if let Some(end_idx) = stripped.find(".->") {
            (EdgeStyle::Dotted, 2 + end_idx + 3)
        } else {
            bail!("No edge found at position {}", pos);
        }
    } else if remaining.starts_with("==>") {
        // Thick
        (EdgeStyle::Thick, 3)
    } else if remaining.starts_with("-->") {
        // Arrow
        (EdgeStyle::Arrow, 3)
    } else if remaining.starts_with("---") {
        // Plain line
        (EdgeStyle::Line, 3)
    } else {
        bail!("No edge found at position {}", pos);
    };

    *pos += consumed;

    // Optional label: |label| — strip surrounding quotes from label text
    skip_whitespace(chars, pos);
    let label = if *pos < chars.len() && chars[*pos] == '|' {
        *pos += 1;
        let raw = collect_until(chars, pos, '|')?;
        let cleaned = raw
            .trim()
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(raw.trim())
            .to_string();
        Some(cleaned)
    } else {
        None
    };

    Ok((style, label))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalize a raw label extracted from bracket notation.
///
/// Handles:
/// - Surrounding double quotes: `"text"` → `text`
/// - Cylinder shape wrapper: `("text")` → `text`
/// - HTML line breaks: `<br/>` / `<br>` → ` / `
fn clean_label(s: &str) -> String {
    let s = s.trim();
    // Strip outer double quotes
    let s = s
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s);
    // Unwrap cylinder notation ("inner") → inner, then strip quotes
    let s = if let Some(inner) = s.strip_prefix("(\"").and_then(|s| s.strip_suffix("\")")) {
        inner
    } else if let Some(inner) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        inner
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(inner)
    } else {
        s
    };
    // Replace HTML line-break tags with a visual separator
    let mut result = s.to_string();
    for br in &["<br/>", "<br>", "<BR/>", "<BR>"] {
        result = result.replace(br, " / ");
    }
    result.trim().to_string()
}

/// Parse a subgraph header: `ID["Label"]` or just `ID` → (id, label).
/// If no label is found, the id is used as the label.
fn parse_subgraph_header(rest: &str) -> (String, String) {
    // rest might be empty (anonymous subgraph)
    let rest = rest.trim();
    if rest.is_empty() {
        return (String::new(), String::new());
    }

    // Find the id (up to '[', '"', or whitespace)
    let id_end = rest
        .find(|c: char| c == '[' || c == '"' || c.is_whitespace())
        .unwrap_or(rest.len());
    let id = rest[..id_end].to_string();

    // Look for a label in brackets: ID["Label"] or ID[Label]
    let remainder = rest[id_end..].trim();
    let label = if let Some(inner) = remainder
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
    {
        clean_label(inner)
    } else if !remainder.is_empty() {
        // Fallback: treat the remainder as a label
        clean_label(remainder)
    } else {
        // No label: use the id
        id.clone()
    };

    (id, label)
}

fn parse_identifier(chars: &[char], pos: &mut usize) -> anyhow::Result<String> {
    let start = *pos;
    while *pos < chars.len() {
        let c = chars[*pos];
        if c.is_alphanumeric() || c == '_' {
            *pos += 1;
        } else {
            break;
        }
    }
    if *pos == start {
        bail!("Expected identifier at position {}", start);
    }
    Ok(chars[start..*pos].iter().collect())
}

fn collect_until(chars: &[char], pos: &mut usize, end: char) -> anyhow::Result<String> {
    let start = *pos;
    while *pos < chars.len() && chars[*pos] != end {
        *pos += 1;
    }
    let text: String = chars[start..*pos].iter().collect();
    if *pos < chars.len() {
        *pos += 1; // consume the end character
    }
    Ok(text)
}

fn skip_whitespace(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::{Direction, EdgeStyle, NodeShape};

    fn flowchart(s: &str) -> FlowChart {
        parse_flowchart(s).unwrap()
    }

    // -----------------------------------------------------------------------
    // Direction
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_direction_td() {
        let chart = flowchart("graph TD\n");
        assert_eq!(chart.direction, Direction::TopDown);
    }

    #[test]
    fn test_parse_direction_lr() {
        let chart = flowchart("graph LR\n");
        assert_eq!(chart.direction, Direction::LeftRight);
    }

    // -----------------------------------------------------------------------
    // Bare nodes
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_bare_nodes() {
        let chart = flowchart("graph TD\n    A\n    B\n");
        assert_eq!(chart.nodes.len(), 2);
        assert_eq!(chart.nodes[0].id, "A");
        assert_eq!(chart.nodes[0].label, "A");
        assert_eq!(chart.nodes[0].shape, NodeShape::Rect);
        assert_eq!(chart.nodes[1].id, "B");
    }

    // -----------------------------------------------------------------------
    // Node shapes
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_rect_node() {
        let chart = flowchart("graph TD\n    A[Hello World]\n");
        assert_eq!(chart.nodes[0].shape, NodeShape::Rect);
        assert_eq!(chart.nodes[0].label, "Hello World");
    }

    #[test]
    fn test_parse_rounded_node() {
        let chart = flowchart("graph TD\n    A(Rounded)\n");
        assert_eq!(chart.nodes[0].shape, NodeShape::Rounded);
        assert_eq!(chart.nodes[0].label, "Rounded");
    }

    #[test]
    fn test_parse_diamond_node() {
        let chart = flowchart("graph TD\n    A{Diamond}\n");
        assert_eq!(chart.nodes[0].shape, NodeShape::Diamond);
        assert_eq!(chart.nodes[0].label, "Diamond");
    }

    #[test]
    fn test_parse_circle_node() {
        let chart = flowchart("graph TD\n    A((Circle))\n");
        assert_eq!(chart.nodes[0].shape, NodeShape::Circle);
        assert_eq!(chart.nodes[0].label, "Circle");
    }

    // -----------------------------------------------------------------------
    // Edge styles
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_arrow_edge() {
        let chart = flowchart("graph TD\n    A --> B\n");
        assert_eq!(chart.edges.len(), 1);
        assert_eq!(chart.edges[0].style, EdgeStyle::Arrow);
        assert_eq!(chart.edges[0].from, "A");
        assert_eq!(chart.edges[0].to, "B");
    }

    #[test]
    fn test_parse_line_edge() {
        let chart = flowchart("graph TD\n    A --- B\n");
        assert_eq!(chart.edges[0].style, EdgeStyle::Line);
    }

    #[test]
    fn test_parse_dotted_edge() {
        let chart = flowchart("graph TD\n    A -.-> B\n");
        assert_eq!(chart.edges[0].style, EdgeStyle::Dotted);
    }

    #[test]
    fn test_parse_thick_edge() {
        let chart = flowchart("graph TD\n    A ==> B\n");
        assert_eq!(chart.edges[0].style, EdgeStyle::Thick);
    }

    // -----------------------------------------------------------------------
    // Edge with label
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_edge_with_label() {
        let chart = flowchart("graph TD\n    A -->|yes| B\n");
        assert_eq!(chart.edges[0].label, Some("yes".to_string()));
        assert_eq!(chart.edges[0].style, EdgeStyle::Arrow);
    }

    // -----------------------------------------------------------------------
    // Chain
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_chain() {
        let chart = flowchart("graph TD\n    A --> B --> C\n");
        assert_eq!(chart.edges.len(), 2);
        assert_eq!(chart.edges[0].from, "A");
        assert_eq!(chart.edges[0].to, "B");
        assert_eq!(chart.edges[1].from, "B");
        assert_eq!(chart.edges[1].to, "C");
    }

    // -----------------------------------------------------------------------
    // Full flowchart
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_full_flowchart() {
        let input = "graph TD\n    A[Start] --> B{Decision}\n    B -->|yes| C[Do thing]\n    B -->|no| D[Skip]\n    C --> D\n";
        let chart = flowchart(input);
        assert_eq!(chart.nodes.len(), 4);
        assert_eq!(chart.edges.len(), 4);
        assert_eq!(chart.direction, Direction::TopDown);
    }

    // -----------------------------------------------------------------------
    // Comments and empty lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_skips_comments_and_empty_lines() {
        let input =
            "graph TD\n%% this is a comment\n\n    A --> B\n%% another comment\n    B --> C\n";
        let chart = flowchart(input);
        assert_eq!(chart.nodes.len(), 3);
        assert_eq!(chart.edges.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Error: missing direction
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_invalid_no_direction() {
        let result = parse_flowchart("A --> B\n");
        assert!(result.is_err(), "Should fail without a graph declaration");
    }

    // -----------------------------------------------------------------------
    // Node deduplication
    // -----------------------------------------------------------------------

    #[test]
    fn test_node_defined_once_even_if_used_multiple_times() {
        let input = "graph TD\n    A --> B\n    B --> C\n    A --> C\n";
        let chart = flowchart(input);
        // A, B, C — each appears once
        assert_eq!(chart.nodes.len(), 3);
        let ids: Vec<&str> = chart.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["A", "B", "C"]);
    }

    // -----------------------------------------------------------------------
    // Style directives
    // -----------------------------------------------------------------------

    use crate::render::Color;

    #[test]
    fn test_parse_style_directive() {
        let input = "graph TD\n    A[Start]\n    style A fill:#ff9900,stroke:#333,color:#fff\n";
        let chart = flowchart(input);
        assert_eq!(chart.nodes.len(), 1);
        let node = &chart.nodes[0];
        let style = node.node_style.as_ref().expect("node should have style");
        assert_eq!(style.fill, Some(Color::Rgb(255, 153, 0)));
        assert_eq!(style.stroke, Some(Color::Rgb(51, 51, 51)));
        assert_eq!(style.color, Some(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn test_parse_classdef_and_class() {
        let input = "graph TD\n    A[Start]\n    B[End]\n    classDef highlight fill:#f9f,stroke:#333\n    class A,B highlight\n";
        let chart = flowchart(input);
        let a_style = chart.nodes[0]
            .node_style
            .as_ref()
            .expect("A should have style");
        let b_style = chart.nodes[1]
            .node_style
            .as_ref()
            .expect("B should have style");
        assert_eq!(a_style.fill, Some(Color::Rgb(255, 153, 255)));
        assert_eq!(b_style.fill, Some(Color::Rgb(255, 153, 255)));
    }

    #[test]
    fn test_parse_linkstyle() {
        let input = "graph TD\n    A --> B\n    B --> C\n    linkStyle 0 stroke:#ff3\n";
        let chart = flowchart(input);
        let edge0_style = chart.edges[0]
            .edge_style
            .as_ref()
            .expect("edge 0 should have style");
        assert_eq!(edge0_style.stroke, Some(Color::Rgb(255, 255, 51)));
        assert!(chart.edges[1].edge_style.is_none());
    }

    #[test]
    fn test_style_directive_invalid_color_ignored() {
        let input = "graph TD\n    A[Start]\n    style A fill:notacolor\n";
        let chart = flowchart(input);
        let style = chart.nodes[0]
            .node_style
            .as_ref()
            .expect("node should have style");
        assert_eq!(style.fill, None);
    }

    // -----------------------------------------------------------------------
    // Quoted labels
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_quoted_rect_label() {
        let chart = flowchart("graph LR\n    A[\"Domain Modules\"]\n");
        assert_eq!(chart.nodes[0].label, "Domain Modules");
    }

    #[test]
    fn test_parse_br_in_label() {
        let chart = flowchart("graph LR\n    A[\"workerA()<br/>workerB()\"]\n");
        assert_eq!(chart.nodes[0].label, "workerA() / workerB()");
    }

    #[test]
    fn test_parse_cylinder_shape_label() {
        let chart = flowchart("graph LR\n    DB[(\"Database\")]\n");
        assert_eq!(chart.nodes[0].label, "Database");
    }

    // -----------------------------------------------------------------------
    // Extended dotted edges
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_extended_dotted_edge() {
        let chart = flowchart("graph LR\n    A -.failure.-> B\n");
        assert_eq!(chart.edges[0].style, EdgeStyle::Dotted);
        assert_eq!(chart.edges[0].from, "A");
        assert_eq!(chart.edges[0].to, "B");
    }

    #[test]
    fn test_parse_extended_dotted_edge_short_label() {
        let chart = flowchart("graph LR\n    A -.x.-> B\n");
        assert_eq!(chart.edges[0].style, EdgeStyle::Dotted);
    }

    // -----------------------------------------------------------------------
    // Quoted edge labels
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_quoted_edge_label() {
        let chart = flowchart("graph LR\n    A -->|\"publish event\"| B\n");
        assert_eq!(chart.edges[0].label, Some("publish event".to_string()));
    }

    // -----------------------------------------------------------------------
    // Subgraph / end skipping
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_subgraph_skipped() {
        let input = "graph LR\n    A[Source]\n    subgraph Broker[\"Broker\"]\n        B[Queue]\n    end\n    A --> B\n";
        let chart = flowchart(input);
        // Only real nodes — "subgraph" and "end" must not appear
        let ids: Vec<&str> = chart.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(!ids.contains(&"subgraph"), "subgraph should not be a node");
        assert!(!ids.contains(&"end"), "end should not be a node");
        assert!(ids.contains(&"A"));
        assert!(ids.contains(&"B"));
        assert_eq!(chart.edges.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Subgraph membership tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_subgraph_membership_single() {
        let input = "graph LR\n    A[Source]\n    subgraph Broker[\"Message Broker\"]\n        B[Queue]\n        C[Router]\n    end\n    A --> B\n";
        let chart = flowchart(input);
        assert_eq!(chart.subgraphs.len(), 1);
        let sg = &chart.subgraphs[0];
        assert_eq!(sg.id, "Broker");
        assert_eq!(sg.label, "Message Broker");
        assert!(sg.node_ids.contains(&"B".to_string()));
        assert!(sg.node_ids.contains(&"C".to_string()));
        assert!(!sg.node_ids.contains(&"A".to_string()));
    }

    #[test]
    fn test_parse_subgraph_membership_multiple() {
        let input = "flowchart LR\n    subgraph G1[\"Group One\"]\n        N1[Alpha]\n        N2[Beta]\n    end\n    subgraph G2[\"Group Two\"]\n        N3[Gamma]\n    end\n    N1 --> N3\n";
        let chart = flowchart(input);
        assert_eq!(chart.subgraphs.len(), 2);
        let g1 = chart.subgraphs.iter().find(|s| s.id == "G1").unwrap();
        let g2 = chart.subgraphs.iter().find(|s| s.id == "G2").unwrap();
        assert_eq!(g1.label, "Group One");
        assert_eq!(g1.node_ids, vec!["N1".to_string(), "N2".to_string()]);
        assert_eq!(g2.label, "Group Two");
        assert_eq!(g2.node_ids, vec!["N3".to_string()]);
    }

    #[test]
    fn test_parse_subgraph_no_subgraphs() {
        let input = "graph TD\n    A --> B\n";
        let chart = flowchart(input);
        assert!(chart.subgraphs.is_empty());
    }

    #[test]
    fn test_parse_subgraph_edge_nodes_not_in_subgraph() {
        // Nodes added by edges OUTSIDE subgraph should NOT be in the subgraph
        let input =
            "graph LR\n    subgraph Broker[\"Broker\"]\n        B[Queue]\n    end\n    A --> B\n";
        let chart = flowchart(input);
        assert_eq!(chart.subgraphs.len(), 1);
        let sg = &chart.subgraphs[0];
        // A is added outside the subgraph
        assert!(!sg.node_ids.contains(&"A".to_string()));
        assert!(sg.node_ids.contains(&"B".to_string()));
    }

    #[test]
    fn test_parse_subgraph_membership_edges_before_subgraph() {
        // Nodes created by edges BEFORE the subgraph block must still be
        // assigned to the subgraph when listed inside it.
        let input = "flowchart LR\n    A --> B\n    B --> C\n    subgraph SG1[\"Group 1\"]\n        A\n        B\n    end\n    subgraph SG2[\"Group 2\"]\n        C\n    end\n";
        let chart = flowchart(input);
        assert_eq!(chart.subgraphs.len(), 2);
        let sg1 = chart.subgraphs.iter().find(|s| s.id == "SG1").unwrap();
        let sg2 = chart.subgraphs.iter().find(|s| s.id == "SG2").unwrap();
        assert!(sg1.node_ids.contains(&"A".to_string()), "A must be in SG1");
        assert!(sg1.node_ids.contains(&"B".to_string()), "B must be in SG1");
        assert!(sg2.node_ids.contains(&"C".to_string()), "C must be in SG2");
        assert!(
            !sg2.node_ids.contains(&"A".to_string()),
            "A must not be in SG2"
        );
    }
}
