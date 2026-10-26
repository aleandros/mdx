use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineElement {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    // Lists hold inline elements, so keep raw blocks here until rendering the item.
    CodeBlock(String),
    Link { text: String, url: String },
    SoftBreak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Block {
    Header {
        level: u8,
        content: Vec<InlineElement>,
    },
    Paragraph {
        content: Vec<InlineElement>,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    MermaidBlock {
        content: String,
    },
    List {
        ordered: bool,
        items: Vec<Vec<InlineElement>>,
    },
    HorizontalRule,
    Image {
        alt: String,
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Style {
    Bold,
    Italic,
    Link(String), // url
}

pub fn parse_markdown(input: &str) -> Vec<Block> {
    let parser = Parser::new_ext(input, Options::all());

    let mut blocks: Vec<Block> = Vec::new();

    // Current container state
    enum Container {
        Header(u8),
        Paragraph,
        List {
            ordered: bool,
            items: Vec<Vec<InlineElement>>,
        },
    }

    let mut container_stack: Vec<Container> = Vec::new();
    let mut inline_buf: Vec<InlineElement> = Vec::new();
    let mut style_stack: Vec<Style> = Vec::new();

    // Temporary storage for code block accumulation
    let mut in_code_block: Option<(Option<String>, bool)> = None; // (language, is_mermaid)
    let mut code_buf = String::new();
    let mut html_buf = String::new();

    // Image tracking state
    let mut in_image: Option<String> = None; // stores the URL while collecting alt text
    let mut image_alt_buf = String::new();

    // For list item accumulation
    let mut list_item_buf: Vec<InlineElement> = Vec::new();
    let mut in_list_item = false;

    for event in parser {
        // Normalize text-like events so they share the current container routing,
        // including image alt text and list items.
        let event = match event {
            Event::InlineMath(text) => {
                // Like inline code, inline math must occupy one terminal line.
                let text = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
                Event::Code(format!("${text}$").into())
            }
            Event::InlineHtml(text) => Event::Text(text),
            Event::FootnoteReference(label) => Event::Text(format!("[^{label}]").into()),
            other => other,
        };
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = heading_level_to_u8(level);
                container_stack.push(Container::Header(lvl));
                inline_buf.clear();
                style_stack.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(Container::Header(level)) = container_stack.pop()
                    && !inline_buf.is_empty()
                {
                    blocks.push(Block::Header {
                        level,
                        content: std::mem::take(&mut inline_buf),
                    });
                }
            }

            Event::Start(Tag::TableHead | Tag::TableRow) => {
                inline_buf.clear();
                style_stack.clear();
            }
            Event::End(TagEnd::TableCell) => inline_buf.push(InlineElement::SoftBreak),
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                blocks.push(Block::Paragraph {
                    content: std::mem::take(&mut inline_buf),
                });
            }
            Event::Start(Tag::Paragraph) => {
                container_stack.push(Container::Paragraph);
                inline_buf.clear();
                style_stack.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                if let Some(Container::Paragraph) = container_stack.pop() {
                    let content = std::mem::take(&mut inline_buf);
                    if !content.is_empty() {
                        blocks.push(Block::Paragraph { content });
                    }
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let (language, is_mermaid) = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang_str = lang.to_string();
                        if lang_str == "mermaid" {
                            (None, true)
                        } else if lang_str.is_empty() {
                            (None, false)
                        } else {
                            (Some(lang_str), false)
                        }
                    }
                    CodeBlockKind::Indented => (None, false),
                };
                in_code_block = Some((language, is_mermaid));
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, is_mermaid)) = in_code_block.take() {
                    let content = std::mem::take(&mut code_buf);
                    if is_mermaid {
                        blocks.push(Block::MermaidBlock { content });
                    } else {
                        blocks.push(Block::CodeBlock { language, content });
                    }
                }
            }

            Event::Start(Tag::List(start)) => {
                let ordered = start.is_some();
                container_stack.push(Container::List {
                    ordered,
                    items: Vec::new(),
                });
            }
            Event::End(TagEnd::List(_)) => {
                // Pop any remaining list item
                if in_list_item {
                    let item = std::mem::take(&mut list_item_buf);
                    in_list_item = false;
                    if let Some(Container::List { items, .. }) = container_stack.last_mut() {
                        items.push(item);
                    }
                }
                if let Some(Container::List { ordered, items }) = container_stack.pop() {
                    blocks.push(Block::List { ordered, items });
                }
            }

            Event::Start(Tag::Item) => {
                in_list_item = true;
                list_item_buf.clear();
                style_stack.clear();
            }
            Event::End(TagEnd::Item) if in_list_item => {
                let item = std::mem::take(&mut list_item_buf);
                in_list_item = false;
                if let Some(Container::List { items, .. }) = container_stack.last_mut() {
                    items.push(item);
                }
            }
            Event::End(TagEnd::Item) => {}

            Event::Start(Tag::Strong) => {
                style_stack.push(Style::Bold);
            }
            Event::End(TagEnd::Strong) => {
                style_stack.pop();
            }

            Event::Start(Tag::Emphasis) => {
                style_stack.push(Style::Italic);
            }
            Event::End(TagEnd::Emphasis) => {
                style_stack.pop();
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                style_stack.push(Style::Link(dest_url.to_string()));
            }
            Event::End(TagEnd::Link) => {
                // Find the link style entry to get the URL
                let url = style_stack
                    .iter()
                    .rev()
                    .find_map(|s| {
                        if let Style::Link(u) = s {
                            Some(u.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                style_stack.retain(|s| !matches!(s, Style::Link(_)));

                // The text accumulated since Start(Link) should be rewritten as a Link element.
                // We look for the last Text element added and convert it.
                let target_buf = if in_list_item {
                    &mut list_item_buf
                } else {
                    &mut inline_buf
                };
                // Collect all Text elements added since the link started into one string
                // (they were pushed as Text elements during the link body)
                // Strategy: pop trailing Text/SoftBreak elements and combine into link text
                let mut link_text = String::new();
                // Walk backwards gathering plain Text items (not Bold/Italic/Code)
                while let Some(last) = target_buf.last() {
                    match last {
                        InlineElement::Text(t) => {
                            link_text.insert_str(0, t);
                            target_buf.pop();
                        }
                        _ => break,
                    }
                }
                target_buf.push(InlineElement::Link {
                    text: link_text,
                    url,
                });
            }

            Event::Text(text) => {
                let text_str = text.to_string();

                if in_image.is_some() {
                    image_alt_buf.push_str(&text_str);
                } else if in_code_block.is_some() {
                    code_buf.push_str(&text_str);
                } else {
                    let target_buf = if in_list_item {
                        &mut list_item_buf
                    } else {
                        &mut inline_buf
                    };
                    let elem = match style_stack.last() {
                        Some(Style::Bold) => InlineElement::Bold(text_str),
                        Some(Style::Italic) => InlineElement::Italic(text_str),
                        Some(Style::Link(_)) => InlineElement::Text(text_str),
                        None => InlineElement::Text(text_str),
                    };
                    target_buf.push(elem);
                }
            }

            Event::Code(text) => {
                if in_image.is_some() {
                    image_alt_buf.push_str(&text);
                    continue;
                }
                let target_buf = if in_list_item {
                    &mut list_item_buf
                } else {
                    &mut inline_buf
                };
                target_buf.push(InlineElement::Code(text.to_string()));
            }

            Event::DisplayMath(text) => {
                let source = format!("$${text}$$");
                if in_image.is_some() {
                    image_alt_buf.push_str(&source);
                } else if in_list_item {
                    list_item_buf.push(InlineElement::CodeBlock(source));
                } else {
                    // Display math can occur mid-paragraph or mid-heading. Flush
                    // preceding text before emitting the block to keep source order.
                    if !inline_buf.is_empty() {
                        let content = std::mem::take(&mut inline_buf);
                        blocks.push(match container_stack.last() {
                            Some(Container::Header(level)) => Block::Header {
                                level: *level,
                                content,
                            },
                            _ => Block::Paragraph { content },
                        });
                    }
                    blocks.push(Block::CodeBlock {
                        language: None,
                        content: source,
                    });
                }
            }

            Event::Start(Tag::HtmlBlock) => html_buf.clear(),
            Event::Html(text) => html_buf.push_str(&text),
            Event::End(TagEnd::HtmlBlock) => {
                // One HTML block can contain several Html events. Keep them
                // contiguous and in the same container as the surrounding text.
                let content = std::mem::take(&mut html_buf);
                if in_list_item {
                    list_item_buf.push(InlineElement::CodeBlock(content));
                } else {
                    blocks.push(Block::CodeBlock {
                        language: None,
                        content,
                    });
                }
            }

            Event::SoftBreak => {
                let target_buf = if in_list_item {
                    &mut list_item_buf
                } else {
                    &mut inline_buf
                };
                if in_code_block.is_none() {
                    target_buf.push(InlineElement::SoftBreak);
                }
            }

            Event::Rule => {
                blocks.push(Block::HorizontalRule);
            }

            Event::Start(Tag::Image { dest_url, .. }) => {
                in_image = Some(dest_url.to_string());
                image_alt_buf.clear();
            }
            Event::End(TagEnd::Image) => {
                if let Some(url) = in_image.take() {
                    blocks.push(Block::Image {
                        alt: std::mem::take(&mut image_alt_buf),
                        url,
                    });
                }
            }

            // Intentionally ignore unsupported structural Start/End tags
            // (block quotes, table/table-cell wrappers, strikethrough, footnote
            // definitions, metadata and definition lists), HardBreak and
            // TaskListMarker. All text-carrying events are handled above.
            _ => {}
        }
    }

    blocks
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_math_preserves_delimiters_and_position() {
        assert_eq!(
            parse_markdown("Before $a^2$ after."),
            vec![Block::Paragraph {
                content: vec![
                    InlineElement::Text("Before ".into()),
                    InlineElement::Code("$a^2$".into()),
                    InlineElement::Text(" after.".into())
                ],
            }]
        );
    }

    #[test]
    fn test_display_math_is_a_separate_unhighlighted_block() {
        assert_eq!(
            parse_markdown("Before\n\n$$\na^2 + b^2 = c^2\n$$\n\nAfter"),
            vec![
                Block::Paragraph {
                    content: vec![InlineElement::Text("Before".into())]
                },
                Block::CodeBlock {
                    language: None,
                    content: "$$\na^2 + b^2 = c^2\n$$".into()
                },
                Block::Paragraph {
                    content: vec![InlineElement::Text("After".into())]
                },
            ]
        );
    }

    #[test]
    fn test_display_math_splits_paragraph_in_source_order() {
        assert_eq!(
            parse_markdown("Before $$a^2$$ after."),
            vec![
                Block::Paragraph {
                    content: vec![InlineElement::Text("Before ".into())]
                },
                Block::CodeBlock {
                    language: None,
                    content: "$$a^2$$".into()
                },
                Block::Paragraph {
                    content: vec![InlineElement::Text(" after.".into())]
                },
            ]
        );
    }

    #[test]
    fn test_math_in_list_items_and_headings() {
        assert_eq!(
            parse_markdown("# Value $a^2$\n\n- First $b^2$\n- Second $c^2$"),
            vec![
                Block::Header {
                    level: 1,
                    content: vec![
                        InlineElement::Text("Value ".into()),
                        InlineElement::Code("$a^2$".into())
                    ]
                },
                Block::List {
                    ordered: false,
                    items: vec![
                        vec![
                            InlineElement::Text("First ".into()),
                            InlineElement::Code("$b^2$".into())
                        ],
                        vec![
                            InlineElement::Text("Second ".into()),
                            InlineElement::Code("$c^2$".into())
                        ],
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_display_math_in_list_and_heading_keeps_source_order() {
        assert_eq!(
            parse_markdown("# Before $$a^2$$ after\n\n1. Before $$b^2$$ after\n2. Next"),
            vec![
                Block::Header {
                    level: 1,
                    content: vec![InlineElement::Text("Before ".into())]
                },
                Block::CodeBlock {
                    language: None,
                    content: "$$a^2$$".into()
                },
                Block::Header {
                    level: 1,
                    content: vec![InlineElement::Text(" after".into())]
                },
                Block::List {
                    ordered: true,
                    items: vec![
                        vec![
                            InlineElement::Text("Before ".into()),
                            InlineElement::CodeBlock("$$b^2$$".into()),
                            InlineElement::Text(" after".into())
                        ],
                        vec![InlineElement::Text("Next".into())],
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_math_in_loose_list_and_image_alt() {
        let blocks = parse_markdown("- $a^2$\n\n- $b^2$\n\n![Value $x$ and $$y$$](plot.png)");
        assert_eq!(
            blocks,
            vec![
                Block::List {
                    ordered: false,
                    items: vec![
                        vec![InlineElement::Code("$a^2$".into())],
                        vec![InlineElement::Code("$b^2$".into())],
                    ]
                },
                Block::Image {
                    alt: "Value $x$ and $$y$$".into(),
                    url: "plot.png".into()
                },
            ]
        );
    }

    #[test]
    fn test_other_text_events_have_raw_fallbacks() {
        let blocks = parse_markdown(
            "<div>raw HTML</div>\n\nA <span>word</span>[^note].\n\n[^note]: Footnote body",
        );
        assert_eq!(
            blocks,
            vec![
                Block::CodeBlock {
                    language: None,
                    content: "<div>raw HTML</div>\n".into()
                },
                Block::Paragraph {
                    content: vec![
                        InlineElement::Text("A ".into()),
                        InlineElement::Text("<span>".into()),
                        InlineElement::Text("word".into()),
                        InlineElement::Text("</span>".into()),
                        InlineElement::Text("[^note]".into()),
                        InlineElement::Text(".".into()),
                    ]
                },
                Block::Paragraph {
                    content: vec![InlineElement::Text("Footnote body".into())]
                },
            ]
        );
    }

    #[test]
    fn test_inline_math_normalizes_line_endings_like_inline_code() {
        for input in ["Before $x\ny$ after", "Before $x\r\ny$ after"] {
            assert_eq!(
                parse_markdown(input),
                vec![Block::Paragraph {
                    content: vec![
                        InlineElement::Text("Before ".into()),
                        InlineElement::Code("$x y$".into()),
                        InlineElement::Text(" after".into()),
                    ]
                }]
            );
        }
    }

    #[test]
    fn test_raw_html_stays_inside_its_list_item() {
        assert_eq!(
            parse_markdown("- Before\n\n  <div>\n  HTML\n  </div>\n\n  After\n"),
            vec![Block::List {
                ordered: false,
                items: vec![vec![
                    InlineElement::Text("Before".into()),
                    InlineElement::CodeBlock("<div>\nHTML\n</div>\n".into()),
                    InlineElement::Text("After".into()),
                ]]
            },]
        );
    }

    #[test]
    fn test_multiline_html_is_one_raw_block() {
        assert_eq!(
            parse_markdown("<div>\nHTML\n</div>\n"),
            vec![Block::CodeBlock {
                language: None,
                content: "<div>\nHTML\n</div>\n".into(),
            }]
        );
    }

    #[test]
    fn test_math_currency_signs_preserve_text() {
        // pulldown-cmark 0.12 leaves this currency example as ordinary text.
        let blocks = parse_markdown("$5 and $10");
        let [Block::Paragraph { content }] = blocks.as_slice() else {
            panic!("{blocks:?}")
        };
        let text: String = content
            .iter()
            .map(|element| match element {
                InlineElement::Text(text) | InlineElement::Code(text) => text.as_str(),
                _ => panic!("Unexpected currency element: {element:?}"),
            })
            .collect();
        assert_eq!(text, "$5 and $10");
    }

    #[test]
    fn test_parse_header() {
        let blocks = parse_markdown("# Hello World");
        assert_eq!(
            blocks,
            vec![Block::Header {
                level: 1,
                content: vec![InlineElement::Text("Hello World".to_string())],
            }]
        );
    }

    #[test]
    fn test_parse_paragraph_with_inline() {
        let blocks = parse_markdown("Hello **bold** and *italic* text");
        assert_eq!(blocks.len(), 1);
        if let Block::Paragraph { content } = &blocks[0] {
            assert!(content.contains(&InlineElement::Text("Hello ".to_string())));
            assert!(content.contains(&InlineElement::Bold("bold".to_string())));
            assert!(content.contains(&InlineElement::Text(" and ".to_string())));
            assert!(content.contains(&InlineElement::Italic("italic".to_string())));
            assert!(content.contains(&InlineElement::Text(" text".to_string())));
        } else {
            panic!("Expected Paragraph, got {:?}", blocks[0]);
        }
    }

    #[test]
    fn test_parse_code_block() {
        let blocks = parse_markdown("```rust\nfn main() {}\n```");
        assert_eq!(
            blocks,
            vec![Block::CodeBlock {
                language: Some("rust".to_string()),
                content: "fn main() {}\n".to_string(),
            }]
        );
    }

    #[test]
    fn test_parse_mermaid_block() {
        let blocks = parse_markdown("```mermaid\ngraph TD\n    A --> B\n```");
        assert_eq!(
            blocks,
            vec![Block::MermaidBlock {
                content: "graph TD\n    A --> B\n".to_string(),
            }]
        );
    }

    #[test]
    fn test_parse_unordered_list() {
        let blocks = parse_markdown("- Item 1\n- Item 2");
        assert_eq!(blocks.len(), 1);
        if let Block::List { ordered, items } = &blocks[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], vec![InlineElement::Text("Item 1".to_string())]);
            assert_eq!(items[1], vec![InlineElement::Text("Item 2".to_string())]);
        } else {
            panic!("Expected List, got {:?}", blocks[0]);
        }
    }

    #[test]
    fn test_parse_horizontal_rule() {
        let blocks = parse_markdown("---");
        assert_eq!(blocks, vec![Block::HorizontalRule]);
    }

    #[test]
    fn test_parse_inline_code() {
        let blocks = parse_markdown("Use `code` here");
        assert_eq!(blocks.len(), 1);
        if let Block::Paragraph { content } = &blocks[0] {
            assert!(content.contains(&InlineElement::Code("code".to_string())));
        } else {
            panic!("Expected Paragraph, got {:?}", blocks[0]);
        }
    }

    #[test]
    fn test_parse_link() {
        let blocks = parse_markdown("[click](http://example.com)");
        assert_eq!(blocks.len(), 1);
        if let Block::Paragraph { content } = &blocks[0] {
            assert!(content.contains(&InlineElement::Link {
                text: "click".to_string(),
                url: "http://example.com".to_string(),
            }));
        } else {
            panic!("Expected Paragraph, got {:?}", blocks[0]);
        }
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let input = "# Title\n\nSome paragraph.\n\n---\n\n```rust\nlet x = 1;\n```";
        let blocks = parse_markdown(input);
        assert_eq!(blocks.len(), 4);
        assert!(matches!(blocks[0], Block::Header { level: 1, .. }));
        assert!(matches!(blocks[1], Block::Paragraph { .. }));
        assert!(matches!(blocks[2], Block::HorizontalRule));
        assert!(matches!(
            blocks[3],
            Block::CodeBlock {
                language: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn test_parse_image() {
        let blocks = parse_markdown("![Alt text](image.png)");
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0],
            Block::Image {
                alt: "Alt text".to_string(),
                url: "image.png".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_image_with_url() {
        let blocks = parse_markdown("![Logo](https://example.com/logo.png)");
        assert_eq!(blocks.len(), 1);
        if let Block::Image { alt, url } = &blocks[0] {
            assert_eq!(alt, "Logo");
            assert_eq!(url, "https://example.com/logo.png");
        } else {
            panic!("Expected Image block");
        }
    }

    #[test]
    fn test_parse_image_empty_alt() {
        let blocks = parse_markdown("![](photo.jpg)");
        assert_eq!(blocks.len(), 1);
        if let Block::Image { alt, url } = &blocks[0] {
            assert_eq!(alt, "");
            assert_eq!(url, "photo.jpg");
        } else {
            panic!("Expected Image block");
        }
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use crate::strategies::{self, significant};
    use proptest::prelude::*;
    fn payload(blocks: &[Block]) -> String {
        fn inline(content: &[InlineElement]) -> String {
            content
                .iter()
                .map(|e| match e {
                    InlineElement::Text(s)
                    | InlineElement::Bold(s)
                    | InlineElement::Italic(s)
                    | InlineElement::Code(s)
                    | InlineElement::CodeBlock(s)
                    | InlineElement::Link { text: s, .. } => s.as_str(),
                    InlineElement::SoftBreak => " ",
                })
                .collect()
        }
        blocks
            .iter()
            .map(|b| match b {
                Block::Header { content, .. } | Block::Paragraph { content } => inline(content),
                Block::List { items, .. } => items.iter().map(|i| inline(i)).collect(),
                Block::CodeBlock { content, .. } | Block::MermaidBlock { content } => {
                    content.clone()
                }
                Block::Image { alt, .. } => alt.clone(),
                Block::HorizontalRule => String::new(),
            })
            .collect()
    }
    proptest! {
        #![proptest_config(strategies::model_config(64))]
        #[test]
        fn property_markdown_retains_payload((source, expected) in strategies::markdown()) {
            prop_assert_eq!(significant(&payload(&parse_markdown(&source))), expected);
        }
        #[test]
        fn property_arbitrary_unicode_never_panics(source in strategies::unicode()) {
            let blocks = parse_markdown(&source);
            prop_assert_eq!(&blocks, &parse_markdown(&source));
        }
    }
}
