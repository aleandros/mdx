pub mod ascii;
pub mod layout;
pub mod parse;

#[derive(Debug, Clone, PartialEq)]
pub struct ErDiagram {
    pub direction: super::Direction,
    /// True when the source contained an explicit `direction ...` line.
    /// When false, `render_mermaid` chooses adaptively (LR with TD fallback).
    pub direction_explicit: bool,
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: String,
    pub attributes: Vec<Attribute>,
    /// Populated by the layout adapter.
    pub rendered_lines: Vec<EntityLine>,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub ty: String,
    pub name: String,
    pub key: KeyKind,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    None,
    Pk,
    Fk,
    PkFk,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub left: String,
    pub right: String,
    pub left_card: Cardinality,
    pub right_card: Cardinality,
    pub identifying: bool,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMany,
    OneOrMany,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityLine {
    pub kind: EntityLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityLineKind {
    Header,
    Separator,
    AttrRow,
    CommentRow,
}

/// Carried on `Edge` for ER edges so the painter can draw cardinality glyphs.
#[derive(Debug, Clone, PartialEq)]
pub struct ErEdgeMeta {
    pub left_card: Cardinality,
    pub right_card: Cardinality,
    pub identifying: bool,
}
