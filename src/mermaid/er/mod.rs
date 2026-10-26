pub(crate) mod ascii;
pub(crate) mod layout;
pub(crate) mod parse;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ErDiagram {
    pub(crate) direction: super::Direction,
    /// True when the source contained an explicit `direction ...` line.
    /// When false, `render_mermaid` chooses adaptively (LR with TD fallback).
    pub(crate) direction_explicit: bool,
    pub(crate) entities: Vec<Entity>,
    pub(crate) relationships: Vec<Relationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entity {
    pub(crate) name: String,
    pub(crate) attributes: Vec<Attribute>,
    /// Populated by the layout adapter.
    pub(crate) rendered_lines: Vec<EntityLine>,
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// User-supplied or class-derived style. None means "use theme defaults".
    pub(crate) node_style: Option<crate::mermaid::NodeStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Attribute {
    pub(crate) ty: String,
    pub(crate) name: String,
    pub(crate) key: KeyKind,
    pub(crate) comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyKind {
    None,
    Pk,
    Fk,
    PkFk,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Relationship {
    pub(crate) left: String,
    pub(crate) right: String,
    pub(crate) left_card: Cardinality,
    pub(crate) right_card: Cardinality,
    pub(crate) identifying: bool,
    pub(crate) label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMany,
    OneOrMany,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EntityLine {
    pub(crate) kind: EntityLineKind,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityLineKind {
    Header,
    Separator,
    AttrRow,
    CommentRow,
}

/// Carried on `Edge` for ER edges so the painter can draw cardinality glyphs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ErEdgeMeta {
    pub(crate) left_card: Cardinality,
    pub(crate) right_card: Cardinality,
    pub(crate) identifying: bool,
}
