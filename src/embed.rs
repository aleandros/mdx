use crate::render::{RenderedBlock, StyledLine, StyledSpan};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[allow(dead_code)]
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

#[allow(dead_code)]
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
}
