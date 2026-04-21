use std::collections::HashSet;
use std::io::stdout;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    style::{Color as RColor, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::render::{Color, RenderedBlock, StyledLine, StyledSpan};

/// Ensures terminal cleanup runs on all exit paths (error propagation, panic, normal return).
/// Without this, an I/O error from `terminal.draw()` or `event::read()` would skip cleanup
/// and leave the terminal in raw mode with the alternate screen still active.
pub(crate) struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            crossterm::cursor::Show,
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}

fn detect_opener() -> Option<&'static str> {
    if std::process::Command::new("open")
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some("open");
    }
    if std::process::Command::new("xdg-open")
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some("xdg-open");
    }
    None
}

// ─── Style conversion ──────────────────────────────────────────────────────

fn span_to_ratatui(span: &StyledSpan) -> Span<'static> {
    let mut style = Style::default();

    if let Some(ref color) = span.style.fg {
        style = style.fg(color_to_ratatui(color));
    }
    if span.style.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if span.style.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if span.style.dim {
        style = style.add_modifier(Modifier::DIM);
    }

    Span::styled(span.text.clone(), style)
}

fn color_to_ratatui(color: &Color) -> RColor {
    match color {
        Color::Red => RColor::Red,
        Color::Green => RColor::Green,
        Color::Yellow => RColor::Yellow,
        Color::Blue => RColor::Blue,
        Color::Magenta => RColor::Magenta,
        Color::Cyan => RColor::Cyan,
        Color::White => RColor::White,
        Color::BrightYellow => RColor::LightYellow,
        Color::BrightCyan => RColor::LightCyan,
        Color::BrightMagenta => RColor::LightMagenta,
        Color::DarkGray => RColor::DarkGray,
        Color::Rgb(r, g, b) => RColor::Rgb(*r, *g, *b),
    }
}

fn styled_line_to_ratatui(line: &StyledLine) -> Line<'static> {
    Line::from(line.spans.iter().map(span_to_ratatui).collect::<Vec<_>>())
}

// ─── FlatLine ──────────────────────────────────────────────────────────────

pub(crate) enum FlatLine {
    Styled(StyledLine),
    DiagramAscii(String),
    DiagramCollapsed {
        #[allow(dead_code)]
        block_index: usize,
        node_count: usize,
        edge_count: usize,
    },
    #[allow(dead_code)]
    ImagePlaceholder {
        alt: String,
        url: String,
        block_index: usize,
    },
}

// ─── Interactive block tracking ───────────────────────────────────────────

pub(crate) struct InteractiveEntry {
    pub(crate) block_index: usize,
    pub(crate) flat_line_index: usize,
    pub(crate) flat_line_end: usize, // exclusive
}

// ─── PagerState ────────────────────────────────────────────────────────────

pub(crate) struct PagerState {
    pub(crate) content: Vec<RenderedBlock>,
    pub(crate) flat_lines: Vec<FlatLine>,
    pub(crate) scroll: usize,
    pub(crate) h_scroll: usize,
    pub(crate) expanded: HashSet<usize>,
    pub(crate) active: Option<usize>,
    interactive_blocks: Vec<InteractiveEntry>,
    pub(crate) terminal_height: u16,
    pub(crate) terminal_width: u16,
    opener: Option<&'static str>,
    theme: &'static crate::theme::Theme,
}

impl PagerState {
    pub(crate) fn new(
        content: Vec<RenderedBlock>,
        terminal_height: u16,
        terminal_width: u16,
        theme: &'static crate::theme::Theme,
    ) -> Self {
        let mut state = PagerState {
            content,
            flat_lines: Vec::new(),
            scroll: 0,
            h_scroll: 0,
            expanded: HashSet::new(),
            active: None,
            interactive_blocks: Vec::new(),
            terminal_height,
            terminal_width,
            opener: detect_opener(),
            theme,
        };
        state.rebuild_flat_lines();
        state.update_active_from_viewport();
        state
    }

