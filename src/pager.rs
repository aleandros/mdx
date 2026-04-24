use std::collections::HashSet;
use std::io::stdout;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
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

/// Tab width used when expanding `\t` in spans before handing them to ratatui.
/// Ratatui measures a tab as 1 cell, but real terminals advance to the next
/// tab stop — the width mismatch causes stale cells to persist across frames
/// (visible as ghosted characters on the right edge when scrolling).
const TAB_WIDTH: usize = 4;

fn expand_tabs(text: &str) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    let spaces: String = " ".repeat(TAB_WIDTH);
    text.replace('\t', &spaces)
}

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

    Span::styled(expand_tabs(&span.text), style)
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

// ─── Search ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(crate) enum SearchDirection {
    Forward,
    Backward,
}

pub(crate) enum KeyAction {
    Quit,
    Redraw,
    Nothing,
}

// ─── FlatLine ──────────────────────────────────────────────────────────────

pub(crate) enum FlatLine {
    Styled(StyledLine),
    DiagramAscii(StyledLine),
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
    // Search state
    search_mode: Option<SearchDirection>,
    search_input: String,
    search_query: String,
    search_matches: Vec<usize>,
    search_current: Option<usize>,
    search_direction: SearchDirection,
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
            search_mode: None,
            search_input: String::new(),
            search_query: String::new(),
            search_matches: Vec::new(),
            search_current: None,
            search_direction: SearchDirection::Forward,
        };
        state.rebuild_flat_lines();
        state.update_active_from_viewport();
        state
    }

    pub(crate) fn rebuild_flat_lines(&mut self) {
        let prev_block_index = self.active.map(|i| self.interactive_blocks[i].block_index);

        self.flat_lines.clear();
        self.interactive_blocks.clear();
        let height_threshold = self.terminal_height as usize;
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
                    // Collapse only when genuinely unmanageable: taller than
                    // the full terminal height, or more than twice as wide.
                    let is_tall = lines.len() > height_threshold;
                    let is_wide = lines.iter().any(|l| {
                        l.spans.iter().map(|s| s.text.len()).sum::<usize>() > width_limit * 2
                    });
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

        if !self.search_query.is_empty() {
            self.recompute_matches();
        }
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
            FlatLine::DiagramAscii(styled_line) => {
                if self.is_in_active_block(flat_line_index) {
                    let mut spans = vec![Span::styled("▎", Style::default().fg(collapsed_color))];
                    spans.extend(styled_line.spans.iter().map(span_to_ratatui));
                    Line::from(spans)
                } else {
                    styled_line_to_ratatui(styled_line)
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
            .map(|(idx, fl)| {
                let mut line = self.flat_line_to_ratatui(fl, idx);
                if self.is_current_search_match(idx) {
                    for span in &mut line.spans {
                        span.style = span.style.bg(RColor::DarkGray);
                    }
                }
                line
            })
            .collect();
        let paragraph = Paragraph::new(lines).scroll((0, self.h_scroll as u16));
        f.render_widget(paragraph, area);
    }
    // ─── Search ────────────────────────────────────────────────────────────

    fn flat_line_text(flat: &FlatLine) -> String {
        match flat {
            FlatLine::Styled(line) | FlatLine::DiagramAscii(line) => {
                line.spans.iter().map(|s| s.text.as_str()).collect()
            }
            FlatLine::DiagramCollapsed {
                node_count,
                edge_count,
                ..
            } => {
                format!("Flowchart: {} nodes, {} edges", node_count, edge_count)
            }
            FlatLine::ImagePlaceholder { alt, .. } => alt.clone(),
        }
    }

    fn recompute_matches(&mut self) {
        self.search_matches.clear();
        self.search_current = None;
        let query = self.search_query.to_lowercase();
        for (i, fl) in self.flat_lines.iter().enumerate() {
            if Self::flat_line_text(fl).to_lowercase().contains(&query) {
                self.search_matches.push(i);
            }
        }
        if !self.search_matches.is_empty() {
            let pos = self.search_matches.iter().position(|&m| m >= self.scroll);
            self.search_current = Some(pos.unwrap_or(0));
        }
    }

    fn enter_search(&mut self, direction: SearchDirection) {
        self.search_mode = Some(direction);
        self.search_direction = direction;
        self.search_input.clear();
    }

    fn cancel_search(&mut self) {
        self.search_mode = None;
        self.search_input.clear();
    }

    fn confirm_search(&mut self) {
        self.search_mode = None;
        let query = std::mem::take(&mut self.search_input);
        if query.is_empty() {
            self.search_query.clear();
            self.search_matches.clear();
            self.search_current = None;
            return;
        }
        self.search_query = query;
        self.recompute_matches();
        if !self.search_matches.is_empty() {
            match self.search_direction {
                SearchDirection::Forward => {
                    let pos = self.search_matches.iter().position(|&m| m >= self.scroll);
                    self.search_current = Some(pos.unwrap_or(0));
                }
                SearchDirection::Backward => {
                    let viewport_end = self.scroll + self.terminal_height as usize;
                    let pos = self.search_matches.iter().rposition(|&m| m < viewport_end);
                    self.search_current = Some(pos.unwrap_or(self.search_matches.len() - 1));
                }
            }
            self.scroll_to_current_match();
        }
    }

    fn scroll_to_current_match(&mut self) {
        if let Some(idx) = self.search_current {
            let line = self.search_matches[idx];
            let height = self.terminal_height as usize;
            if line < self.scroll || line >= self.scroll + height {
                self.scroll = line.saturating_sub(height / 3);
                self.clamp_scroll();
            }
            self.update_active_from_viewport();
        }
    }

    fn next_match(&mut self) -> bool {
        if self.search_matches.is_empty() {
            return false;
        }
        let len = self.search_matches.len();
        self.search_current = Some(match self.search_current {
            Some(idx) => (idx + 1) % len,
            None => 0,
        });
        self.scroll_to_current_match();
        true
    }

    fn prev_match(&mut self) -> bool {
        if self.search_matches.is_empty() {
            return false;
        }
        let len = self.search_matches.len();
        self.search_current = Some(match self.search_current {
            Some(idx) => (idx + len - 1) % len,
            None => len - 1,
        });
        self.scroll_to_current_match();
        true
    }

    fn is_current_search_match(&self, flat_line_index: usize) -> bool {
        self.search_current
            .is_some_and(|c| self.search_matches[c] == flat_line_index)
    }

    pub(crate) fn search_bar_line(&self) -> Option<Line<'static>> {
        if let Some(dir) = &self.search_mode {
            let prefix = match dir {
                SearchDirection::Forward => "/",
                SearchDirection::Backward => "?",
            };
            Some(Line::from(vec![
                Span::styled(
                    format!("{}{}", prefix, self.search_input),
                    Style::default().fg(RColor::White),
                ),
                Span::styled("█", Style::default().fg(RColor::DarkGray)),
            ]))
        } else if !self.search_query.is_empty() && !self.search_matches.is_empty() {
            let prefix = match self.search_direction {
                SearchDirection::Forward => "/",
                SearchDirection::Backward => "?",
            };
            let current = self.search_current.map(|c| c + 1).unwrap_or(0);
            let total = self.search_matches.len();
            Some(Line::from(Span::styled(
                format!("{}{} [{}/{}]", prefix, self.search_query, current, total),
                Style::default().fg(RColor::DarkGray),
            )))
        } else if !self.search_query.is_empty() {
            Some(Line::from(Span::styled(
                format!("Pattern not found: {}", self.search_query),
                Style::default().fg(RColor::Red),
            )))
        } else {
            None
        }
    }

    // ─── Key handling ─────────────────────────────────────────────────────

    fn handle_search_input(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                self.cancel_search();
                KeyAction::Redraw
            }
            KeyCode::Enter => {
                self.confirm_search();
                KeyAction::Redraw
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                KeyAction::Redraw
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
                KeyAction::Redraw
            }
            _ => KeyAction::Nothing,
        }
    }

    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> KeyAction {
        if key.kind != KeyEventKind::Press {
            return KeyAction::Nothing;
        }

        if self.search_mode.is_some() {
            return self.handle_search_input(key);
        }

        let page = (self.terminal_height as usize).saturating_sub(1).max(1);
        let half_page = (page / 2).max(1);

        // Check Ctrl combinations first
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('d') => {
                    let max = self.max_scroll();
                    let new = (self.scroll + half_page).min(max);
                    if new != self.scroll {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    } else {
                        KeyAction::Nothing
                    }
                }
                KeyCode::Char('u') => {
                    let new = self.scroll.saturating_sub(half_page);
                    if new != self.scroll {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    } else {
                        KeyAction::Nothing
                    }
                }
                KeyCode::Char('f') => {
                    let max = self.max_scroll();
                    let new = (self.scroll + page).min(max);
                    if new != self.scroll {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    } else {
                        KeyAction::Nothing
                    }
                }
                KeyCode::Char('b') => {
                    let new = self.scroll.saturating_sub(page);
                    if new != self.scroll {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    } else {
                        KeyAction::Nothing
                    }
                }
                _ => KeyAction::Nothing,
            };
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => KeyAction::Quit,
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.max_scroll();
                if self.scroll < max {
                    self.scroll += 1;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.scroll > 0 {
                    self.scroll = self.scroll.saturating_sub(1);
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                let max = self.max_scroll();
                let new = (self.scroll + page).min(max);
                if new != self.scroll {
                    self.scroll = new;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::PageUp => {
                let new = self.scroll.saturating_sub(page);
                if new != self.scroll {
                    self.scroll = new;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if self.scroll != 0 {
                    self.scroll = 0;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                let max = self.max_scroll();
                if self.scroll != max {
                    self.scroll = max;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.h_scroll = self.h_scroll.saturating_add(4);
                KeyAction::Redraw
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.h_scroll = self.h_scroll.saturating_sub(4);
                KeyAction::Redraw
            }
            KeyCode::Tab => {
                self.cycle_active(true);
                KeyAction::Redraw
            }
            KeyCode::BackTab => {
                self.cycle_active(false);
                KeyAction::Redraw
            }
            KeyCode::Enter => {
                self.activate_current();
                KeyAction::Redraw
            }
            KeyCode::Char('/') => {
                self.enter_search(SearchDirection::Forward);
                KeyAction::Redraw
            }
            KeyCode::Char('?') => {
                self.enter_search(SearchDirection::Backward);
                KeyAction::Redraw
            }
            KeyCode::Char('n') => {
                if self.next_match() {
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            KeyCode::Char('N') => {
                if self.prev_match() {
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
            }
            _ => KeyAction::Nothing,
        }
    }

    pub(crate) fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                let max = self.max_scroll();
                self.scroll = (self.scroll + 3).min(max);
                self.update_active_from_viewport();
                true
            }
            MouseEventKind::ScrollUp => {
                self.scroll = self.scroll.saturating_sub(3);
                self.update_active_from_viewport();
                true
            }
            _ => false,
        }
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
            let area = f.area();
            if let Some(search_line) = state.search_bar_line() {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(1)])
                    .split(area);
                state.draw_content(f, chunks[0]);
                f.render_widget(Paragraph::new(search_line), chunks[1]);
            } else {
                state.draw_content(f, area);
            }
        })?;

        match event::read()? {
            Event::Key(key) => match state.handle_key_event(key) {
                KeyAction::Quit => break,
                KeyAction::Redraw | KeyAction::Nothing => {}
            },
            Event::Mouse(mouse) => {
                state.handle_mouse_event(mouse);
            }
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
