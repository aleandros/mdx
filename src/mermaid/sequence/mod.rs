pub(crate) mod ascii;
pub(crate) mod layout;
pub(crate) mod parse;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SequenceDiagram {
    pub(crate) participants: Vec<Participant>,
    pub(crate) events: Vec<Event>,
    pub(crate) autonumber: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Participant {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) style: Option<crate::mermaid::NodeStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    Message {
        from: String,
        to: String,
        label: String,
        arrow: ArrowStyle,
        edge_style: Option<crate::mermaid::MermaidEdgeStyle>,
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
pub(crate) struct FragmentSection {
    pub(crate) label: Option<String>,
    pub(crate) events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ArrowStyle {
    SolidArrow,
    DashedArrow,
    SolidOpen,
    DashedOpen,
    SolidCross,
    DashedCross,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NotePosition {
    RightOf,
    LeftOf,
    Over,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FragmentKind {
    Loop,
    Alt,
    Opt,
    Par,
}