    pub(crate) fn rebuild_flat_lines(&mut self) {
        let prev_block_index = self.active.map(|i| self.interactive_blocks[i].block_index);

        self.flat_lines.clear();
        self.interactive_blocks.clear();
        let height_threshold = (self.terminal_height as usize) / 2;
        let width_limit = self.terminal_width as usize;

        for (block_index, block) in self.content.iter().enumerate() {
            match block {
                RenderedBlock::Lines(lines) => {
                    for line in lines {
                        self.flat_lines.push(FlatLine::Styled(line.clone()));
                    }
                }
                RenderedBlock::Diagram {
                    lines,
                    node_count,
                    edge_count,
                } => {
                    let is_tall = lines.len() > height_threshold;
                    let is_wide = lines.iter().any(|l| l.len() > width_limit);
                    let is_large = is_tall || is_wide;
                    if is_large && !self.expanded.contains(&block_index) {
                        let flat_line_index = self.flat_lines.len();
                        self.flat_lines.push(FlatLine::DiagramCollapsed {
                            block_index,
                            node_count: *node_count,
                            edge_count: *edge_count,
                        });
                        self.interactive_blocks.push(InteractiveEntry {
                            block_index,
                            flat_line_index,
                            flat_line_end: flat_line_index + 1,
                        });
                    } else if is_large {
                        let flat_line_index = self.flat_lines.len();
                        for line in lines {
                            self.flat_lines.push(FlatLine::DiagramAscii(line.clone()));
                        }
                        self.interactive_blocks.push(InteractiveEntry {
                            block_index,
                            flat_line_index,
                            flat_line_end: flat_line_index + lines.len(),
                        });
                    } else {
                        for line in lines {
                            self.flat_lines.push(FlatLine::DiagramAscii(line.clone()));
                        }
                    }
                }
                RenderedBlock::Image { alt, url } => {
                    let flat_line_index = self.flat_lines.len();
                    self.flat_lines.push(FlatLine::ImagePlaceholder {
                        alt: alt.clone(),
                        url: url.clone(),
                        block_index,
                    });
                    self.interactive_blocks.push(InteractiveEntry {
                        block_index,
                        flat_line_index,
                        flat_line_end: flat_line_index + 1,
                    });
                }
            }
        }

        // Preserve active selection across rebuilds by matching block_index
        self.active = prev_block_index.and_then(|bi| {
            self.interactive_blocks
                .iter()
                .position(|e| e.block_index == bi)
        });
    }

    pub(crate) fn max_scroll(&self) -> usize {
        let total = self.flat_lines.len();
        let height = self.terminal_height as usize;
        total.saturating_sub(height)
    }

