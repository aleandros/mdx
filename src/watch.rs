use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::stdout;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color as RColor, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::pager::{KeyAction, PagerState, TerminalGuard};
use crate::render::{self, MermaidMode, RenderedBlock};

/// How often watch mode checks shown image files for changes.
const IMAGE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

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

    let mut watcher: Box<dyn Watcher + Send> = if let Ok(w) = result {
        Box::new(w)
    } else {
        let config = Config::default().with_poll_interval(Duration::from_millis(500));
        Box::new(PollWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if res.is_ok() {
                    let _ = tx.send(());
                }
            },
            config,
        )?)
    };

    watcher.watch(path, RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

/// Directory relative image paths in `path` are resolved against.
pub(crate) fn base_dir(path: &Path) -> std::path::PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    }
}

fn read_file_with_retry(path: &Path) -> Option<String> {
    if let Ok(content) = std::fs::read_to_string(path) {
        Some(content)
    } else {
        std::thread::sleep(Duration::from_millis(50));
        std::fs::read_to_string(path).ok()
    }
}

/// The last diagram a mermaid block rendered to, shown again while the
/// block's source fails to parse.
type MermaidCacheEntry = crate::render::RenderedBlock;

#[expect(
    clippy::too_many_arguments,
    reason = "watch mode combines input, rendering, and terminal settings"
)]
fn diff_and_render(
    old_blocks: &[crate::parser::Block],
    old_groups: &[Vec<crate::render::RenderedBlock>],
    new_blocks: &[crate::parser::Block],
    width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &'static crate::theme::Theme,
    mermaid_mode: MermaidMode,
    mermaid_cache: &mut HashMap<usize, MermaidCacheEntry>,
) -> Vec<Vec<crate::render::RenderedBlock>> {
    use crate::parser::Block;
    use crate::render::{Color, RenderedBlock, SpanStyle, StyledLine, StyledSpan};

    debug_assert_eq!(old_blocks.len(), old_groups.len());

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
        if let Block::MermaidBlock { .. } = new_block {
            let has_diagram = group
                .iter()
                .any(|rb| matches!(rb, RenderedBlock::Diagram { .. }));

            if has_diagram {
                // Update cache with successful render
                for rb in &group {
                    if matches!(rb, RenderedBlock::Diagram { .. }) {
                        mermaid_cache.insert(i, rb.clone());
                    }
                }
            } else if let Some(cached) = mermaid_cache.get(&i) {
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
                    cached.clone(),
                ];
            }
            // If no cache and render failed, keep default error output
        }

        result.push(group);
    }

    result
}

fn flatten_groups(
    groups: &[Vec<crate::render::RenderedBlock>],
) -> Vec<crate::render::RenderedBlock> {
    groups.iter().flat_map(|g| g.iter().cloned()).collect()
}

struct StatusState {
    filename: String,
    message: Option<String>,
    message_time: Option<Instant>,
}

impl StatusState {
    fn new(filename: &str) -> Self {
        StatusState {
            filename: filename.to_string(),
            message: None,
            message_time: None,
        }
    }

