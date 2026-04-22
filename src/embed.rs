use crate::render::{RenderedBlock, StyledLine, StyledSpan};

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
}
