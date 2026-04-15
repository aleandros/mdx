use std::collections::HashMap;

use super::{ArrowStyle, Event, FragmentKind, FragmentSection, SequenceDiagram};

const PARTICIPANT_PADDING: usize = 4;
const MIN_COLUMN_GAP: usize = 16;
const PARTICIPANT_BOX_HEIGHT: usize = 3;
const MESSAGE_HEIGHT: usize = 2;
const SELF_MESSAGE_HEIGHT: usize = 3;
const NOTE_HEIGHT: usize = 3;
const FRAGMENT_MARGIN: usize = 2;

#[derive(Debug, Clone)]
pub struct SequenceLayout {
    pub participants: Vec<PositionedParticipant>,
    pub messages: Vec<PositionedMessage>,
    pub notes: Vec<PositionedNote>,
    pub activations: Vec<PositionedActivation>,
    pub fragments: Vec<PositionedFragment>,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedParticipant {
    pub label: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub center_x: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedMessage {
    pub from_x: usize,
    pub to_x: usize,
    pub y: usize,
    pub label: String,
    pub arrow: ArrowStyle,
    pub self_message: bool,
}

#[derive(Debug, Clone)]
pub struct PositionedNote {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PositionedActivation {
    pub x: usize,
    pub y_start: usize,
    pub y_end: usize,
}

#[derive(Debug, Clone)]
pub struct PositionedFragment {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub kind: FragmentKind,
    pub label: String,
    pub section_dividers: Vec<(usize, Option<String>)>,
}

pub fn layout(diagram: &SequenceDiagram) -> SequenceLayout {
    if diagram.participants.is_empty() {
        return SequenceLayout {
            participants: vec![],
            messages: vec![],
            notes: vec![],
            activations: vec![],
            fragments: vec![],
            width: 0,
            height: 0,
        };
    }

    // Build participant index (id -> index)
    let participant_index: HashMap<String, usize> = diagram
        .participants
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.clone(), i))
        .collect();

    let n = diagram.participants.len();

    // Step 1: Compute box width for each participant
    let box_widths: Vec<usize> = diagram
        .participants
        .iter()
        .map(|p| p.label.len() + PARTICIPANT_PADDING)
        .collect();

    // Step 2: Compute minimum gaps between adjacent participants based on message labels
    // gap[i] = minimum gap between participant i and participant i+1
    let mut gaps: Vec<usize> = vec![MIN_COLUMN_GAP; n.saturating_sub(1)];

    // Walk all events recursively to find messages and update gaps
    collect_message_gaps(
        &diagram.events,
        &participant_index,
        &box_widths,
        &mut gaps,
    );

    // Step 3: Assign x positions (center_x) for each participant
    // center_x[0] = box_widths[0] / 2
    let mut center_xs: Vec<usize> = vec![0; n];
    center_xs[0] = box_widths[0] / 2;
    for i in 1..n {
        let prev_right = center_xs[i - 1] + box_widths[i - 1] / 2;
        let gap = gaps[i - 1];
        center_xs[i] = prev_right + gap + box_widths[i] / 2;
    }

    // Build PositionedParticipants
    let positioned_participants: Vec<PositionedParticipant> = diagram
        .participants
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w = box_widths[i];
            let cx = center_xs[i];
            PositionedParticipant {
                label: p.label.clone(),
                x: cx.saturating_sub(w / 2),
                y: 0,
                width: w,
                center_x: cx,
            }
        })
        .collect();

    // Total canvas width = rightmost right edge
    let total_width = {
        let last = n - 1;
        center_xs[last] + box_widths[last] / 2 + box_widths[last] % 2
    };

    // Step 4: Vertical pass
    let mut current_y = PARTICIPANT_BOX_HEIGHT + 1;
    let mut messages: Vec<PositionedMessage> = vec![];
    let mut notes: Vec<PositionedNote> = vec![];
    let mut activations: Vec<PositionedActivation> = vec![];
    let mut fragments: Vec<PositionedFragment> = vec![];

    // Per-participant activation stacks: participant index -> stack of y_start values
    let mut activation_stacks: Vec<Vec<usize>> = vec![vec![]; n];

    let mut message_counter: usize = 0;
    process_events(
        &diagram.events,
        &participant_index,
        &center_xs,
        &box_widths,
        &mut current_y,
        &mut messages,
        &mut notes,
        &mut activations,
        &mut fragments,
        &mut activation_stacks,
        diagram.autonumber,
        &mut message_counter,
    );

    let height = current_y;

    SequenceLayout {
        participants: positioned_participants,
        messages,
        notes,
        activations,
        fragments,
        width: total_width,
        height,
    }
}

