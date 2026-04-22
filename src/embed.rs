use crate::render::{RenderedBlock, StyledLine, StyledSpan};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub struct EmbedOptions {
    pub width: u16,
    pub height: Option<usize>,
    pub no_color: bool,
}

pub fn run<W: std::io::Write>(
    input: &str,
    opts: EmbedOptions,
    highlighter: &crate::highlight::Highlighter,
    ui_theme: &crate::theme::Theme,
    mermaid_mode: crate::render::MermaidMode,
    writer: &mut W,
) -> anyhow::Result<()> {
    let blocks = crate::parser::parse_markdown(input);
    let rendered =
        crate::render::render_blocks(&blocks, opts.width, highlighter, ui_theme, mermaid_mode);
    let lines = flatten_blocks(&rendered);
    let capped: Box<dyn Iterator<Item = StyledLine>> = match opts.height {
        Some(h) => Box::new(lines.into_iter().take(h)),
        None => Box::new(lines.into_iter()),
    };
    let max_cols = opts.width as usize;
    for line in capped {
        let cut = truncate_line_width(&line, max_cols);
        writeln!(
            writer,
            "{}",
            crate::render::styled_line_to_ansi(&cut, opts.no_color)
        )?;
    }
    Ok(())
}

pub fn flatten_blocks(blocks: &[RenderedBlock]) -> Vec<StyledLine> {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            RenderedBlock::Lines(lines) => {
                for line in lines {
                    out.push(line.clone());
                }
            }
            RenderedBlock::Diagram { lines, .. } => {
                for line in lines {
                    out.push(line.clone());
                }
                out.push(StyledLine::empty());
            }
            RenderedBlock::Image { alt, url } => {
                let text = if alt.is_empty() {
                    format!("[Image]({})", url)
                } else {
                    format!("[Image: {}]({})", alt, url)
                };
                out.push(StyledLine {
                    spans: vec![StyledSpan::plain(text)],
                });
            }
        }
    }
    out
}

