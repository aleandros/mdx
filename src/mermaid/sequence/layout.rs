use super::{ArrowStyle, FragmentKind, SequenceDiagram};

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

pub fn layout(_diagram: &SequenceDiagram) -> SequenceLayout {
    SequenceLayout {
        participants: vec![],
        messages: vec![],
        notes: vec![],
        activations: vec![],
        fragments: vec![],
        width: 0,
        height: 0,
    }
}
