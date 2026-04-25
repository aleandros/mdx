use anyhow::{Result, bail};

use super::{Cardinality, Entity, ErDiagram, Relationship};
use crate::mermaid::Direction;

/// Returns (left_card, right_card, identifying, after_op_idx) on success,
/// or None if no valid relationship operator is found.
fn try_parse_relationship_op(s: &str) -> Option<(Cardinality, Cardinality, bool, usize)> {
    const LEFT_TOKENS: &[(&str, Cardinality)] = &[
        ("||", Cardinality::ExactlyOne),
        ("o|", Cardinality::ZeroOrOne),
        ("}o", Cardinality::ZeroOrMany),
        ("}|", Cardinality::OneOrMany),
    ];
    const RIGHT_TOKENS: &[(&str, Cardinality)] = &[
        ("||", Cardinality::ExactlyOne),
        ("|o", Cardinality::ZeroOrOne),
        ("o{", Cardinality::ZeroOrMany),
        ("|{", Cardinality::OneOrMany),
    ];

    for (lt, lc) in LEFT_TOKENS {
        if !s.starts_with(lt) {
            continue;
        }
        let after_l = &s[lt.len()..];
        for (op, identifying) in [("--", true), ("..", false)] {
            if !after_l.starts_with(op) {
                continue;
            }
            let after_op = &after_l[op.len()..];
            for (rt, rc) in RIGHT_TOKENS {
                if after_op.starts_with(rt) {
                    let consumed = lt.len() + op.len() + rt.len();
                    return Some((*lc, *rc, identifying, consumed));
                }
            }
        }
    }
    None
}

fn parse_relationship_line(line: &str) -> Result<Option<Relationship>> {
    let line = line.trim();
    let Some((left, rest)) = line.split_once(char::is_whitespace) else {
        return Ok(None);
    };
    let rest = rest.trim_start();

    let Some((lc, rc, identifying, consumed)) = try_parse_relationship_op(rest) else {
        return Ok(None);
    };
    let after_op = rest[consumed..].trim_start();

    let (right, label_part) = match after_op.split_once(char::is_whitespace) {
        Some(p) => p,
        None => (after_op, ""),
    };
    if right.is_empty() {
        bail!("Relationship missing right entity: `{}`", line);
    }

    let label = {
        let lp = label_part.trim_start();
        if let Some(after_colon) = lp.strip_prefix(':') {
            let raw = after_colon.trim();
            if raw.is_empty() {
                None
            } else if let Some(quoted) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
                Some(quoted.to_string())
            } else {
                Some(raw.to_string())
            }
        } else {
            None
        }
    };

    Ok(Some(Relationship {
        left: left.to_string(),
        right: right.to_string(),
        left_card: lc,
        right_card: rc,
        identifying,
        label,
    }))
}

