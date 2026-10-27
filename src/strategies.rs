//! Shared, shrinking inputs. References are generated from the declared IDs so
//! removing an edge or shrinking the node count always leaves valid grammar.
use crate::render::{Color, DiagramKind, RenderedBlock, SpanStyle, StyledLine, StyledSpan};
use proptest::prelude::*;
use std::fmt::Write as _;

pub(crate) fn model_config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(cases),
        failure_persistence: Some(Box::new(
            proptest::test_runner::FileFailurePersistence::SourceParallel("proptest-regressions"),
        )),
        ..ProptestConfig::default()
    }
}

pub(crate) fn width() -> impl Strategy<Value = u16> {
    1u16..=300
}
pub(crate) fn unicode() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..100).prop_map(|v| v.into_iter().collect())
}
pub(crate) fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!['a', 'Z', '0', ' ', 'é', '漢', '🦀', '\u{301}']),
        0..60,
    )
    .prop_map(|v| v.into_iter().collect())
}
pub(crate) fn styled_line() -> impl Strategy<Value = StyledLine> {
    (
        0usize..10,
        prop::collection::vec(
            (
                text(),
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
                any::<bool>(),
            ),
            0..6,
        ),
    )
        .prop_map(|(indent, spans)| {
            let mut result = vec![StyledSpan::plain(" ".repeat(indent))];
            result.extend(
                spans
                    .into_iter()
                    .map(|(text, bold, italic, dim, color)| StyledSpan {
                        text,
                        style: SpanStyle {
                            bold,
                            italic,
                            dim,
                            fg: color.then_some(Color::Cyan),
                        },
                    }),
            );
            StyledLine { spans: result }
        })
}
pub(crate) fn line_text(line: &StyledLine) -> String {
    line.spans.iter().map(|s| s.text.as_str()).collect()
}
pub(crate) fn significant(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
pub(crate) fn styled_chars(line: &StyledLine) -> Vec<(char, SpanStyle)> {
    line.spans
        .iter()
        .flat_map(|s| s.text.chars().map(|c| (c, s.style.clone())))
        .collect()
}

/// Each fragment carries its visible payload independently of Markdown parsing.
/// Formatting delimiters, destination metadata and table separators are syntax.
/// Math delimiters remain visible as part of the raw-source fallback.
pub(crate) fn markdown() -> impl Strategy<Value = (String, String)> {
    prop::collection::vec((0u8..11, "[a-zA-Zé漢]{1,12}"), 1..12).prop_map(|parts| {
        let mut source = String::new();
        let mut expected = String::new();
        for (kind, word) in parts {
            let fragment = match kind {
                0 => format!("# {word}"),
                1 => format!("**{word}**"),
                2 => format!("- {word}"),
                3 => format!("```rust\n{word}\n```"),
                4 => format!("```mermaid\n{word}\n```"),
                5 => format!("| {word} |\n| --- |"),
                6 => format!("[{word}](https://example.test)"),
                7 => format!("![{word}](image.png)"),
                8 => format!("${word}$"),
                9 => format!("$$\n{word}\n$$"),
                _ => word.clone(),
            };
            source.push_str(&fragment);
            source.push_str("\n\n");
            match kind {
                8 => write!(expected, "${word}$").expect("writing to a String cannot fail"),
                9 => write!(expected, "$${word}$$").expect("writing to a String cannot fail"),
                _ => expected.push_str(&word),
            }
        }
        (source, expected)
    })
}

#[derive(Debug, Clone)]
pub(crate) struct GraphInput {
    pub(crate) direction: &'static str,
    pub(crate) labels: Vec<String>,
    pub(crate) edges: Vec<(usize, usize)>,
}
pub(crate) fn graph() -> impl Strategy<Value = GraphInput> {
    (
        prop::sample::select(vec!["TD", "BT", "LR", "RL"]),
        prop::collection::vec("[a-zé漢]{0,8}", 1..10),
    )
        .prop_flat_map(|(direction, labels)| {
            let n = labels.len();
            (
                Just(direction),
                Just(labels),
                prop::collection::vec((0..n, 0..n), 0..20),
            )
        })
        .prop_map(|(direction, labels, edges)| GraphInput {
            direction,
            labels,
            edges,
        })
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Topology {
    Dag,
    Cyclic,
}

impl GraphInput {
    pub(crate) fn flowchart(&self, topology: Topology) -> String {
        let dag = topology == Topology::Dag;
        let mut s = format!("graph {}\n", self.direction);
        for (i, label) in self.labels.iter().enumerate() {
            writeln!(s, "N{i}[node{i}X{label}]").expect("writing to a String cannot fail");
        }
        for (i, &(a, b)) in self.edges.iter().enumerate() {
            if dag && a == b {
                continue;
            }
            let (a, b) = if dag { (a.min(b), a.max(b)) } else { (a, b) };
            writeln!(s, "N{a} -->|edge{i}X| N{b}").expect("writing to a String cannot fail");
        }
        s
    }
    pub(crate) fn sequence(&self) -> String {
        let mut s = "sequenceDiagram\n".to_string();
        for (i, label) in self.labels.iter().enumerate() {
            writeln!(s, "participant N{i} as node{i}X{label}")
                .expect("writing to a String cannot fail");
        }
        for (i, (a, b)) in self.edges.iter().enumerate() {
            writeln!(s, "N{a}->>N{b}: edge{i}X").expect("writing to a String cannot fail");
        }
        s
    }
    pub(crate) fn er(&self) -> String {
        let mut s = format!("erDiagram\ndirection {}\n", self.direction);
        for (i, label) in self.labels.iter().enumerate() {
            write!(s, "N{i} {{\n string field{i} \"{label}\"\n}}\n")
                .expect("writing to a String cannot fail");
        }
        for (i, (a, b)) in self.edges.iter().enumerate() {
            writeln!(s, "N{a} ||--o{{ N{b} : edge{i}X").expect("writing to a String cannot fail");
        }
        s
    }
}

/// The formula every generated math block draws.
pub(crate) const MATH_TEX: &str = r"\frac{a}{b}";

pub(crate) fn rendered_blocks() -> impl Strategy<Value = Vec<RenderedBlock>> {
    prop::collection::vec(
        prop_oneof![
            prop::collection::vec(styled_line(), 0..5).prop_map(RenderedBlock::Lines),
            (
                prop::collection::vec(styled_line(), 0..8),
                0usize..20,
                0usize..20
            )
                .prop_map(|(lines, node_count, edge_count)| RenderedBlock::Diagram {
                    lines,
                    node_count,
                    edge_count,
                    kind: DiagramKind::Flowchart,
                    image: None,
                }),
            text().prop_map(|alt| RenderedBlock::Image {
                alt,
                url: "test.png".into()
            }),
            prop::collection::vec(styled_line(), 0..5).prop_map(|lines| RenderedBlock::Math {
                lines,
                image: std::sync::Arc::new(crate::math::MathImage::new(
                    MATH_TEX,
                    crate::theme::Theme::default_theme()
                )),
            }),
        ],
        0..10,
    )
}

#[derive(Debug, Clone)]
pub(crate) enum PagerOp {
    Key(u8),
    Search(String),
    Resize(u16, u16),
    Reload(Vec<RenderedBlock>),
    ReloadDiagram(GraphInput),
    ToggleTwice,
    ImageToggleTwice,
}
pub(crate) fn pager_ops() -> impl Strategy<Value = Vec<PagerOp>> {
    prop::collection::vec(
        prop_oneof![
            8 => (0u8..13).prop_map(PagerOp::Key),
            2 => prop::sample::select(vec!["a", "漢", "node", "", "missing", "IMAGE"]).prop_map(|s| PagerOp::Search(s.into())),
            2 => (width(),0u16..50).prop_map(|(w,h)| PagerOp::Resize(w,h)),
            1 => rendered_blocks().prop_map(PagerOp::Reload),
        1 => graph().prop_map(PagerOp::ReloadDiagram),
            1 => Just(PagerOp::ToggleTwice),
            1 => Just(PagerOp::ImageToggleTwice),
        ],
        0..60,
    )
}