pub fn truncate_line_width(line: &StyledLine, max_cols: usize) -> StyledLine {
    if max_cols == 0 {
        return StyledLine::empty();
    }

    let mut acc_cols: usize = 0;
    let mut out_spans: Vec<StyledSpan> = Vec::new();

    for span in &line.spans {
        let span_w = UnicodeWidthStr::width(span.text.as_str());
        if acc_cols + span_w <= max_cols {
            out_spans.push(span.clone());
            acc_cols += span_w;
            if acc_cols == max_cols {
                break;
            }
            continue;
        }

        // Cut inside this span. Walk chars until the next one would overflow.
        let remaining = max_cols - acc_cols;
        let mut taken_cols = 0usize;
        let mut take_bytes = 0usize;
        for (byte_idx, ch) in span.text.char_indices() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if taken_cols + cw > remaining {
                break;
            }
            taken_cols += cw;
            take_bytes = byte_idx + ch.len_utf8();
        }
        if take_bytes > 0 {
            out_spans.push(StyledSpan {
                text: span.text[..take_bytes].to_string(),
                style: span.style.clone(),
            });
        }
        break;
    }

    StyledLine { spans: out_spans }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{StyledLine, StyledSpan};

    fn line_text(line: &StyledLine) -> String {
        line.spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn flatten_lines_preserves_order_and_styles() {
        let block = RenderedBlock::Lines(vec![
            StyledLine {
                spans: vec![StyledSpan::plain("first")],
            },
            StyledLine {
                spans: vec![StyledSpan::plain("second")],
            },
        ]);
        let out = flatten_blocks(&[block]);
        assert_eq!(out.len(), 2);
        assert_eq!(line_text(&out[0]), "first");
        assert_eq!(line_text(&out[1]), "second");
    }

    #[test]
    fn flatten_diagram_appends_trailing_blank() {
        let block = RenderedBlock::Diagram {
            lines: vec![
                StyledLine {
                    spans: vec![StyledSpan::plain("aaa")],
                },
                StyledLine {
                    spans: vec![StyledSpan::plain("bbb")],
                },
            ],
            node_count: 2,
            edge_count: 1,
        };
        let out = flatten_blocks(&[block]);
        assert_eq!(out.len(), 3, "diagram lines + one trailing blank");
        assert_eq!(line_text(&out[0]), "aaa");
        assert_eq!(line_text(&out[1]), "bbb");
        assert!(out[2].spans.is_empty(), "trailing blank is empty");
    }

    #[test]
    fn flatten_image_with_alt() {
        let block = RenderedBlock::Image {
            alt: "diagram".to_string(),
            url: "x.png".to_string(),
        };
        let out = flatten_blocks(&[block]);
        assert_eq!(out.len(), 1);
        assert_eq!(line_text(&out[0]), "[Image: diagram](x.png)");
    }

    #[test]
    fn flatten_image_without_alt() {
        let block = RenderedBlock::Image {
            alt: String::new(),
            url: "x.png".to_string(),
        };
        let out = flatten_blocks(&[block]);
        assert_eq!(line_text(&out[0]), "[Image](x.png)");
    }

    #[test]
    fn flatten_mixed_sequence_preserves_order() {
        let blocks = vec![
            RenderedBlock::Lines(vec![StyledLine {
                spans: vec![StyledSpan::plain("para")],
            }]),
            RenderedBlock::Image {
                alt: "a".to_string(),
                url: "b".to_string(),
            },
            RenderedBlock::Diagram {
                lines: vec![StyledLine {
                    spans: vec![StyledSpan::plain("D1")],
                }],
                node_count: 1,
                edge_count: 0,
            },
        ];
        let out = flatten_blocks(&blocks);
        assert_eq!(out.len(), 4);
        assert_eq!(line_text(&out[0]), "para");
        assert_eq!(line_text(&out[1]), "[Image: a](b)");
        assert_eq!(line_text(&out[2]), "D1");
        assert!(out[3].spans.is_empty());
    }

    #[test]
    fn truncate_ascii_exact_boundary_unchanged() {
        let line = StyledLine {
            spans: vec![StyledSpan::plain("hello")],
        };
        let out = truncate_line_width(&line, 5);
        assert_eq!(line_text(&out), "hello");
    }

    #[test]
    fn truncate_ascii_below_boundary_cuts_mid_span() {
        let line = StyledLine {
            spans: vec![
                StyledSpan::plain("ab"),
                StyledSpan {
                    text: "cdef".to_string(),
                    style: crate::render::SpanStyle {
                        bold: true,
                        ..Default::default()
                    },
                },
            ],
        };
        let out = truncate_line_width(&line, 4);
        assert_eq!(line_text(&out), "abcd");
        // Style of the cut span is preserved on the surviving prefix.
        assert!(out.spans[1].style.bold);
    }

    #[test]
    fn truncate_cjk_before_wide_char_on_boundary() {
        // "a漢b" has display width 1+2+1=4. Truncating to 2 must cut BEFORE "漢"
        // (cutting through a double-width glyph is not allowed), yielding "a"
        // with display width 1.
        let line = StyledLine {
            spans: vec![StyledSpan::plain("a漢b")],
        };
        let out = truncate_line_width(&line, 2);
        assert_eq!(line_text(&out), "a");
    }

    #[test]
    fn truncate_empty_line_stays_empty() {
        let out = truncate_line_width(&StyledLine::empty(), 10);
        assert!(out.spans.is_empty());
    }

    #[test]
    fn truncate_zero_width_produces_empty_line() {
        let line = StyledLine {
            spans: vec![StyledSpan::plain("anything")],
        };
        let out = truncate_line_width(&line, 0);
        assert!(out.spans.is_empty());
    }

    #[test]
    fn truncate_preserves_line_already_shorter() {
        let line = StyledLine {
            spans: vec![StyledSpan::plain("hi")],
        };
        let out = truncate_line_width(&line, 100);
        assert_eq!(line_text(&out), "hi");
    }

    #[test]
    fn run_emits_only_height_lines_when_capped() {
        let input = "# A\n\nparagraph one\n\nparagraph two\n";
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let theme = crate::theme::Theme::default_theme();
        let opts = EmbedOptions {
            width: 80,
            height: Some(2),
            no_color: true,
        };
        let mut buf: Vec<u8> = Vec::new();
        run(
            input,
            opts,
            &highlighter,
            theme,
            crate::render::MermaidMode::Render,
            &mut buf,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        let line_count = s.matches('\n').count();
        assert_eq!(line_count, 2, "exactly 2 newlines emitted, got: {:?}", s);
    }

    #[test]
    fn run_crops_each_line_to_width() {
        let input = "a very long paragraph that will be wrapped or truncated depending on width";
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let theme = crate::theme::Theme::default_theme();
        let opts = EmbedOptions {
            width: 10,
            height: None,
            no_color: true,
        };
        let mut buf: Vec<u8> = Vec::new();
        run(
            input,
            opts,
            &highlighter,
            theme,
            crate::render::MermaidMode::Render,
            &mut buf,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        for line in s.lines() {
            assert!(
                UnicodeWidthStr::width(line) <= 10,
                "line exceeded width 10: {:?}",
                line
            );
        }
    }

    #[test]
    fn run_respects_no_color_flag() {
        let input = "# Heading\n\nbody text\n";
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let theme = crate::theme::Theme::default_theme();
        let opts = EmbedOptions {
            width: 80,
            height: None,
            no_color: true,
        };
        let mut buf: Vec<u8> = Vec::new();
        run(
            input,
            opts,
            &highlighter,
            theme,
            crate::render::MermaidMode::Render,
            &mut buf,
        )
        .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(!s.contains('\x1b'), "no-color output must not contain ESC");
    }
}
