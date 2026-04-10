use std::collections::HashMap;

use anyhow::{bail, Context};

use super::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape};

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

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }
        parse_statement(trimmed, &mut node_order, &mut node_map, &mut edges)
            .with_context(|| format!("Failed to parse line: {:?}", trimmed))?;
    }

    let nodes: Vec<Node> = node_order
        .iter()
        .map(|id| node_map[id].clone())
        .collect();

    Ok(FlowChart { direction, nodes, edges })
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
        });

        prev_node = next_node;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Register a node, respecting that explicit labels override implicit ones
// ---------------------------------------------------------------------------

fn register_node(
    node: &Node,
    node_order: &mut Vec<String>,
    node_map: &mut HashMap<String, Node>,
) {
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
        return Ok(Node { id: id.clone(), label: id, shape: NodeShape::Rect });
    }

    match chars[*pos] {
        '[' => {
            *pos += 1;
            let label = collect_until(chars, pos, ']')?;
            Ok(Node { id, label, shape: NodeShape::Rect })
        }
        '(' => {
            // Could be `(label)` (rounded) or `((label))` (circle)
            *pos += 1;
            if *pos < chars.len() && chars[*pos] == '(' {
                // Circle: ((label))
                *pos += 1;
                let label = collect_until(chars, pos, ')')?;
                // consume second ')'
                if *pos < chars.len() && chars[*pos] == ')' {
                    *pos += 1;
                }
                Ok(Node { id, label, shape: NodeShape::Circle })
            } else {
                // Rounded: (label)
                let label = collect_until(chars, pos, ')')?;
                Ok(Node { id, label, shape: NodeShape::Rounded })
            }
        }
        '{' => {
            *pos += 1;
            let label = collect_until(chars, pos, '}')?;
            Ok(Node { id, label, shape: NodeShape::Diamond })
        }
        _ => {
            // Bare node — label equals id
            Ok(Node { id: id.clone(), label: id, shape: NodeShape::Rect })
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
        // Dotted: `-.->` (consume 4 chars)
        (EdgeStyle::Dotted, 4)
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

    // Optional label: |label|
    skip_whitespace(chars, pos);
    let label = if *pos < chars.len() && chars[*pos] == '|' {
        *pos += 1;
        let lbl = collect_until(chars, pos, '|')?;
        Some(lbl)
    } else {
        None
    };

    Ok((style, label))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
        let input = "graph TD\n%% this is a comment\n\n    A --> B\n%% another comment\n    B --> C\n";
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
}
