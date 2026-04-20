use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::stdout;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind},
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

use crate::pager::{PagerState, TerminalGuard};
use crate::render::{self, MermaidMode, RenderedBlock};

pub fn content_hash(content: &str) -> u64 {
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

fn read_file_with_retry(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Some(content),
        Err(_) => {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(path).ok()
        }
    }
}

struct MermaidCacheEntry {
    lines: Vec<String>,
    node_count: usize,
    edge_count: usize,
}

#[allow(clippy::too_many_arguments)]
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
        if let Block::MermaidBlock { .. } = new_block {
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
                            i,
                            MermaidCacheEntry {
                                lines: lines.clone(),
                                node_count: *node_count,
                                edge_count: *edge_count,
                            },
                        );
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
                    format!("│ {} ", msg),
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

pub fn run_watch(
    path: &Path,
    width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &'static crate::theme::Theme,
    mermaid_mode: MermaidMode,
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
                if let RenderedBlock::Diagram {
                    lines,
                    node_count,
                    edge_count,
                } = rb
                {
                    mermaid_cache.insert(
                        i,
                        MermaidCacheEntry {
                            lines: lines.clone(),
                            node_count: *node_count,
                            edge_count: *edge_count,
                        },
                    );
                }
            }
        }
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let term_height = terminal.size()?.height;
    let content_height = term_height.saturating_sub(1);
    let mut pager = PagerState::new(flat_rendered, content_height, theme);

    // Start file watcher
    let (_watcher, rx) = start_watcher(path)?;
    let filename = path.file_name().unwrap_or_default().to_str().unwrap_or("?");
    let mut status = StatusState::new(filename);

    // Event loop state
    let mut pending_change = false;
    let mut last_event_time: Option<Instant> = None;
    let mut needs_redraw = true;

    loop {
        // 1. Draw if needed
        if needs_redraw {
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(f.area());

                pager.draw_content(f, chunks[0]);

                let status_widget = Paragraph::new(status.render());
                f.render_widget(status_widget, chunks[1]);
            })?;
            needs_redraw = false;
        }

        // 2. Poll for keyboard/mouse events (50ms timeout)
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let page = (pager.terminal_height as usize).saturating_sub(1).max(1);
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Down | KeyCode::Char('j') => {
                            let max = pager.max_scroll();
                            if pager.scroll < max {
                                pager.scroll += 1;
                                needs_redraw = true;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') if pager.scroll > 0 => {
                            pager.scroll = pager.scroll.saturating_sub(1);
                            needs_redraw = true;
                        }
                        KeyCode::PageDown | KeyCode::Char(' ') => {
                            let max = pager.max_scroll();
                            let new_scroll = (pager.scroll + page).min(max);
                            if new_scroll != pager.scroll {
                                pager.scroll = new_scroll;
                                needs_redraw = true;
                            }
                        }
                        KeyCode::PageUp => {
                            let new_scroll = pager.scroll.saturating_sub(page);
                            if new_scroll != pager.scroll {
                                pager.scroll = new_scroll;
                                needs_redraw = true;
                            }
                        }
                        KeyCode::Home | KeyCode::Char('g') if pager.scroll != 0 => {
                            pager.scroll = 0;
                            needs_redraw = true;
                        }
                        KeyCode::End | KeyCode::Char('G') => {
                            let max = pager.max_scroll();
                            if pager.scroll != max {
                                pager.scroll = max;
                                needs_redraw = true;
                            }
                        }
                        KeyCode::Tab => {
                            pager.toggle_diagram_at_scroll();
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        let max = pager.max_scroll();
                        pager.scroll = (pager.scroll + 3).min(max);
                        needs_redraw = true;
                    }
                    MouseEventKind::ScrollUp => {
                        pager.scroll = pager.scroll.saturating_sub(3);
                        needs_redraw = true;
                    }
                    _ => {}
                },
                Event::Resize(_, h) => {
                    let new_content_height = h.saturating_sub(1);
                    pager.terminal_height = new_content_height;
                    pager.rebuild_flat_lines();
                    pager.clamp_scroll();
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        // 3. Drain file change events
        while rx.try_recv().is_ok() {
            last_event_time = Some(Instant::now());
            pending_change = true;
        }

        // 4. Debounce check
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
                        mermaid_cache.clear();
                    }

                    pager.content = flat;
                    pager.rebuild_flat_lines();
                    pager.clamp_scroll();

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

        // 5. Clear expired status messages
        if status.tick() {
            needs_redraw = true;
        }
    }

    Ok(())
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
            MermaidCacheEntry {
                lines: vec!["cached line".to_string()],
                node_count: 2,
                edge_count: 1,
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
