use anyhow::bail;

use super::{
    ArrowStyle, Event, FragmentKind, FragmentSection, NotePosition, Participant, SequenceDiagram,
};
use crate::mermaid::color::parse_color;
use crate::mermaid::{MermaidEdgeStyle, NodeStyle};

pub fn parse_sequence(input: &str) -> anyhow::Result<SequenceDiagram> {
    let mut lines = input.lines().peekable();

    // Find and validate the header
    let found_header = loop {
        match lines.next() {
            None => break false,
            Some(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("%%") {
                    continue;
                }
                break trimmed == "sequenceDiagram";
            }
        }
    };

    if !found_header {
        bail!("Expected 'sequenceDiagram' declaration");
    }

    let mut participants: Vec<Participant> = vec![];
    let mut autonumber = false;

    // Stack for nested fragments. Each entry is (kind, label, sections).
    // sections is a Vec<FragmentSection> where the last section is "current".
    let mut fragment_stack: Vec<(FragmentKind, String, Vec<FragmentSection>)> = vec![];

    // Top-level events list
    let mut top_events: Vec<Event> = vec![];

    // Style directive accumulators
    let mut class_defs: std::collections::HashMap<String, NodeStyle> =
        std::collections::HashMap::new();
    let mut class_assignments: Vec<(Vec<String>, String)> = Vec::new();
    let mut node_styles: Vec<(String, NodeStyle)> = Vec::new();
    let mut link_styles: Vec<(usize, MermaidEdgeStyle)> = Vec::new();

    // Push an event to the current scope (top-level or innermost fragment section)
    macro_rules! push_event {
        ($event:expr) => {
            if let Some((_, _, sections)) = fragment_stack.last_mut() {
                sections.last_mut().unwrap().events.push($event);
            } else {
                top_events.push($event);
            }
        };
    }

    for raw_line in lines {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        // participant / actor
        if let Some(rest) = line
            .strip_prefix("participant ")
            .or_else(|| line.strip_prefix("actor "))
        {
            let rest = rest.trim();
            if let Some((id, label)) = rest.split_once(" as ") {
                let id = id.trim().to_string();
                let label = label.trim().to_string();
                if !participants.iter().any(|p| p.id == id) {
                    participants.push(Participant {
                        id,
                        label,
                        style: None,
                    });
                }
            } else {
                let id = rest.to_string();
                if !participants.iter().any(|p| p.id == id) {
                    participants.push(Participant {
                        id: id.clone(),
                        label: id,
                        style: None,
                    });
                }
            }
            continue;
        }

        // autonumber
        if line == "autonumber" {
            autonumber = true;
            continue;
        }

        // activate / deactivate
        if let Some(rest) = line.strip_prefix("activate ") {
            let participant = rest.trim().to_string();
            push_event!(Event::Activate { participant });
            continue;
        }
        if let Some(rest) = line.strip_prefix("deactivate ") {
            let participant = rest.trim().to_string();
            push_event!(Event::Deactivate { participant });
            continue;
        }

        // Note right of / left of / over
        if let Some(event) = line.strip_prefix("Note ").and_then(parse_note) {
            push_event!(event);
            continue;
        }

        // Fragment openers: loop, alt, opt, par
        if let Some(kind) = parse_fragment_opener(line) {
            let label = parse_fragment_label(line);
            let first_section = FragmentSection {
                label: None,
                events: vec![],
            };
            fragment_stack.push((kind, label, vec![first_section]));
            continue;
        }

        // else / and — new section in current fragment
        if line == "else" || line.starts_with("else ") {
            let section_label = if line == "else" {
                None
            } else {
                Some(line["else ".len()..].trim().to_string())
            };
            if let Some((_, _, sections)) = fragment_stack.last_mut() {
                sections.push(FragmentSection {
                    label: section_label,
                    events: vec![],
                });
            }
            continue;
        }
        if line == "and" || line.starts_with("and ") {
            let section_label = if line == "and" {
                None
            } else {
                Some(line["and ".len()..].trim().to_string())
            };
            if let Some((_, _, sections)) = fragment_stack.last_mut() {
                sections.push(FragmentSection {
                    label: section_label,
                    events: vec![],
                });
            }
            continue;
        }

        // end — pop fragment
        if line == "end" {
            if let Some((kind, label, sections)) = fragment_stack.pop() {
                let event = Event::Fragment {
                    kind,
                    label,
                    sections,
                };
                push_event!(event);
            }
            continue;
        }

        // style ParticipantId fill:#f9f,stroke:#333
        if let Some(rest) = line.strip_prefix("style ") {
            if let Some((id, props)) = rest.split_once(' ') {
                let style = parse_node_style_props(props);
                node_styles.push((id.trim().to_string(), style));
            }
            continue;
        }

        // classDef className fill:#f9f
        if let Some(rest) = line.strip_prefix("classDef ") {
            if let Some((name, props)) = rest.split_once(' ') {
                let style = parse_node_style_props(props);
                class_defs.insert(name.trim().to_string(), style);
            }
            continue;
        }

        // class A,B className
        if let Some(rest) = line.strip_prefix("class ") {
            if let Some((ids_str, class_name)) = rest.rsplit_once(' ') {
                let ids: Vec<String> = ids_str.split(',').map(|s| s.trim().to_string()).collect();
                class_assignments.push((ids, class_name.trim().to_string()));
            }
            continue;
        }

        // linkStyle 0 stroke:#ff3
        if let Some(rest) = line.strip_prefix("linkStyle ") {
            if let Some((idx_str, props)) = rest.split_once(' ')
                && let Ok(idx) = idx_str.parse::<usize>()
            {
                let style = parse_edge_style_props(props);
                link_styles.push((idx, style));
            }
            continue;
        }

        // Message — try to parse arrow
        if let Some(event) = parse_message(line, &mut participants) {
            push_event!(event);
            continue;
        }
    }

    // Apply class assignments to participants
    for (ids, class_name) in &class_assignments {
        if let Some(class_style) = class_defs.get(class_name) {
            for id in ids {
                if let Some(p) = participants.iter_mut().find(|p| p.id == *id) {
                    p.style = Some(class_style.clone());
                }
            }
        }
    }

    // Apply inline style directives (override class)
    for (id, style) in &node_styles {
        if let Some(p) = participants.iter_mut().find(|p| p.id == *id) {
            let existing = p.style.get_or_insert_with(NodeStyle::default);
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

    // Apply link styles to messages by index
    let mut msg_idx = 0usize;
    for event in top_events.iter_mut() {
        if let Event::Message { edge_style, .. } = event {
            if let Some((_, style)) = link_styles.iter().find(|(i, _)| *i == msg_idx) {
                *edge_style = Some(style.clone());
            }
            msg_idx += 1;
        }
    }

    Ok(SequenceDiagram {
        participants,
        events: top_events,
        autonumber,
    })
}

fn parse_note(rest: &str) -> Option<Event> {
    // rest is everything after "Note "
    // Patterns: "right of A: text", "left of A: text", "over A: text", "over A,B: text"
    let (position, after_pos) = if let Some(r) = rest.strip_prefix("right of ") {
        (NotePosition::RightOf, r)
    } else if let Some(r) = rest.strip_prefix("left of ") {
        (NotePosition::LeftOf, r)
    } else if let Some(r) = rest.strip_prefix("over ") {
        (NotePosition::Over, r)
    } else {
        return None;
    };

    // after_pos is "A: text" or "A,B: text"
    let (participants_str, text) = after_pos.split_once(':')?;
    let participants: Vec<String> = participants_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let text = text.trim().to_string();

    Some(Event::Note {
        position,
        participants,
        text,
    })
}

fn parse_fragment_opener(line: &str) -> Option<FragmentKind> {
    if line == "loop" || line.starts_with("loop ") {
        Some(FragmentKind::Loop)
    } else if line == "alt" || line.starts_with("alt ") {
        Some(FragmentKind::Alt)
    } else if line == "opt" || line.starts_with("opt ") {
        Some(FragmentKind::Opt)
    } else if line == "par" || line.starts_with("par ") {
        Some(FragmentKind::Par)
    } else {
        None
    }
}

fn parse_fragment_label(line: &str) -> String {
    for prefix in &["loop ", "alt ", "opt ", "par "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    String::new()
}

/// Parse arrow variants — match longest prefix first:
/// -->> DashedArrow
/// ->>  SolidArrow
/// -->  DashedOpen
/// --x  DashedCross
/// ->   SolidOpen
/// -x   SolidCross
fn parse_message(line: &str, participants: &mut Vec<Participant>) -> Option<Event> {
    // Try each arrow variant in longest-first order
    let arrows: &[(&str, ArrowStyle)] = &[
        ("-->>", ArrowStyle::DashedArrow),
        ("->>", ArrowStyle::SolidArrow),
        ("-->", ArrowStyle::DashedOpen),
        ("--x", ArrowStyle::DashedCross),
        ("->", ArrowStyle::SolidOpen),
        ("-x", ArrowStyle::SolidCross),
    ];

    for (arrow_str, arrow_style) in arrows {
        if let Some(pos) = line.find(arrow_str) {
            // Ensure there's a colon after the arrow (message label separator)
            let after_arrow = &line[pos + arrow_str.len()..];
            if let Some(colon_pos) = after_arrow.find(':') {
                let from = line[..pos].trim().to_string();
                let to = after_arrow[..colon_pos].trim().to_string();
                let label = after_arrow[colon_pos + 1..].trim().to_string();

                // Validate from/to are non-empty
                if from.is_empty() || to.is_empty() {
                    continue;
                }

                // Auto-create implicit participants
                for id in &[&from, &to] {
                    if !participants.iter().any(|p| &p.id == *id) {
                        participants.push(Participant {
                            id: id.to_string(),
                            label: id.to_string(),
                            style: None,
                        });
                    }
                }

                return Some(Event::Message {
                    from,
                    to,
                    label,
                    arrow: arrow_style.clone(),
                    edge_style: None,
                });
            }
        }
    }

    None
}

fn parse_node_style_props(props: &str) -> NodeStyle {
    let mut style = NodeStyle::default();
    for prop in props.split(',') {
        let prop = prop.trim();
        if let Some((key, value)) = prop.split_once(':') {
            match key.trim() {
                "fill" => style.fill = parse_color(value.trim()),
                "stroke" => style.stroke = parse_color(value.trim()),
                "color" => style.color = parse_color(value.trim()),
                _ => {}
            }
        }
    }
    style
}

fn parse_edge_style_props(props: &str) -> MermaidEdgeStyle {
    let mut style = MermaidEdgeStyle::default();
    for prop in props.split(',') {
        let prop = prop.trim();
        if let Some((key, value)) = prop.split_once(':') {
            match key.trim() {
                "stroke" => style.stroke = parse_color(value.trim()),
                "color" => style.label_color = parse_color(value.trim()),
                _ => {}
            }
        }
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::sequence::{ArrowStyle, Event, FragmentKind, NotePosition};
    use crate::render::Color;

    #[test]
    fn test_parse_explicit_participants() {
        let input = "sequenceDiagram\n    participant A as Alice\n    participant B as Bob\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.participants.len(), 2);
        assert_eq!(diagram.participants[0].id, "A");
        assert_eq!(diagram.participants[0].label, "Alice");
        assert_eq!(diagram.participants[1].id, "B");
        assert_eq!(diagram.participants[1].label, "Bob");
    }

    #[test]
    fn test_parse_participant_no_alias() {
        let input = "sequenceDiagram\n    participant Alice\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.participants[0].id, "Alice");
        assert_eq!(diagram.participants[0].label, "Alice");
    }

    #[test]
    fn test_parse_actor_treated_as_participant() {
        let input = "sequenceDiagram\n    actor A as Alice\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.participants[0].id, "A");
        assert_eq!(diagram.participants[0].label, "Alice");
    }

    #[test]
    fn test_parse_solid_arrow_message() {
        let input = "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Hello\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.events.len(), 1);
        if let Event::Message {
            from,
            to,
            label,
            arrow,
            ..
        } = &diagram.events[0]
        {
            assert_eq!(from, "A");
            assert_eq!(to, "B");
            assert_eq!(label, "Hello");
            assert_eq!(*arrow, ArrowStyle::SolidArrow);
        } else {
            panic!("Expected Message event");
        }
    }

    #[test]
    fn test_parse_all_arrow_styles() {
        let input = "\
sequenceDiagram
    participant A
    participant B
    A->>B: solid arrow
    B-->>A: dashed arrow
    A->B: solid open
    B-->A: dashed open
    A-xB: solid cross
    B--xA: dashed cross
";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.events.len(), 6);

        let arrows: Vec<ArrowStyle> = diagram
            .events
            .iter()
            .map(|e| {
                if let Event::Message { arrow, .. } = e {
                    arrow.clone()
                } else {
                    panic!()
                }
            })
            .collect();

        assert_eq!(
            arrows,
            vec![
                ArrowStyle::SolidArrow,
                ArrowStyle::DashedArrow,
                ArrowStyle::SolidOpen,
                ArrowStyle::DashedOpen,
                ArrowStyle::SolidCross,
                ArrowStyle::DashedCross,
            ]
        );
    }

    #[test]
    fn test_parse_implicit_participants() {
        let input = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob->>Charlie: Forward\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.participants.len(), 3);
        assert_eq!(diagram.participants[0].id, "Alice");
        assert_eq!(diagram.participants[1].id, "Bob");
        assert_eq!(diagram.participants[2].id, "Charlie");
    }

    #[test]
    fn test_parse_comments_ignored() {
        let input = "sequenceDiagram\n    %% this is a comment\n    participant A\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.participants.len(), 1);
    }

    #[test]
    fn test_parse_missing_header() {
        let input = "graph TD\n    A --> B\n";
        let result = parse_sequence(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_note_right_of() {
        let input = "sequenceDiagram\n    participant A\n    Note right of A: Hello\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.events.len(), 1);
        if let Event::Note {
            position,
            participants,
            text,
        } = &diagram.events[0]
        {
            assert_eq!(*position, NotePosition::RightOf);
            assert_eq!(participants, &vec!["A".to_string()]);
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected Note event");
        }
    }

    #[test]
    fn test_parse_note_over_spanning() {
        let input =
            "sequenceDiagram\n    participant A\n    participant B\n    Note over A,B: Shared\n";
        let diagram = parse_sequence(input).unwrap();
        if let Event::Note {
            position,
            participants,
            ..
        } = &diagram.events[0]
        {
            assert_eq!(*position, NotePosition::Over);
            assert_eq!(participants.len(), 2);
        } else {
            panic!("Expected Note event");
        }
    }

    #[test]
    fn test_parse_activate_deactivate() {
        let input = "sequenceDiagram\n    participant A\n    activate A\n    deactivate A\n";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.events.len(), 2);
        assert!(
            matches!(&diagram.events[0], Event::Activate { participant } if participant == "A")
        );
        assert!(
            matches!(&diagram.events[1], Event::Deactivate { participant } if participant == "A")
        );
    }

    #[test]
    fn test_parse_loop_fragment() {
        let input = "\
sequenceDiagram
    participant A
    participant B
    loop Every minute
        A->>B: Ping
    end
";
        let diagram = parse_sequence(input).unwrap();
        assert_eq!(diagram.events.len(), 1);
        if let Event::Fragment {
            kind,
            label,
            sections,
        } = &diagram.events[0]
        {
            assert_eq!(*kind, FragmentKind::Loop);
            assert_eq!(label, "Every minute");
            assert_eq!(sections.len(), 1);
            assert_eq!(sections[0].events.len(), 1);
        } else {
            panic!("Expected Fragment event");
        }
    }

    #[test]
    fn test_parse_alt_with_else() {
        let input = "\
sequenceDiagram
    participant A
    participant B
    alt Success
        A->>B: OK
    else Failure
        A->>B: Error
    end
";
        let diagram = parse_sequence(input).unwrap();
        if let Event::Fragment { kind, sections, .. } = &diagram.events[0] {
            assert_eq!(*kind, FragmentKind::Alt);
            assert_eq!(sections.len(), 2);
            assert_eq!(sections[0].label, None);
            assert_eq!(sections[1].label, Some("Failure".to_string()));
        } else {
            panic!("Expected Fragment event");
        }
    }

    #[test]
    fn test_parse_autonumber() {
        let input = "sequenceDiagram\n    autonumber\n    participant A\n";
        let diagram = parse_sequence(input).unwrap();
        assert!(diagram.autonumber);
    }

    #[test]
    fn test_parse_nested_fragments() {
        let input = "\
sequenceDiagram
    participant A
    participant B
    loop Retry
        alt Success
            A->>B: OK
        else Fail
            A->>B: Error
        end
    end
";
        let diagram = parse_sequence(input).unwrap();
        if let Event::Fragment { kind, sections, .. } = &diagram.events[0] {
            assert_eq!(*kind, FragmentKind::Loop);
            assert_eq!(sections[0].events.len(), 1);
            if let Event::Fragment {
                kind: inner_kind,
                sections: inner_sections,
                ..
            } = &sections[0].events[0]
            {
                assert_eq!(*inner_kind, FragmentKind::Alt);
                assert_eq!(inner_sections.len(), 2);
            } else {
                panic!("Expected nested Fragment");
            }
        } else {
            panic!("Expected Fragment event");
        }
    }

    #[test]
    fn test_parse_sequence_style_participant() {
        let input = "sequenceDiagram\n    participant A\n    participant B\n    style A fill:#ff9900,stroke:#333\n    A->>B: Hello\n";
        let diagram = parse_sequence(input).unwrap();
        let a_style = diagram.participants[0]
            .style
            .as_ref()
            .expect("A should have style");
        assert_eq!(a_style.fill, Some(Color::Rgb(255, 153, 0)));
        assert!(diagram.participants[1].style.is_none());
    }

    #[test]
    fn test_parse_sequence_linkstyle() {
        let input = "sequenceDiagram\n    participant A\n    participant B\n    A->>B: Hello\n    B->>A: World\n    linkStyle 0 stroke:#f00\n";
        let diagram = parse_sequence(input).unwrap();
        if let Event::Message { edge_style, .. } = &diagram.events[0] {
            let es = edge_style
                .as_ref()
                .expect("message 0 should have edge style");
            assert_eq!(es.stroke, Some(Color::Rgb(255, 0, 0)));
        } else {
            panic!("Expected Message event");
        }
        if let Event::Message { edge_style, .. } = &diagram.events[1] {
            assert!(edge_style.is_none());
        }
    }

    #[test]
    fn test_parse_sequence_classdef_and_class() {
        let input = "sequenceDiagram\n    participant A\n    participant B\n    classDef server fill:#0f0\n    class A server\n    A->>B: Hello\n";
        let diagram = parse_sequence(input).unwrap();
        let a_style = diagram.participants[0]
            .style
            .as_ref()
            .expect("A should have style");
        assert_eq!(a_style.fill, Some(Color::Rgb(0, 255, 0)));
    }
}
