use crate::mermaid;
use crate::parser::{Block, InlineElement};

// ─── Styled output types ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightYellow,
    BrightCyan,
    BrightMagenta,
    DarkGray,
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpanStyle {
    pub fg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledSpan {
    pub text: String,
    pub style: SpanStyle,
}

impl StyledSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        StyledSpan {
            text: text.into(),
            style: SpanStyle::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
}

impl StyledLine {
    pub fn empty() -> Self {
        StyledLine { spans: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderedBlock {
    Lines(Vec<StyledLine>),
    Diagram {
        lines: Vec<StyledLine>,
        node_count: usize,
        edge_count: usize,
    },
    Image {
        alt: String,
        url: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MermaidMode {
    Render,
    Raw,
    Split,
}

// ─── Rendering helpers ─────────────────────────────────────────────────────

fn color_ansi_code(color: &Color) -> &'static str {
    match color {
        Color::Red => "31",
        Color::Green => "32",
        Color::Yellow => "33",
        Color::Blue => "34",
        Color::Magenta => "35",
        Color::Cyan => "36",
        Color::White => "37",
        Color::BrightYellow => "93",
        Color::BrightCyan => "96",
        Color::BrightMagenta => "95",
        Color::DarkGray => "90",
        Color::Rgb(_, _, _) => unreachable!("Rgb is handled separately in styled_line_to_ansi"),
    }
}

fn render_inline(elem: &InlineElement, theme: &crate::theme::Theme) -> StyledSpan {
    match elem {
        InlineElement::Text(t) => StyledSpan {
            text: t.clone(),
            style: SpanStyle {
                fg: Some(theme.body.clone()),
                ..Default::default()
            },
        },
        InlineElement::Bold(t) => StyledSpan {
            text: t.clone(),
            style: SpanStyle {
                fg: Some(theme.bold.clone()),
                bold: true,
                ..Default::default()
            },
        },
        InlineElement::Italic(t) => StyledSpan {
            text: t.clone(),
            style: SpanStyle {
                fg: Some(theme.italic.clone()),
                italic: true,
                ..Default::default()
            },
        },
        InlineElement::Code(t) => StyledSpan {
            text: t.clone(),
            style: SpanStyle {
                fg: Some(theme.inline_code.clone()),
                dim: true,
                ..Default::default()
            },
        },
        InlineElement::Link { text, url } => StyledSpan {
            text: format!("{} ({})", text, url),
            style: SpanStyle {
                fg: Some(theme.link.clone()),
                ..Default::default()
            },
        },
        InlineElement::SoftBreak => StyledSpan::plain(" "),
    }
}

fn render_inline_elements(content: &[InlineElement], theme: &crate::theme::Theme) -> StyledLine {
    StyledLine {
        spans: content.iter().map(|e| render_inline(e, theme)).collect(),
    }
}

fn render_code_block_lines(
    language: &Option<String>,
    content: &str,
    highlighter: &crate::highlight::Highlighter,
) -> Vec<StyledLine> {
    let mut lines = Vec::new();

    // Optional language label
    if let Some(lang) = language {
        lines.push(StyledLine {
            spans: vec![StyledSpan {
                text: format!("  [{}]", lang),
                style: SpanStyle {
                    dim: true,
                    fg: Some(Color::DarkGray),
                    ..Default::default()
                },
            }],
        });
    }

    // Try syntax highlighting
    if let Some(highlighted) = highlighter.highlight_code(content, language.as_deref()) {
        for spans in highlighted {
            let mut indented = vec![StyledSpan::plain("  ")];
            indented.extend(spans);
            lines.push(StyledLine { spans: indented });
        }
    } else {
        // Fallback: dim monochrome
        for line in content.lines() {
            lines.push(StyledLine {
                spans: vec![StyledSpan {
                    text: format!("  {}", line),
                    style: SpanStyle {
                        dim: true,
                        fg: Some(Color::DarkGray),
                        ..Default::default()
                    },
                }],
            });
        }
    }

    lines
}

// ─── Main rendering entry point ────────────────────────────────────────────

pub fn render_blocks(
    blocks: &[Block],
    width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &crate::theme::Theme,
    mermaid_mode: MermaidMode,
) -> Vec<RenderedBlock> {
    let mut out = Vec::new();

    for block in blocks {
        match block {
            Block::Header { level, content } => {
                let color = theme.heading[(*level as usize - 1).min(5)].clone();
                let prefix = "#".repeat(*level as usize);
                // Build the header text from inline elements (plain text)
                let text: String = content
                    .iter()
                    .map(|e| match e {
                        InlineElement::Text(t)
                        | InlineElement::Bold(t)
                        | InlineElement::Italic(t)
                        | InlineElement::Code(t) => t.clone(),
                        InlineElement::Link { text, .. } => text.clone(),
                        InlineElement::SoftBreak => " ".to_string(),
                    })
                    .collect();

                let header_line = StyledLine {
                    spans: vec![StyledSpan {
                        text: format!("{} {}", prefix, text),
                        style: SpanStyle {
                            fg: Some(color),
                            bold: true,
                            ..Default::default()
                        },
                    }],
                };
                out.push(RenderedBlock::Lines(vec![header_line, StyledLine::empty()]));
            }

            Block::Paragraph { content } => {
                let line = render_inline_elements(content, theme);
                out.push(RenderedBlock::Lines(vec![line, StyledLine::empty()]));
            }

            Block::CodeBlock { language, content } => {
                let lines = render_code_block_lines(language, content, highlighter);
                let mut all_lines = lines;
                all_lines.push(StyledLine::empty());
                out.push(RenderedBlock::Lines(all_lines));
            }

            Block::MermaidBlock { content } => {
                let render_as_code = || {
                    RenderedBlock::Lines({
                        let mut lines = render_code_block_lines(
                            &Some("mermaid".to_string()),
                            content,
                            highlighter,
                        );
                        lines.push(StyledLine::empty());
                        lines
                    })
                };

                match mermaid_mode {
                    MermaidMode::Raw => {
                        out.push(render_as_code());
                    }
                    MermaidMode::Render => match mermaid::render_mermaid(content, theme) {
                        Ok((lines, node_count, edge_count)) => {
                            out.push(RenderedBlock::Diagram {
                                lines,
                                node_count,
                                edge_count,
                            });
                        }
                        Err(_) => {
                            let warning_line = StyledLine {
                                spans: vec![StyledSpan {
                                    text: "[mermaid: parse error]".to_string(),
                                    style: SpanStyle {
                                        fg: Some(Color::Red),
                                        ..Default::default()
                                    },
                                }],
                            };
                            let code_lines = render_code_block_lines(&None, content, highlighter);
                            let mut all_lines = vec![warning_line];
                            all_lines.extend(code_lines);
                            all_lines.push(StyledLine::empty());
                            out.push(RenderedBlock::Lines(all_lines));
                        }
                    },
                    MermaidMode::Split => {
                        out.push(render_as_code());
                        match mermaid::render_mermaid(content, theme) {
                            Ok((lines, node_count, edge_count)) => {
                                out.push(RenderedBlock::Diagram {
                                    lines,
                                    node_count,
                                    edge_count,
                                });
                            }
                            Err(_) => {
                                let warning_line = StyledLine {
                                    spans: vec![StyledSpan {
                                        text: "[mermaid: parse error]".to_string(),
                                        style: SpanStyle {
                                            fg: Some(Color::Red),
                                            ..Default::default()
                                        },
                                    }],
                                };
                                out.push(RenderedBlock::Lines(vec![
                                    warning_line,
                                    StyledLine::empty(),
                                ]));
                            }
                        }
                    }
                }
            }

            Block::List { ordered, items } => {
                let mut lines = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let prefix = if *ordered {
                        format!("  {}. ", i + 1)
                    } else {
                        "  * ".to_string()
                    };
                    let mut spans = vec![StyledSpan::plain(prefix)];
                    spans.extend(item.iter().map(|e| render_inline(e, theme)));
                    lines.push(StyledLine { spans });
                }
                lines.push(StyledLine::empty());
                out.push(RenderedBlock::Lines(lines));
            }

            Block::HorizontalRule => {
                let rule_char = '─';
                let rule_text: String = std::iter::repeat_n(rule_char, width as usize).collect();
                let rule_line = StyledLine {
                    spans: vec![StyledSpan {
                        text: rule_text,
                        style: SpanStyle {
                            fg: Some(theme.horizontal_rule.clone()),
                            dim: true,
                            ..Default::default()
                        },
                    }],
                };
                out.push(RenderedBlock::Lines(vec![rule_line]));
            }

            Block::Image { alt, url } => {
                out.push(RenderedBlock::Image {
                    alt: alt.clone(),
                    url: url.clone(),
                });
            }
        }
    }

    out
}

// ─── ANSI output ──────────────────────────────────────────────────────────

pub fn styled_line_to_ansi(line: &StyledLine, no_color: bool) -> String {
    if no_color {
        return line.spans.iter().map(|s| s.text.as_str()).collect();
    }

    let mut result = String::new();
    for span in &line.spans {
        let style = &span.style;
        let mut codes: Vec<String> = Vec::new();

        if style.bold {
            codes.push("1".to_string());
        }
        if style.italic {
            codes.push("3".to_string());
        }
        if style.dim {
            codes.push("2".to_string());
        }
        if let Some(ref color) = style.fg {
            match color {
                Color::Rgb(r, g, b) => codes.push(format!("38;2;{};{};{}", r, g, b)),
                other => codes.push(color_ansi_code(other).to_string()),
            }
        }

        if codes.is_empty() {
            result.push_str(&span.text);
        } else {
            let code_str = codes.join(";");
            result.push_str(&format!("\x1b[{}m{}\x1b[0m", code_str, span.text));
        }
    }
    result
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Block;

    fn test_theme() -> &'static crate::theme::Theme {
        crate::theme::Theme::default_theme()
    }

    #[test]
    fn test_render_header() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::Header {
            level: 1,
            content: vec![InlineElement::Text("Title".to_string())],
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            let first_line = &lines[0];
            assert!(!first_line.spans.is_empty());
            let span = &first_line.spans[0];
            assert!(span.style.bold, "Header should be bold");
            assert!(span.text.contains("Title"), "Header should contain 'Title'");
        } else {
            panic!("Expected Lines variant");
        }
    }

    #[test]
    fn test_render_paragraph_with_bold() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::Paragraph {
            content: vec![
                InlineElement::Text("Hello ".to_string()),
                InlineElement::Bold("world".to_string()),
            ],
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            let first_line = &lines[0];
            let bold_span = first_line.spans.iter().find(|s| s.style.bold);
            assert!(bold_span.is_some(), "Should have a bold span");
            assert_eq!(bold_span.unwrap().text, "world");
        } else {
            panic!("Expected Lines variant");
        }
    }

    #[test]
    fn test_render_code_block() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::CodeBlock {
            language: Some("rust".to_string()),
            content: "fn main() {}".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            let code_line = lines.iter().find(|l| {
                l.spans
                    .iter()
                    .any(|s| s.text.contains("fn") || s.text.contains("main"))
            });
            assert!(code_line.is_some(), "Should have code line with text");
        } else {
            panic!("Expected Lines variant");
        }
    }

    #[test]
    fn test_render_horizontal_rule() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::HorizontalRule];
        let rendered = render_blocks(&blocks, 40, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            let rule_line = &lines[0];
            assert!(!rule_line.spans.is_empty());
            let span = &rule_line.spans[0];
            assert!(span.text.contains('─'), "Should contain rule character");
            assert!(span.text.len() >= 40, "Rule should be at least width chars");
            assert!(span.style.dim, "Rule should be dim");
        } else {
            panic!("Expected Lines variant");
        }
    }

    #[test]
    fn test_render_list() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::List {
            ordered: false,
            items: vec![
                vec![InlineElement::Text("Alpha".to_string())],
                vec![InlineElement::Text("Beta".to_string())],
            ],
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            // First two lines are the items, last is blank
            let item_lines: Vec<_> = lines.iter().filter(|l| !l.spans.is_empty()).collect();
            assert_eq!(item_lines.len(), 2);
            for item_line in &item_lines {
                let full_text: String = item_line.spans.iter().map(|s| s.text.as_str()).collect();
                assert!(
                    full_text.contains('*') || full_text.contains('.'),
                    "Item should contain bullet"
                );
            }
            let alpha_line = item_lines[0];
            let alpha_text: String = alpha_line.spans.iter().map(|s| s.text.as_str()).collect();
            assert!(alpha_text.contains("Alpha"));
        } else {
            panic!("Expected Lines variant");
        }
    }

    #[test]
    fn test_render_mermaid_block() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::MermaidBlock {
            content: "graph TD\n    A --> B\n".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Diagram {
            node_count,
            edge_count,
            ..
        } = &rendered[0]
        {
            assert_eq!(*node_count, 2, "Should have 2 nodes");
            assert_eq!(*edge_count, 1, "Should have 1 edge");
        } else {
            panic!("Expected Diagram variant, got Lines (mermaid parse may have failed)");
        }
    }

    #[test]
    fn test_render_malformed_mermaid_falls_back() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::MermaidBlock {
            content: "THIS IS NOT VALID MERMAID @@@@".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        assert!(
            matches!(rendered[0], RenderedBlock::Lines(_)),
            "Malformed mermaid should fall back to Lines"
        );
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            let has_error = lines.iter().any(|l| {
                l.spans
                    .iter()
                    .any(|s| s.text.contains("mermaid") && s.style.fg == Some(Color::Red))
            });
            assert!(has_error, "Should have error warning span");
        }
    }

    #[test]
    fn test_render_code_block_with_highlighting() {
        use crate::highlight::Highlighter;

        let highlighter = Highlighter::new(None).unwrap();
        let blocks = vec![Block::CodeBlock {
            language: Some("rust".to_string()),
            content: "fn main() {}\n".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Lines(lines) = &rendered[0] {
            // Should have language label + code line(s) + blank line
            // The code lines should have some colored spans (not all DarkGray dim)
            let code_lines: Vec<_> = lines
                .iter()
                .filter(|l| {
                    l.spans
                        .iter()
                        .any(|s| s.text.contains("fn") || s.text.contains("main"))
                })
                .collect();
            assert!(!code_lines.is_empty(), "Should have code lines");
            let has_non_gray_color = code_lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|s| matches!(s.style.fg, Some(ref c) if *c != Color::DarkGray))
            });
            assert!(
                has_non_gray_color,
                "Highlighted Rust code should have colors beyond DarkGray"
            );
        } else {
            panic!("Expected Lines variant");
        }
    }

    #[test]
    fn test_ansi_output_no_color() {
        let line = StyledLine {
            spans: vec![
                StyledSpan {
                    text: "Hello".to_string(),
                    style: SpanStyle {
                        bold: true,
                        fg: Some(Color::Red),
                        ..Default::default()
                    },
                },
                StyledSpan::plain(" world"),
            ],
        };
        let output = styled_line_to_ansi(&line, true);
        assert!(!output.contains('\x1b'), "Should have no escape codes");
        assert_eq!(output, "Hello world");
    }

    #[test]
    fn test_ansi_output_rgb_color() {
        let line = StyledLine {
            spans: vec![StyledSpan {
                text: "colored".to_string(),
                style: SpanStyle {
                    fg: Some(Color::Rgb(255, 100, 50)),
                    ..Default::default()
                },
            }],
        };
        let output = styled_line_to_ansi(&line, false);
        assert!(
            output.contains("\x1b[38;2;255;100;50m"),
            "Should use 24-bit color escape: {}",
            output
        );
        assert!(output.contains("colored"));
    }

    #[test]
    fn test_ansi_output_with_color() {
        let line = StyledLine {
            spans: vec![StyledSpan {
                text: "Bold".to_string(),
                style: SpanStyle {
                    bold: true,
                    fg: Some(Color::Green),
                    ..Default::default()
                },
            }],
        };
        let output = styled_line_to_ansi(&line, false);
        assert!(output.contains('\x1b'), "Should contain escape codes");
        assert!(output.contains("Bold"), "Should contain the text");
    }

    #[test]
    fn test_mermaid_raw_mode_produces_code_block() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::MermaidBlock {
            content: "graph TD\n    A --> B\n".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Raw);
        assert_eq!(rendered.len(), 1);
        assert!(
            matches!(rendered[0], RenderedBlock::Lines(_)),
            "Raw mode should produce Lines, not Diagram"
        );
    }

    #[test]
    fn test_mermaid_split_mode_produces_both() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::MermaidBlock {
            content: "graph TD\n    A --> B\n".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Split);
        assert_eq!(rendered.len(), 2, "Split mode should produce 2 blocks");
        assert!(
            matches!(rendered[0], RenderedBlock::Lines(_)),
            "First block should be code (Lines)"
        );
        assert!(
            matches!(rendered[1], RenderedBlock::Diagram { .. }),
            "Second block should be Diagram"
        );
    }

    #[test]
    fn test_mermaid_render_mode_is_default_behavior() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::MermaidBlock {
            content: "graph TD\n    A --> B\n".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        assert!(matches!(rendered[0], RenderedBlock::Diagram { .. }));
    }

    #[test]
    fn test_render_image_block() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let blocks = vec![Block::Image {
            alt: "A photo".to_string(),
            url: "photo.png".to_string(),
        }];
        let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
        assert_eq!(rendered.len(), 1);
        if let RenderedBlock::Image { alt, url } = &rendered[0] {
            assert_eq!(alt, "A photo");
            assert_eq!(url, "photo.png");
        } else {
            panic!("Expected Image variant, got {:?}", rendered[0]);
        }
    }
}