/// Recursively walk events and update the gaps array based on message labels.
fn collect_message_gaps(
    events: &[Event],
    participant_index: &HashMap<String, usize>,
    box_widths: &[usize],
    gaps: &mut Vec<usize>,
) {
    for event in events {
        match event {
            Event::Message { from, to, label, .. } => {
                if from == to {
                    continue; // self-message, no inter-participant gap needed
                }
                let fi = match participant_index.get(from) {
                    Some(&i) => i,
                    None => continue,
                };
                let ti = match participant_index.get(to) {
                    Some(&i) => i,
                    None => continue,
                };
                let (left, right) = if fi < ti { (fi, ti) } else { (ti, fi) };
                let num_gaps = right - left;
                if num_gaps == 0 {
                    continue;
                }
                // Label must fit across all gaps and intermediate box widths in range.
                let label_needed = label.len() + 2;
                let intermediate_width: usize = (left + 1..right).map(|k| box_widths[k]).sum();
                let current_span: usize =
                    gaps[left..right].iter().sum::<usize>() + intermediate_width;
                if current_span < label_needed {
                    let extra = label_needed - current_span;
                    let extra_per_gap = extra.div_ceil(num_gaps);
                    for gap in gaps[left..right].iter_mut() {
                        *gap += extra_per_gap;
                    }
                }
            }
            Event::Fragment { sections, .. } => {
                for section in sections {
                    collect_message_gaps(
                        &section.events,
                        participant_index,
                        box_widths,
                        gaps,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Process events recursively, advancing current_y and collecting positioned items.
#[allow(clippy::too_many_arguments)]
fn process_events(
    events: &[Event],
    participant_index: &HashMap<String, usize>,
    center_xs: &[usize],
    box_widths: &[usize],
    current_y: &mut usize,
    messages: &mut Vec<PositionedMessage>,
    notes: &mut Vec<PositionedNote>,
    activations: &mut Vec<PositionedActivation>,
    fragments: &mut Vec<PositionedFragment>,
    activation_stacks: &mut Vec<Vec<usize>>,
    autonumber: bool,
    message_counter: &mut usize,
) {
    for event in events {
        match event {
            Event::Message { from, to, label, arrow } => {
                let fi = participant_index.get(from).copied().unwrap_or(0);
                let ti = participant_index.get(to).copied().unwrap_or(0);
                let self_msg = from == to;

                let from_x = center_xs[fi];
                let to_x = center_xs[ti];

                let display_label = if autonumber {
                    *message_counter += 1;
                    format!("{}. {}", *message_counter, label)
                } else {
                    label.clone()
                };

                messages.push(PositionedMessage {
                    from_x,
                    to_x,
                    y: *current_y,
                    label: display_label,
                    arrow: arrow.clone(),
                    self_message: self_msg,
                });

                if self_msg {
                    *current_y += SELF_MESSAGE_HEIGHT;
                } else {
                    *current_y += MESSAGE_HEIGHT;
                }
            }
            Event::Note { position, participants, text } => {
                let note_width = text.len() + 4;

                let (note_x, width) = match position {
                    super::NotePosition::RightOf => {
                        // Place note to the right of the rightmost named participant
                        let right_edge = participants
                            .iter()
                            .filter_map(|id| participant_index.get(id))
                            .map(|&i| center_xs[i] + (box_widths[i] + 1) / 2)
                            .max()
                            .unwrap_or(0);
                        (right_edge + 1, note_width)
                    }
                    super::NotePosition::LeftOf => {
                        // Place note to the left of the leftmost named participant
                        let left_edge = participants
                            .iter()
                            .filter_map(|id| participant_index.get(id))
                            .map(|&i| center_xs[i].saturating_sub(box_widths[i] / 2))
                            .min()
                            .unwrap_or(note_width + 1);
                        let x = left_edge.saturating_sub(note_width + 1);
                        (x, note_width)
                    }
                    super::NotePosition::Over => {
                        // Span between leftmost and rightmost participant boxes
                        let min_x = participants
                            .iter()
                            .filter_map(|id| participant_index.get(id))
                            .map(|&i| center_xs[i].saturating_sub(box_widths[i] / 2))
                            .min()
                            .unwrap_or(0);
                        let max_x = participants
                            .iter()
                            .filter_map(|id| participant_index.get(id))
                            .map(|&i| center_xs[i] + (box_widths[i] + 1) / 2)
                            .max()
                            .unwrap_or(min_x + note_width);
                        let width = (max_x - min_x).max(note_width);
                        (min_x, width)
                    }
                };

                notes.push(PositionedNote {
                    x: note_x,
                    y: *current_y,
                    width,
                    height: NOTE_HEIGHT,
                    text: text.clone(),
                });
                *current_y += NOTE_HEIGHT;
            }
            Event::Activate { participant } => {
                if let Some(&idx) = participant_index.get(participant) {
                    // Start activation at the arrow row of the preceding message
                    // (one row back), so the activation box spans the response message.
                    let y_start = current_y.saturating_sub(1);
                    activation_stacks[idx].push(y_start);
                }
                // 0 rows consumed
            }
            Event::Deactivate { participant } => {
                if let Some(&idx) = participant_index.get(participant) {
                    let y_start = activation_stacks[idx].pop();
                    if let Some(y_start) = y_start {
                        activations.push(PositionedActivation {
                            x: center_xs[idx],
                            y_start,
                            y_end: *current_y,
                        });
                    }
                }
                // 0 rows consumed
            }
            Event::Fragment { kind, label, sections } => {
                let frag_y = *current_y;

                // Fragment header takes 1 row
                *current_y += 1;

                // Find participant bounds across all sections
                let all_participant_ids = collect_participant_ids_in_sections(sections);
                let (frag_left_x, frag_right_x) = fragment_bounds(
                    &all_participant_ids,
                    participant_index,
                    center_xs,
                    box_widths,
                );

                let mut section_dividers: Vec<(usize, Option<String>)> = vec![];

                for (sec_idx, section) in sections.iter().enumerate() {
                    // Process events within this section
                    process_events(
                        &section.events,
                        participant_index,
                        center_xs,
                        box_widths,
                        current_y,
                        messages,
                        notes,
                        activations,
                        fragments,
                        activation_stacks,
                        autonumber,
                        message_counter,
                    );

                    // If not the last section, add a divider row labeled with the NEXT section's label
                    if sec_idx < sections.len() - 1 {
                        let next_label = sections[sec_idx + 1].label.clone();
                        section_dividers.push((*current_y, next_label));
                        *current_y += 1;
                    }
                }

                // Fragment footer takes 1 row
                *current_y += 1;

                let frag_height = *current_y - frag_y;

                // Minimum width: header text "─ {kind} [{label}] ─┐" needs space
                let kind_str_len = match kind {
                    super::FragmentKind::Loop => 4,
                    super::FragmentKind::Alt => 3,
                    super::FragmentKind::Opt => 3,
                    super::FragmentKind::Par => 3,
                };
                let header_min = if label.is_empty() {
                    kind_str_len + 4 // "─ kind ─┐" plus "┌" = kind + 5 chars min
                } else {
                    kind_str_len + label.len() + 7 // "─ kind [label] ─┐"
                };
                // Also consider section divider labels
                let divider_min = section_dividers.iter().filter_map(|(_, lbl)| lbl.as_ref())
                    .map(|lbl| lbl.len() + 7) // "─ [label] ─┤"
                    .max()
                    .unwrap_or(0);
                let frag_width = (frag_right_x - frag_left_x).max(header_min).max(divider_min);

                fragments.push(PositionedFragment {
                    x: frag_left_x,
                    y: frag_y,
                    width: frag_width,
                    height: frag_height,
                    kind: kind.clone(),
                    label: label.clone(),
                    section_dividers,
                });
            }
        }
    }
}

/// Collect all unique participant IDs mentioned in fragment sections (recursively).
fn collect_participant_ids_in_sections(sections: &[FragmentSection]) -> Vec<String> {
    let mut ids: Vec<String> = vec![];
    for section in sections {
        collect_participant_ids_in_events(&section.events, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn collect_participant_ids_in_events(events: &[Event], ids: &mut Vec<String>) {
    for event in events {
        match event {
            Event::Message { from, to, .. } => {
                ids.push(from.clone());
                ids.push(to.clone());
            }
            Event::Note { participants, .. } => {
                ids.extend(participants.iter().cloned());
            }
            Event::Activate { participant } | Event::Deactivate { participant } => {
                ids.push(participant.clone());
            }
            Event::Fragment { sections, .. } => {
                collect_participant_ids_in_sections(sections)
                    .into_iter()
                    .for_each(|id| ids.push(id));
            }
        }
    }
}

/// Compute the left and right x bounds for a fragment given a set of participant IDs.
fn fragment_bounds(
    participant_ids: &[String],
    participant_index: &HashMap<String, usize>,
    center_xs: &[usize],
    box_widths: &[usize],
) -> (usize, usize) {
    let n = center_xs.len();
    let left_x = participant_ids
        .iter()
        .filter_map(|id| participant_index.get(id))
        .map(|&i| center_xs[i].saturating_sub(box_widths[i] / 2))
        .min()
        .unwrap_or_else(|| {
            if n > 0 {
                center_xs[0].saturating_sub(box_widths[0] / 2)
            } else {
                0
            }
        })
        .saturating_sub(FRAGMENT_MARGIN);

    let right_x = participant_ids
        .iter()
        .filter_map(|id| participant_index.get(id))
        .map(|&i| center_xs[i] + box_widths[i] / 2)
        .max()
        .unwrap_or_else(|| {
            if n > 0 {
                center_xs[n - 1] + box_widths[n - 1] / 2
            } else {
                FRAGMENT_MARGIN * 2
            }
        })
        + FRAGMENT_MARGIN;

    (left_x, right_x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::sequence::{ArrowStyle, Event, Participant, SequenceDiagram};

    fn make_diagram(participants: Vec<(&str, &str)>, events: Vec<Event>) -> SequenceDiagram {
        SequenceDiagram {
            participants: participants
                .into_iter()
                .map(|(id, label)| Participant {
                    id: id.to_string(),
                    label: label.to_string(),
                })
                .collect(),
            events,
            autonumber: false,
        }
    }

    #[test]
    fn test_two_participants_positioned() {
        let diagram = make_diagram(vec![("A", "Alice"), ("B", "Bob")], vec![]);
        let result = layout(&diagram);
        assert_eq!(result.participants.len(), 2);
        assert!(
            result.participants[0].center_x < result.participants[1].center_x,
            "Alice center_x ({}) should be left of Bob center_x ({})",
            result.participants[0].center_x,
            result.participants[1].center_x
        );
    }

    #[test]
    fn test_three_participants_ordered() {
        let diagram = make_diagram(
            vec![("A", "Alice"), ("B", "Bob"), ("C", "Charlie")],
            vec![],
        );
        let result = layout(&diagram);
        assert!(result.participants[0].center_x < result.participants[1].center_x);
        assert!(result.participants[1].center_x < result.participants[2].center_x);
    }

    #[test]
    fn test_message_between_participants() {
        let diagram = make_diagram(
            vec![("A", "Alice"), ("B", "Bob")],
            vec![Event::Message {
                from: "A".to_string(),
                to: "B".to_string(),
                label: "Hello".to_string(),
                arrow: ArrowStyle::SolidArrow,
            }],
        );
        let result = layout(&diagram);
        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].from_x < result.messages[0].to_x);
        assert!(result.height > 0);
    }

    #[test]
    fn test_self_message_detected() {
        let diagram = make_diagram(
            vec![("A", "Alice")],
            vec![Event::Message {
                from: "A".to_string(),
                to: "A".to_string(),
                label: "Think".to_string(),
                arrow: ArrowStyle::SolidArrow,
            }],
        );
        let result = layout(&diagram);
        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].self_message);
    }

    #[test]
    fn test_layout_dimensions_nonzero() {
        let diagram = make_diagram(
            vec![("A", "Alice"), ("B", "Bob")],
            vec![Event::Message {
                from: "A".to_string(),
                to: "B".to_string(),
                label: "Hello".to_string(),
                arrow: ArrowStyle::SolidArrow,
            }],
        );
        let result = layout(&diagram);
        assert!(result.width > 0, "Width should be > 0");
        assert!(result.height > 0, "Height should be > 0");
    }

    #[test]
    fn test_fragment_creates_positioned_fragment() {
        let diagram = make_diagram(
            vec![("A", "Alice"), ("B", "Bob")],
            vec![Event::Fragment {
                kind: crate::mermaid::sequence::FragmentKind::Loop,
                label: "Retry".to_string(),
                sections: vec![crate::mermaid::sequence::FragmentSection {
                    label: None,
                    events: vec![Event::Message {
                        from: "A".to_string(),
                        to: "B".to_string(),
                        label: "Ping".to_string(),
                        arrow: ArrowStyle::SolidArrow,
                    }],
                }],
            }],
        );
        let result = layout(&diagram);
        assert_eq!(result.fragments.len(), 1);
        assert!(result.fragments[0].height > 0);
        assert!(result.fragments[0].width > 0);
    }

    #[test]
    fn test_autonumber_prefixes_labels() {
        let diagram = SequenceDiagram {
            participants: vec![
                Participant { id: "A".to_string(), label: "A".to_string() },
                Participant { id: "B".to_string(), label: "B".to_string() },
            ],
            events: vec![
                Event::Message {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    label: "First".to_string(),
                    arrow: ArrowStyle::SolidArrow,
                },
                Event::Message {
                    from: "B".to_string(),
                    to: "A".to_string(),
                    label: "Second".to_string(),
                    arrow: ArrowStyle::DashedArrow,
                },
            ],
            autonumber: true,
        };
        let result = layout(&diagram);
        assert_eq!(result.messages[0].label, "1. First");
        assert_eq!(result.messages[1].label, "2. Second");
    }

    #[test]
    fn test_activation_creates_positioned_activation() {
        let diagram = make_diagram(
            vec![("A", "Alice"), ("B", "Bob")],
            vec![
                Event::Message {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    label: "Request".to_string(),
                    arrow: ArrowStyle::SolidArrow,
                },
                Event::Activate {
                    participant: "B".to_string(),
                },
                Event::Message {
                    from: "B".to_string(),
                    to: "A".to_string(),
                    label: "Response".to_string(),
                    arrow: ArrowStyle::DashedArrow,
                },
                Event::Deactivate {
                    participant: "B".to_string(),
                },
            ],
        );
        let result = layout(&diagram);
        assert_eq!(result.activations.len(), 1);
        assert!(result.activations[0].y_start < result.activations[0].y_end);
    }
}
