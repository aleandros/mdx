use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};

use crate::render::MermaidMode;

#[allow(dead_code)]
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[allow(dead_code)]
fn start_watcher(path: &Path) -> Result<(Box<dyn Watcher + Send>, mpsc::Receiver<()>)> {
    let (tx, rx) = mpsc::channel();

    let tx1 = tx.clone();
    let result = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = tx1.send(());
            }
        },
        Config::default(),
    );

    let mut watcher: Box<dyn Watcher + Send> = match result {
        Ok(w) => Box::new(w),
        Err(_) => {
            let config = Config::default().with_poll_interval(Duration::from_millis(500));
            Box::new(PollWatcher::new(
                move |res: notify::Result<notify::Event>| {
                    if res.is_ok() {
                        let _ = tx.send(());
                    }
                },
                config,
            )?)
        }
    };

    watcher.watch(path, RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

#[allow(dead_code)]
fn read_file_with_retry(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(_) => {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(path).ok()
        }
    }
}

#[allow(dead_code)]
struct MermaidCacheEntry {
    lines: Vec<String>,
    node_count: usize,
    edge_count: usize,
}

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn diff_and_render(
    old_blocks: &[crate::parser::Block],
    old_groups: &[Vec<crate::render::RenderedBlock>],
    new_blocks: &[crate::parser::Block],
    width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &'static crate::theme::Theme,
    mermaid_mode: MermaidMode,
    mermaid_cache: &mut HashMap<String, MermaidCacheEntry>,
) -> Vec<Vec<crate::render::RenderedBlock>> {
    debug_assert_eq!(old_blocks.len(), old_groups.len());

    use crate::parser::Block;
    use crate::render::{Color, RenderedBlock, SpanStyle, StyledLine, StyledSpan};

    let mut result = Vec::with_capacity(new_blocks.len());

    for (i, new_block) in new_blocks.iter().enumerate() {
        // Reuse if unchanged
        if i < old_blocks.len() && old_blocks[i] == *new_block {
            result.push(old_groups[i].clone());
            continue;
        }

        // Re-render this block
        let mut group = crate::render::render_blocks(
            std::slice::from_ref(new_block),
            width,
            highlighter,
            theme,
            mermaid_mode,
        );

        // Mermaid cache logic
        if let Block::MermaidBlock { content } = new_block {
            let has_diagram = group
                .iter()
                .any(|rb| matches!(rb, RenderedBlock::Diagram { .. }));

            if has_diagram {
                // Update cache with successful render
                for rb in &group {
                    if let RenderedBlock::Diagram {
                        lines,
                        node_count,
                        edge_count,
                    } = rb
                    {
                        mermaid_cache.insert(
                            content.clone(),
                            MermaidCacheEntry {
                                lines: lines.clone(),
                                node_count: *node_count,
                                edge_count: *edge_count,
                            },
                        );
                    }
                }
            } else if let Some(cached) = mermaid_cache.get(content.as_str()) {
                // Render failed — use cached diagram with error indicator
                group = vec![
                    RenderedBlock::Lines(vec![StyledLine {
                        spans: vec![StyledSpan {
                            text: "[mermaid: parse error — showing last good render]".to_string(),
                            style: SpanStyle {
                                fg: Some(Color::Red),
                                ..Default::default()
                            },
                        }],
                    }]),
                    RenderedBlock::Diagram {
                        lines: cached.lines.clone(),
                        node_count: cached.node_count,
                        edge_count: cached.edge_count,
                    },
                ];
            }
            // If no cache and render failed, keep default error output
        }

        result.push(group);
    }

    result
}

#[allow(dead_code)]
fn flatten_groups(
    groups: &[Vec<crate::render::RenderedBlock>],
) -> Vec<crate::render::RenderedBlock> {
    groups.iter().flat_map(|g| g.iter().cloned()).collect()
}

