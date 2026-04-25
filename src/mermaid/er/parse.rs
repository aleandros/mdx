use anyhow::{Result, bail};

use super::ErDiagram;
use crate::mermaid::Direction;

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
        // Other lines parsed in later tasks.
    }

    Ok(ErDiagram {
        direction,
        direction_explicit,
        entities: Vec::new(),
        relationships: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::Direction;

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
}