    pub(crate) fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll > max {
            self.scroll = max;
        }
    }

    pub(crate) fn update_active_from_viewport(&mut self) {
        let start = self.scroll;
        let end = (self.scroll + self.terminal_height as usize).min(self.flat_lines.len());

        self.active = self
            .interactive_blocks
            .iter()
            .position(|entry| entry.flat_line_index >= start && entry.flat_line_index < end);
    }

    pub(crate) fn cycle_active(&mut self, forward: bool) {
        if self.interactive_blocks.is_empty() {
            return;
        }

        let len = self.interactive_blocks.len();
        self.active = Some(match self.active {
            None => 0,
            Some(i) if forward => (i + 1) % len,
            Some(i) => (i + len - 1) % len,
        });

        // Scroll to make active block visible
        if let Some(idx) = self.active {
            let flat_idx = self.interactive_blocks[idx].flat_line_index;
            let height = self.terminal_height as usize;
            if flat_idx < self.scroll {
                self.scroll = flat_idx;
            } else if flat_idx >= self.scroll + height {
                self.scroll = flat_idx.saturating_sub(height / 2);
            }
            self.clamp_scroll();
        }
    }

    pub(crate) fn activate_current(&mut self) {
        let block_index = match self.active {
            Some(idx) => self.interactive_blocks[idx].block_index,
            None => return,
        };

        match &self.content[block_index] {
            RenderedBlock::Diagram { .. } => {
                if self.expanded.contains(&block_index) {
                    self.expanded.remove(&block_index);
                } else {
                    self.expanded.insert(block_index);
                }
                self.rebuild_flat_lines();
                self.clamp_scroll();
            }
            RenderedBlock::Image { url, .. } => {
                let url = url.clone();
                if let Some(opener) = self.opener.as_ref() {
                    let _ = std::process::Command::new(opener)
                        .arg(&url)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            }
            _ => {}
        }
    }

    fn active_entry(&self) -> Option<&InteractiveEntry> {
        self.active.map(|idx| &self.interactive_blocks[idx])
    }

    fn is_active_indicator_line(&self, flat_line_index: usize) -> bool {
        self.active_entry()
            .is_some_and(|e| e.flat_line_index == flat_line_index)
    }

    fn is_in_active_block(&self, flat_line_index: usize) -> bool {
        self.active_entry().is_some_and(|e| {
            flat_line_index >= e.flat_line_index && flat_line_index < e.flat_line_end
        })
    }

    pub(crate) fn flat_line_to_ratatui(
        &self,
        flat: &FlatLine,
        flat_line_index: usize,
    ) -> Line<'static> {
        let collapsed_color = color_to_ratatui(&self.theme.diagram_collapsed);

        match flat {
            FlatLine::Styled(line) => styled_line_to_ratatui(line),
            FlatLine::DiagramAscii(text) => {
                if self.is_in_active_block(flat_line_index) {
                    Line::from(vec![
                        Span::styled("▎", Style::default().fg(collapsed_color)),
                        Span::raw(text.clone()),
                    ])
                } else {
                    Line::raw(text.clone())
                }
            }
            FlatLine::DiagramCollapsed {
                node_count,
                edge_count,
                ..
            } => {
                if self.is_active_indicator_line(flat_line_index) {
                    let text = format!(
                        "▸ [Flowchart: {} nodes, {} edges — Enter to expand]",
                        node_count, edge_count
                    );
                    Line::from(Span::styled(text, Style::default().fg(collapsed_color)))
                } else {
                    let text = format!(
                        "  [Flowchart: {} nodes, {} edges — Enter to expand]",
                        node_count, edge_count
                    );
                    Line::from(Span::styled(
                        text,
                        Style::default()
                            .fg(collapsed_color)
                            .add_modifier(Modifier::DIM),
                    ))
                }
            }
            FlatLine::ImagePlaceholder { alt, .. } => {
                let (prefix, modifier) = if self.is_active_indicator_line(flat_line_index) {
                    ("▸", Modifier::empty())
                } else {
                    (" ", Modifier::DIM)
                };
                let text = if alt.is_empty() {
                    format!("{} [Image — Enter to open]", prefix)
                } else {
                    format!("{} [Image: {} — Enter to open]", prefix, alt)
                };
                Line::from(Span::styled(
                    text,
                    Style::default().fg(collapsed_color).add_modifier(modifier),
                ))
            }
        }
    }

    pub(crate) fn draw_content(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let height = area.height as usize;
        let lines: Vec<Line> = self
            .flat_lines
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(height)
            .map(|(idx, fl)| self.flat_line_to_ratatui(fl, idx))
            .collect();
        let paragraph = Paragraph::new(lines).scroll((0, self.h_scroll as u16));
        f.render_widget(paragraph, area);
    }
}

// ─── Public entry point ────────────────────────────────────────────────────

pub fn run_pager(content: Vec<RenderedBlock>, theme: &'static crate::theme::Theme) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let mut state = PagerState::new(content, size.height, size.width, theme);

    loop {
        terminal.draw(|f| {
            state.draw_content(f, f.area());
        })?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let page = (state.terminal_height as usize).saturating_sub(1).max(1);
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max = state.max_scroll();
                        if state.scroll < max {
                            state.scroll += 1;
                            state.update_active_from_viewport();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.scroll = state.scroll.saturating_sub(1);
                        state.update_active_from_viewport();
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        let max = state.max_scroll();
                        state.scroll = (state.scroll + page).min(max);
                        state.update_active_from_viewport();
                    }
                    KeyCode::PageUp => {
                        state.scroll = state.scroll.saturating_sub(page);
                        state.update_active_from_viewport();
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        state.scroll = 0;
                        state.update_active_from_viewport();
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        state.scroll = state.max_scroll();
                        state.update_active_from_viewport();
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        state.h_scroll = state.h_scroll.saturating_add(4);
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        state.h_scroll = state.h_scroll.saturating_sub(4);
                    }
                    KeyCode::Tab => {
                        state.cycle_active(true);
                    }
                    KeyCode::BackTab => {
                        state.cycle_active(false);
                    }
                    KeyCode::Enter => {
                        state.activate_current();
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => {
                    let max = state.max_scroll();
                    state.scroll = (state.scroll + 3).min(max);
                    state.update_active_from_viewport();
                }
                MouseEventKind::ScrollUp => {
                    state.scroll = state.scroll.saturating_sub(3);
                    state.update_active_from_viewport();
                }
                _ => {}
            },
            Event::Resize(w, h) => {
                state.terminal_height = h;
                state.terminal_width = w;
                state.rebuild_flat_lines();
                state.clamp_scroll();
                state.update_active_from_viewport();
            }
            _ => {}
        }
    }

    Ok(())
}