pub fn run_watch(
    _path: &Path,
    _width: u16,
    _highlighter: &crate::highlight::Highlighter,
    _theme: &'static crate::theme::Theme,
    _mermaid_mode: MermaidMode,
) -> Result<()> {
    anyhow::bail!("watch mode not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parser::{Block, InlineElement};
    use crate::render::{self, RenderedBlock};

    fn test_highlighter() -> crate::highlight::Highlighter {
        crate::highlight::Highlighter::new(None).unwrap()
    }

    fn test_theme() -> &'static crate::theme::Theme {
        crate::theme::Theme::default_theme()
    }

    fn render_groups(
        blocks: &[Block],
        h: &crate::highlight::Highlighter,
        theme: &'static crate::theme::Theme,
    ) -> Vec<Vec<RenderedBlock>> {
        blocks
            .iter()
            .map(|b| {
                render::render_blocks(std::slice::from_ref(b), 80, h, theme, MermaidMode::Render)
            })
            .collect()
    }

    #[test]
    fn test_diff_unchanged_blocks_reused() {
        let blocks = vec![
            Block::Header {
                level: 1,
                content: vec![InlineElement::Text("Title".into())],
            },
            Block::Paragraph {
                content: vec![InlineElement::Text("Body".into())],
            },
        ];
        let h = test_highlighter();
        let theme = test_theme();
        let groups = render_groups(&blocks, &h, theme);
        let mut cache = HashMap::new();

        let result = diff_and_render(
            &blocks,
            &groups,
            &blocks,
            80,
            &h,
            theme,
            MermaidMode::Render,
            &mut cache,
        );

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], groups[0]);
        assert_eq!(result[1], groups[1]);
    }

    #[test]
    fn test_diff_changed_block_rerendered() {
        let old_blocks = vec![Block::Header {
            level: 1,
            content: vec![InlineElement::Text("Old".into())],
        }];
        let new_blocks = vec![Block::Header {
            level: 1,
            content: vec![InlineElement::Text("New".into())],
        }];
        let h = test_highlighter();
        let theme = test_theme();
        let old_groups = render_groups(&old_blocks, &h, theme);
        let mut cache = HashMap::new();

        let result = diff_and_render(
            &old_blocks,
            &old_groups,
            &new_blocks,
            80,
            &h,
            theme,
            MermaidMode::Render,
            &mut cache,
        );

        assert_eq!(result.len(), 1);
        assert_ne!(result[0], old_groups[0]);
    }

    #[test]
    fn test_diff_block_added() {
        let old_blocks = vec![Block::Header {
            level: 1,
            content: vec![InlineElement::Text("Title".into())],
        }];
        let new_blocks = vec![
            Block::Header {
                level: 1,
                content: vec![InlineElement::Text("Title".into())],
            },
            Block::Paragraph {
                content: vec![InlineElement::Text("New para".into())],
            },
        ];
        let h = test_highlighter();
        let theme = test_theme();
        let old_groups = render_groups(&old_blocks, &h, theme);
        let mut cache = HashMap::new();

        let result = diff_and_render(
            &old_blocks,
            &old_groups,
            &new_blocks,
            80,
            &h,
            theme,
            MermaidMode::Render,
            &mut cache,
        );

        assert_eq!(result.len(), 2);
        // First block unchanged — should reuse
        assert_eq!(result[0], old_groups[0]);
    }

    #[test]
    fn test_diff_block_removed() {
        let old_blocks = vec![
            Block::Header {
                level: 1,
                content: vec![InlineElement::Text("Title".into())],
            },
            Block::Paragraph {
                content: vec![InlineElement::Text("Gone".into())],
            },
        ];
        let new_blocks = vec![Block::Header {
            level: 1,
            content: vec![InlineElement::Text("Title".into())],
        }];
        let h = test_highlighter();
        let theme = test_theme();
        let old_groups = render_groups(&old_blocks, &h, theme);
        let mut cache = HashMap::new();

        let result = diff_and_render(
            &old_blocks,
            &old_groups,
            &new_blocks,
            80,
            &h,
            theme,
            MermaidMode::Render,
            &mut cache,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], old_groups[0]);
    }

    #[test]
    fn test_diff_mermaid_cache_populated_on_success() {
        let blocks = vec![Block::MermaidBlock {
            content: "graph TD\n    A --> B\n".into(),
        }];
        let h = test_highlighter();
        let theme = test_theme();
        let old_groups: Vec<Vec<RenderedBlock>> = Vec::new();
        let mut cache = HashMap::new();

        diff_and_render(
            &[],
            &old_groups,
            &blocks,
            80,
            &h,
            theme,
            MermaidMode::Render,
            &mut cache,
        );

        assert!(cache.contains_key("graph TD\n    A --> B\n"));
    }

    #[test]
    fn test_flatten_groups() {
        let h = test_highlighter();
        let theme = test_theme();
        let blocks = vec![
            Block::Header {
                level: 1,
                content: vec![InlineElement::Text("A".into())],
            },
            Block::Paragraph {
                content: vec![InlineElement::Text("B".into())],
            },
        ];
        let groups = render_groups(&blocks, &h, theme);
        let flat = flatten_groups(&groups);

        assert!(flat.len() >= 2);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_differs_for_different_content() {
        let h1 = content_hash("hello");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_content_hash_empty() {
        let h1 = content_hash("");
        let h2 = content_hash("x");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_diff_mermaid_cache_fallback_on_error() {
        let h = test_highlighter();
        let theme = test_theme();
        let mut cache = HashMap::new();

        // Pre-populate cache with a known entry
        let content = "INVALID MERMAID CONTENT";
        cache.insert(
            content.to_string(),
            MermaidCacheEntry {
                lines: vec!["cached line".to_string()],
                node_count: 2,
                edge_count: 1,
            },
        );

        // Render block with content that fails to parse but has cache entry
        let blocks = vec![Block::MermaidBlock {
            content: content.into(),
        }];
        let result = diff_and_render(
            &[],
            &[],
            &blocks,
            80,
            &h,
            theme,
            MermaidMode::Render,
            &mut cache,
        );

        // Should fall back to cached diagram + error indicator
        let group = &result[0];
        let has_error = group.iter().any(|rb| {
            if let RenderedBlock::Lines(lines) = rb {
                lines
                    .iter()
                    .any(|l| l.spans.iter().any(|s| s.text.contains("last good render")))
            } else {
                false
            }
        });
        let has_diagram = group
            .iter()
            .any(|rb| matches!(rb, RenderedBlock::Diagram { .. }));
        assert!(has_error, "Should have error indicator line");
        assert!(has_diagram, "Should have cached diagram fallback");
    }
}