    fn set_message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
        self.message_time = Some(Instant::now());
    }

    fn render(&self) -> Line<'static> {
        let mut spans = vec![
            Span::styled(
                format!(" {} ", self.filename),
                Style::default()
                    .fg(RColor::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("│ watching ", Style::default().fg(RColor::DarkGray)),
        ];

        if let Some(ref msg) = self.message {
            let age = self.message_time.map(|t| t.elapsed()).unwrap_or_default();
            if age < Duration::from_secs(3) {
                spans.push(Span::styled(
                    format!("│ {msg} "),
                    Style::default().fg(RColor::Yellow),
                ));
            }
        }

        Line::from(spans)
    }

    fn tick(&mut self) -> bool {
        if let Some(t) = self.message_time
            && t.elapsed() >= Duration::from_secs(3)
            && self.message.is_some()
        {
            self.message = None;
            self.message_time = None;
            return true;
        }
        false
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "event loop coordinates terminal input, file changes, and redraws"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "watch mode combines input, rendering, and terminal settings"
)]
pub(crate) fn run_watch(
    path: &Path,
    width: u16,
    max_content_width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &'static crate::theme::Theme,
    mermaid_mode: MermaidMode,
    graphics: crate::graphics::GraphicsSetting,
    mermaid_graphics: crate::graphics::MermaidGraphics,
) -> Result<()> {
    // Initial read and render
    let input = std::fs::read_to_string(path)?;
    let mut last_hash = content_hash(&input);
    let mut blocks = crate::parser::parse_markdown(&input);
    let mut rendered_groups: Vec<Vec<RenderedBlock>> = blocks
        .iter()
        .map(|b| {
            render::render_blocks(
                std::slice::from_ref(b),
                width,
                highlighter,
                theme,
                mermaid_mode,
            )
        })
        .collect();
    let flat_rendered = flatten_groups(&rendered_groups);
    let mut mermaid_cache: HashMap<usize, MermaidCacheEntry> = HashMap::new();

    // Populate initial mermaid cache
    for (i, (block, group)) in blocks.iter().zip(rendered_groups.iter()).enumerate() {
        if let crate::parser::Block::MermaidBlock { .. } = block {
            for rb in group {
                if matches!(rb, RenderedBlock::Diagram { .. }) {
                    mermaid_cache.insert(i, rb.clone());
                }
            }
        }
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;
    let detection = crate::graphics::detect(graphics, crate::graphics::OutputTarget::Pager);
    crate::pager::clear_after_probe(&mut stdout, &detection)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let term_size = terminal.size()?;
    let content_height = term_size.height.saturating_sub(1);
    let mut pager = PagerState::new(
        flat_rendered,
        content_height,
        term_size.width,
        max_content_width,
        theme,
    )
    .with_mermaid_graphics(mermaid_graphics)
    .with_graphics(detection.caps, base_dir(path));

    // Start file watcher
    let (_watcher, rx) = start_watcher(path)?;
    let filename = path.file_name().unwrap_or_default().to_str().unwrap_or("?");
    let mut status = StatusState::new(filename);

    // Event loop state
    let mut pending_change = false;
    let mut last_event_time: Option<Instant> = None;
    let mut last_image_check = Instant::now();
    let mut needs_redraw = true;

    let mut event_loop = || -> Result<()> {
        loop {
            // 1. Show images that finished decoding; send their data first
            if pager.poll_images() {
                needs_redraw = true;
            }
            crate::pager::write_graphics(terminal.backend_mut(), &pager.take_graphics_output())?;

            // 2. Draw if needed
            if needs_redraw {
                terminal.draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(0), Constraint::Length(1)])
                        .split(f.area());

                    pager.draw_content(f, chunks[0]);

                    let bottom_line = if let Some(search_line) = pager.search_bar_line() {
                        search_line
                    } else {
                        status.render()
                    };
                    f.render_widget(Paragraph::new(bottom_line), chunks[1]);
                })?;
                needs_redraw = false;
            }

            // 3. Poll for keyboard/mouse events (50ms timeout)
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => match pager.handle_key_event(key) {
                        KeyAction::Quit => break,
                        KeyAction::Redraw => needs_redraw = true,
                        KeyAction::Nothing => {}
                    },
                    Event::Mouse(mouse) if pager.handle_mouse_event(mouse) => {
                        needs_redraw = true;
                    }
                    Event::Resize(w, h) => {
                        if let Some(cell_px) = crate::pager::current_cell_px() {
                            pager.set_cell_px(cell_px);
                        }
                        let new_content_height = h.saturating_sub(1);
                        pager.resize(w, new_content_height);
                        needs_redraw = true;
                    }
                    _ => {}
                }
            }

            // 4. Drain file change events
            while rx.try_recv().is_ok() {
                last_event_time = Some(Instant::now());
                pending_change = true;
            }

            // 5. Debounce check
            if pending_change
                && let Some(t) = last_event_time
                && t.elapsed() >= Duration::from_millis(100)
            {
                pending_change = false;
                last_event_time = None;

                if let Some(content) = read_file_with_retry(path) {
                    let new_hash = content_hash(&content);
                    if new_hash != last_hash {
                        let new_blocks = crate::parser::parse_markdown(&content);
                        let new_groups = diff_and_render(
                            &blocks,
                            &rendered_groups,
                            &new_blocks,
                            width,
                            highlighter,
                            theme,
                            mermaid_mode,
                            &mut mermaid_cache,
                        );
                        let flat = flatten_groups(&new_groups);

                        if new_blocks.len() != blocks.len() {
                            pager.expanded.clear();
                            pager.toggled_diagrams.clear();
                            mermaid_cache.clear();
                        }

                        pager.replace_content(flat);

                        blocks = new_blocks;
                        rendered_groups = new_groups;
                        last_hash = new_hash;

                        status.set_message("updated");
                        needs_redraw = true;
                    }
                } else {
                    status.set_message("file unreadable");
                    needs_redraw = true;
                }
            }

            // 6. Clear expired status messages
            if status.tick() {
                needs_redraw = true;
            }

            // 7. Only the markdown file is watched; check image files for
            // changes now and then (one stat each).
            if last_image_check.elapsed() >= IMAGE_CHECK_INTERVAL {
                last_image_check = Instant::now();
                pager.refresh_images();
            }
        }
        Ok(())
    };
    let result = event_loop();
    // Free transmitted images before leaving the alternate screen.
    let _ = crate::pager::write_graphics(terminal.backend_mut(), &pager.delete_images());
    result
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

        assert!(cache.contains_key(&0));
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
        cache.insert(
            0, // position-based key
            crate::render::RenderedBlock::Diagram {
                lines: vec![crate::render::StyledLine {
                    spans: vec![crate::render::StyledSpan::plain("cached line")],
                }],
                node_count: 2,
                edge_count: 1,
                kind: crate::render::DiagramKind::Flowchart,
                image: None,
            },
        );

        // Render block with content that fails to parse but has cache entry
        let blocks = vec![Block::MermaidBlock {
            content: "INVALID MERMAID CONTENT".into(),
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
