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

fn parse_entity_opener(line: &str) -> Option<String> {
    // Matches `Name {` or `Name{` (after trimming).
    let line = line.trim_end();
    let line = line.strip_suffix('{')?;
    let name = line.trim();
    if name.is_empty() {
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(name.to_string())
}

fn parse_attribute_line(line: &str) -> Result<super::Attribute> {
    // Format: TYPE NAME [PK | FK | PK,FK | FK,PK] ["comment"]
    let mut rest = line.trim();
    let comment = if let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let close = after
            .find('"')
            .ok_or_else(|| anyhow::anyhow!("Unterminated comment in attribute: `{}`", line))?;
        let c = after[..close].to_string();
        rest = rest[..open].trim();
        Some(c)
    } else {
        None
    };

    let mut parts = rest.split_whitespace();
    let ty = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Empty attribute line"))?
        .to_string();
    let name = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Attribute missing name: `{}`", line))?
        .to_string();
    let key_token = parts.next();
    if parts.next().is_some() {
        bail!("Unexpected extra tokens in attribute: `{}`", line);
    }
    let key = match key_token {
        None => super::KeyKind::None,
        Some("PK") => super::KeyKind::Pk,
        Some("FK") => super::KeyKind::Fk,
        Some("PK,FK") | Some("FK,PK") => super::KeyKind::PkFk,
        Some(other) => bail!("Unknown key marker: `{}`", other),
    };
    Ok(super::Attribute {
        ty,
        name,
        key,
        comment,
    })
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
    let mut node_styles: Vec<(String, crate::mermaid::NodeStyle)> = Vec::new();
    let mut class_defs: std::collections::HashMap<String, crate::mermaid::NodeStyle> =
        std::collections::HashMap::new();
    let mut class_assignments: Vec<(Vec<String>, String)> = Vec::new();

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
                    node_style: None,
                },
            );
        }
    }

    let body_lines: Vec<&str> = lines.collect();
    let mut i = 0;
    while i < body_lines.len() {
        let trimmed = body_lines[i].trim();
        i += 1;
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
        if let Some(rest) = trimmed.strip_prefix("style ") {
            if let Some((id, props)) = rest.split_once(char::is_whitespace) {
                let style = crate::mermaid::color::parse_node_style_props(props.trim());
                node_styles.push((id.trim().to_string(), style));
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("classDef ") {
            if let Some((name, props)) = rest.split_once(char::is_whitespace) {
                let style = crate::mermaid::color::parse_node_style_props(props.trim());
                class_defs.insert(name.trim().to_string(), style);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some((ids_str, class_name)) = rest.rsplit_once(char::is_whitespace) {
                let ids: Vec<String> = ids_str.split(',').map(|s| s.trim().to_string()).collect();
                class_assignments.push((ids, class_name.trim().to_string()));
            }
            continue;
        }
        if let Some(rel) = parse_relationship_line(trimmed)? {
            ensure_entity(&rel.left, &mut entity_order, &mut entities);
            ensure_entity(&rel.right, &mut entity_order, &mut entities);
            relationships.push(rel);
            continue;
        }
        if let Some(name) = parse_entity_opener(trimmed) {
            let mut attrs: Vec<super::Attribute> = Vec::new();
            let mut closed = false;
            while i < body_lines.len() {
                let inner = body_lines[i].trim();
                i += 1;
                if inner.is_empty() || inner.starts_with("%%") {
                    continue;
                }
                if inner == "}" {
                    closed = true;
                    break;
                }
                let attr = parse_attribute_line(inner)?;
                attrs.push(attr);
            }
            if !closed {
                bail!("Unclosed entity block for `{}`", name);
            }
            ensure_entity(&name, &mut entity_order, &mut entities);
            let e = entities.get_mut(&name).unwrap();
            e.attributes = attrs;
            continue;
        }

        // A line containing `--` or `..` that didn't parse as a relationship is an error.
        if trimmed.contains("--") || trimmed.contains("..") {
            bail!("Unrecognized cardinality token: `{}`", trimmed);
        }

        // Other lines: silently ignored to match Mermaid's leniency.
    }

    // Apply class assignments first (so explicit `style` lines override class).
    for (entity_ids, cls) in &class_assignments {
        if let Some(style) = class_defs.get(cls) {
            for id in entity_ids {
                ensure_entity(id, &mut entity_order, &mut entities);
                let e = entities.get_mut(id).unwrap();
                e.node_style = Some(style.clone());
            }
        }
        // Unknown class: silently ignored.
    }
    // Explicit `style` lines (already accumulated by T2; replaces class-applied
    // style on overlapping entities).
    for (id, style) in &node_styles {
        ensure_entity(id, &mut entity_order, &mut entities);
        let e = entities.get_mut(id).unwrap();
        e.node_style = Some(style.clone());
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
    use crate::mermaid::er::KeyKind;

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

    #[test]
    fn test_parse_entity_block_basic() {
        let src = "erDiagram\n  Foo {\n    string id\n    int count\n  }\n";
        let d = parse_er(src).unwrap();
        assert_eq!(d.entities.len(), 1);
        let e = &d.entities[0];
        assert_eq!(e.name, "Foo");
        assert_eq!(e.attributes.len(), 2);
        assert_eq!(e.attributes[0].ty, "string");
        assert_eq!(e.attributes[0].name, "id");
        assert_eq!(e.attributes[0].key, KeyKind::None);
        assert_eq!(e.attributes[1].ty, "int");
        assert_eq!(e.attributes[1].name, "count");
    }

    #[test]
    fn test_parse_entity_block_pk_fk() {
        let src = "erDiagram\n  Foo {\n    string id PK\n    string parentId FK\n  }\n";
        let d = parse_er(src).unwrap();
        assert_eq!(d.entities[0].attributes[0].key, KeyKind::Pk);
        assert_eq!(d.entities[0].attributes[1].key, KeyKind::Fk);
    }

    #[test]
    fn test_parse_entity_block_pk_and_fk() {
        let src = "erDiagram\n  Foo {\n    string id PK,FK\n  }\n";
        let d = parse_er(src).unwrap();
        assert_eq!(d.entities[0].attributes[0].key, KeyKind::PkFk);
    }

    #[test]
    fn test_parse_unclosed_entity_block_errors() {
        let src = "erDiagram\n  Foo {\n    string id\n";
        let err = parse_er(src).unwrap_err();
        assert!(err.to_string().contains("Foo") || err.to_string().contains("nclosed"));
    }

    #[test]
    fn test_parse_entity_block_then_relationship() {
        let src = "erDiagram\n  Foo {\n    string id\n  }\n  Foo ||--o{ Bar : has\n";
        let d = parse_er(src).unwrap();
        assert_eq!(d.entities.len(), 2);
        assert_eq!(d.entities[0].name, "Foo");
        assert_eq!(d.entities[0].attributes.len(), 1);
        assert_eq!(d.entities[1].name, "Bar");
        assert_eq!(d.entities[1].attributes.len(), 0);
        assert_eq!(d.relationships.len(), 1);
    }

    #[test]
    fn test_parse_attribute_with_comment() {
        let src = "erDiagram\n  Foo {\n    string name \"the slug, lowercase\"\n  }\n";
        let d = parse_er(src).unwrap();
        let a = &d.entities[0].attributes[0];
        assert_eq!(a.ty, "string");
        assert_eq!(a.name, "name");
        assert_eq!(a.comment.as_deref(), Some("the slug, lowercase"));
    }

    #[test]
    fn test_parse_attribute_pk_with_comment() {
        let src = "erDiagram\n  Foo {\n    string id PK \"primary key\"\n  }\n";
        let d = parse_er(src).unwrap();
        let a = &d.entities[0].attributes[0];
        assert_eq!(a.key, KeyKind::Pk);
        assert_eq!(a.comment.as_deref(), Some("primary key"));
    }

    #[test]
    fn test_parse_attribute_unterminated_comment_errors() {
        let src = "erDiagram\n  Foo {\n    string id PK \"oops\n  }\n";
        let err = parse_er(src).unwrap_err();
        assert!(
            err.to_string().contains("Unterminated") || err.to_string().contains("comment"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_parse_style_directive() {
        use crate::mermaid::NodeStyle;
        use crate::render::Color;
        let _ = NodeStyle::default(); // suppress unused if NodeStyle isn't otherwise referenced
        let src = "erDiagram\n  Foo {\n    string id\n  }\n  style Foo fill:#ff0000,stroke:#00ff00,color:#0000ff\n";
        let d = parse_er(src).unwrap();
        let style = d.entities[0].node_style.as_ref().unwrap();
        assert_eq!(style.fill, Some(Color::Rgb(255, 0, 0)));
        assert_eq!(style.stroke, Some(Color::Rgb(0, 255, 0)));
        assert_eq!(style.color, Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_parse_style_on_implicit_entity() {
        use crate::render::Color;
        // Entity referenced only by relationship + style line; no { } block.
        let src = "erDiagram\n  A ||--o{ B : has\n  style A stroke:#ff0000\n";
        let d = parse_er(src).unwrap();
        let a = d.entities.iter().find(|e| e.name == "A").unwrap();
        assert_eq!(
            a.node_style.as_ref().unwrap().stroke,
            Some(Color::Rgb(255, 0, 0))
        );
    }

    #[test]
    fn test_parse_class_def_and_assignment() {
        use crate::render::Color;
        let src =
            "erDiagram\n  A {\n    string id\n  }\n  classDef bar fill:#ff00ff\n  class A bar\n";
        let d = parse_er(src).unwrap();
        let a = &d.entities[0];
        assert_eq!(
            a.node_style.as_ref().unwrap().fill,
            Some(Color::Rgb(255, 0, 255))
        );
    }

    #[test]
    fn test_parse_class_assignment_multiple_entities() {
        use crate::render::Color;
        let src = "erDiagram\n  A {\n    string id\n  }\n  B {\n    string id\n  }\n  C {\n    string id\n  }\n  classDef bar fill:#ff00ff\n  class A,B,C bar\n";
        let d = parse_er(src).unwrap();
        for entity in &d.entities {
            assert_eq!(
                entity.node_style.as_ref().unwrap().fill,
                Some(Color::Rgb(255, 0, 255)),
                "entity {} missing class fill",
                entity.name
            );
        }
    }

    #[test]
    fn test_parse_style_overrides_class() {
        use crate::render::Color;
        // class sets fill, then explicit style sets stroke. Style line REPLACES the
        // class-applied style on the entity (matches flowchart's apply order: class
        // first, style second). Final node_style has only stroke set.
        let src = "erDiagram\n  A {\n    string id\n  }\n  classDef bar fill:#ff00ff\n  class A bar\n  style A stroke:#00ff00\n";
        let d = parse_er(src).unwrap();
        let a = &d.entities[0];
        let style = a.node_style.as_ref().unwrap();
        assert_eq!(style.stroke, Some(Color::Rgb(0, 255, 0)));
        assert_eq!(style.fill, None);
    }

    #[test]
    fn test_parse_unknown_class_silently_ignored() {
        let src = "erDiagram\n  A {\n    string id\n  }\n  class A nonexistent\n";
        let d = parse_er(src).unwrap();
        assert!(d.entities[0].node_style.is_none());
    }
}
