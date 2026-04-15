pub mod ascii;
pub mod layout;
pub mod parse;

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceDiagram {
    pub participants: Vec<Participant>,
    pub events: Vec<Event>,
    pub autonumber: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Message {
        from: String,
        to: String,
        label: String,
        arrow: ArrowStyle,
    },
    Note {
        position: NotePosition,
        participants: Vec<String>,
        text: String,
    },
    Activate {
        participant: String,
    },
    Deactivate {
        participant: String,
    },
    Fragment {
        kind: FragmentKind,
        label: String,
        sections: Vec<FragmentSection>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FragmentSection {
    pub label: Option<String>,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrowStyle {
    SolidArrow,
    DashedArrow,
    SolidOpen,
    DashedOpen,
    SolidCross,
    DashedCross,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NotePosition {
    RightOf,
    LeftOf,
    Over,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FragmentKind {
    Loop,
    Alt,
    Opt,
    Par,
}
