use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq)]
pub enum InlineElement {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link { text: String, url: String },
    SoftBreak,
}

#[derive(Debug, Clone, PartialEq)]
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

    // Image tracking state
    let mut in_image: Option<String> = None; // stores the URL while collecting alt text
    let mut image_alt_buf = String::new();

    // For list item accumulation
    let mut list_item_buf: Vec<InlineElement> = Vec::new();
    let mut in_list_item = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let lvl = heading_level_to_u8(level);
                container_stack.push(Container::Header(lvl));
                inline_buf.clear();
                style_stack.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(Container::Header(level)) = container_stack.pop() {
                    blocks.push(Block::Header {
                        level,
                        content: std::mem::take(&mut inline_buf),
                    });
                }
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
                let target_buf = if in_list_item {
                    &mut list_item_buf
                } else {
                    &mut inline_buf
                };
                target_buf.push(InlineElement::Code(text.to_string()));
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