pub fn parse_er(input: &str) -> Result<ErDiagram> {
    let mut lines = input.lines().peekable();

    // Header line.
    loop {
        match lines.next() {
            None => bail!("Empty input: expected `erDiagram` header"),
            Some(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("%%") {
                    continue;
                }
                if trimmed != "erDiagram" {
                    bail!("Expected `erDiagram` header, got `{}`", trimmed);
                }
                break;
            }
        }
    }

    let mut direction = Direction::TopDown;
    let mut direction_explicit = false;
    let mut entity_order: Vec<String> = Vec::new();
    let mut entities: std::collections::HashMap<String, Entity> = std::collections::HashMap::new();
    let mut relationships: Vec<Relationship> = Vec::new();

    fn ensure_entity(
        name: &str,
        entity_order: &mut Vec<String>,
        entities: &mut std::collections::HashMap<String, Entity>,
    ) {
        if !entities.contains_key(name) {
            entity_order.push(name.to_string());
            entities.insert(
                name.to_string(),
                Entity {
                    name: name.to_string(),
                    attributes: Vec::new(),
                    rendered_lines: Vec::new(),
                    width: 0,
                    height: 0,
                },
            );
        }
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("direction ") {
            direction = match rest.trim() {
                "TD" | "TB" => Direction::TopDown,
                "BT" => Direction::BottomTop,
                "LR" => Direction::LeftRight,
                "RL" => Direction::RightLeft,
                other => bail!("Unknown direction: `{}`", other),
            };
            direction_explicit = true;
            continue;
        }
        if let Some(rel) = parse_relationship_line(trimmed)? {
            ensure_entity(&rel.left, &mut entity_order, &mut entities);
            ensure_entity(&rel.right, &mut entity_order, &mut entities);
            relationships.push(rel);
            continue;
        }

        // A line containing `--` or `..` that didn't parse as a relationship is an error.
        if trimmed.contains("--") || trimmed.contains("..") {
            bail!("Unrecognized cardinality token: `{}`", trimmed);
        }

        // Other lines: ignored. Entity blocks land in Task 6.
    }

    let entities_vec: Vec<Entity> = entity_order
        .into_iter()
        .map(|n| entities.remove(&n).unwrap())
        .collect();

    Ok(ErDiagram {
        direction,
        direction_explicit,
        entities: entities_vec,
        relationships,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::Direction;
    use crate::mermaid::er::Cardinality;

    #[test]
    fn test_parse_empty_diagram() {
        let d = parse_er("erDiagram\n").unwrap();
        assert_eq!(d.direction, Direction::TopDown);
        assert!(d.entities.is_empty());
        assert!(d.relationships.is_empty());
    }

    #[test]
    fn test_parse_explicit_direction_lr() {
        let d = parse_er("erDiagram\n    direction LR\n").unwrap();
        assert_eq!(d.direction, Direction::LeftRight);
        assert!(d.direction_explicit);
    }

    #[test]
    fn test_parse_explicit_direction_td() {
        let d = parse_er("erDiagram\n    direction TD\n").unwrap();
        assert_eq!(d.direction, Direction::TopDown);
        assert!(d.direction_explicit);
    }

    #[test]
    fn test_parse_default_direction_is_not_explicit() {
        let d = parse_er("erDiagram\n").unwrap();
        assert!(!d.direction_explicit);
    }

    #[test]
    fn test_parse_skips_comments_before_header() {
        let d = parse_er("%% comment\n\nerDiagram\n").unwrap();
        assert!(d.entities.is_empty());
    }

    #[test]
    fn test_parse_missing_header_errors() {
        let err = parse_er("graph TD\n").unwrap_err();
        assert!(err.to_string().contains("erDiagram"));
    }

    #[test]
    fn test_parse_relationship_one_to_many_identifying() {
        let d = parse_er("erDiagram\n    A ||--o{ B : has\n").unwrap();
        assert_eq!(d.relationships.len(), 1);
        let r = &d.relationships[0];
        assert_eq!(r.left, "A");
        assert_eq!(r.right, "B");
        assert_eq!(r.left_card, Cardinality::ExactlyOne);
        assert_eq!(r.right_card, Cardinality::ZeroOrMany);
        assert!(r.identifying);
        assert_eq!(r.label.as_deref(), Some("has"));
    }

    #[test]
    fn test_parse_relationship_non_identifying() {
        let d = parse_er("erDiagram\n    A ||..o{ B : maybe\n").unwrap();
        assert!(!d.relationships[0].identifying);
    }

    #[test]
    fn test_parse_relationship_quoted_label() {
        let d = parse_er("erDiagram\n    A ||--|{ B : \"per-user pref\"\n").unwrap();
        assert_eq!(d.relationships[0].label.as_deref(), Some("per-user pref"));
    }

    #[test]
    fn test_parse_relationship_no_label() {
        let d = parse_er("erDiagram\n    A }o--o{ B\n").unwrap();
        assert_eq!(d.relationships[0].label, None);
        assert_eq!(d.relationships[0].left_card, Cardinality::ZeroOrMany);
        assert_eq!(d.relationships[0].right_card, Cardinality::ZeroOrMany);
    }

    #[test]
    fn test_parse_all_cardinality_tokens() {
        let pairs = [
            ("||", "||", Cardinality::ExactlyOne, Cardinality::ExactlyOne),
            ("o|", "|o", Cardinality::ZeroOrOne, Cardinality::ZeroOrOne),
            ("}o", "o{", Cardinality::ZeroOrMany, Cardinality::ZeroOrMany),
            ("}|", "|{", Cardinality::OneOrMany, Cardinality::OneOrMany),
        ];
        for (l, r, lc, rc) in pairs {
            let src = format!("erDiagram\n    A {l}--{r} B\n");
            let d = parse_er(&src).unwrap();
            assert_eq!(d.relationships[0].left_card, lc, "left {l}");
            assert_eq!(d.relationships[0].right_card, rc, "right {r}");
        }
    }

    #[test]
    fn test_parse_relationship_creates_implicit_entities() {
        let d = parse_er("erDiagram\n    A ||--o{ B : x\n").unwrap();
        let names: Vec<&str> = d.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"A"));
        assert!(names.contains(&"B"));
    }

    #[test]
    fn test_parse_unknown_cardinality_token_errors() {
        let err = parse_er("erDiagram\n    A xx--yy B\n").unwrap_err();
        assert!(err.to_string().contains("cardinality") || err.to_string().contains("xx"));
    }
}
