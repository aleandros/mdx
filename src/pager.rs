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

use crate::render::{Color, DiagramKind, RenderedBlock, SpanStyle, StyledLine, StyledSpan};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

// ─── Layout constants ──────────────────────────────────────────────────────

/// Default cap on the pager's content column, in columns. On terminals wider
/// than this the content is centered between equal left/right gutters so
/// prose doesn't stretch across the whole screen. `0` disables the cap.
pub(crate) const DEFAULT_MAX_CONTENT_WIDTH: u16 = 100;

/// Effective content column width for a terminal `terminal_width` columns wide
/// under a `max_content_width` cap (`0` = uncapped).
pub(crate) fn content_width(terminal_width: u16, max_content_width: u16) -> u16 {
    if max_content_width == 0 {
        terminal_width
    } else {
        terminal_width.min(max_content_width)
    }
}

/// Display width of a styled line in terminal columns (not bytes — box-drawing
/// characters are 3 bytes each in UTF-8 but occupy a single cell).
fn line_width(line: &StyledLine) -> usize {
    line.spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.text.as_str()))
        .sum()
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
    DiagramAscii {
        line: StyledLine,
        /// Left indent for this diagram's rows. Equals the text gutter when
        /// the diagram fits the content column; shrinks (down to 0) for wider
        /// diagrams so they use the full terminal width before h-scroll is needed.
        indent: usize,
    },
    DiagramCollapsed {
        #[allow(dead_code)]
        block_index: usize,
        node_count: usize,
        edge_count: usize,
        kind: DiagramKind,
    },
    #[allow(dead_code)]
    ImagePlaceholder {
        alt: String,
        url: String,
        block_index: usize,
    },
}

/// Word-wrap a styled line at `width` columns, preserving span styles.
/// Continuation lines are prefixed with the original leading-space indent
/// (capped) so wrapped paragraphs and list items keep their visual rhythm.
fn wrap_styled_line(line: &StyledLine, width: usize) -> Vec<StyledLine> {
    if width < 4 {
        return vec![line.clone()];
    }
    let total: usize = line
        .spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.text.as_str()))
        .sum();
    if total <= width {
        return vec![line.clone()];
    }

    // Detect leading indent (count of leading space chars across spans).
    let mut indent_chars: usize = 0;
    'outer: for span in &line.spans {
        for c in span.text.chars() {
            if c == ' ' {
                indent_chars += 1;
            } else {
                break 'outer;
            }
        }
    }
    let indent = indent_chars.min(width.saturating_sub(4));

    // Flatten line into char/style pairs for easier walking.
    let mut chars: Vec<(char, SpanStyle)> = Vec::with_capacity(total);
    for span in &line.spans {
        for ch in span.text.chars() {
            chars.push((ch, span.style.clone()));
        }
    }

    let mut lines: Vec<Vec<(char, SpanStyle)>> = vec![Vec::new()];
    let mut cur_w: usize = 0;
    let mut last_break: Option<usize> = None; // index in current line at last whitespace
    let mut is_first_line = true;
    // Whether the current line holds any non-space content yet. Spaces before
    // that point (the leading indent) are not break candidates: breaking there
    // would leave an empty first line and carry a tail longer than the column.
    let mut seen_content = false;

    let push_continuation = |lines: &mut Vec<Vec<(char, SpanStyle)>>, cur_w: &mut usize| {
        let mut new_line: Vec<(char, SpanStyle)> = Vec::new();
        for _ in 0..indent {
            new_line.push((' ', SpanStyle::default()));
        }
        lines.push(new_line);
        *cur_w = indent;
    };

    let mut i = 0;
    while i < chars.len() {
        let (ch, style) = chars[i].clone();
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);

        // Skip leading whitespace on continuation lines (we already laid an indent).
        if !is_first_line && cur_w == indent && ch == ' ' {
            i += 1;
            continue;
        }

        if cur_w + cw > width && cur_w > 0 {
            let cur = lines.last_mut().unwrap();
            if let Some(brk) = last_break {
                let tail: Vec<(char, SpanStyle)> = cur.drain(brk..).collect();
                // Drop the leading whitespace from tail
                let tail: Vec<(char, SpanStyle)> =
                    tail.into_iter().skip_while(|(c, _)| *c == ' ').collect();
                push_continuation(&mut lines, &mut cur_w);
                is_first_line = false;
                seen_content = !tail.is_empty();
                let cur = lines.last_mut().unwrap();
                for entry in tail {
                    cur.push(entry);
                }
                cur_w = lines
                    .last()
                    .unwrap()
                    .iter()
                    .map(|(c, _)| UnicodeWidthChar::width(*c).unwrap_or(0))
                    .sum::<usize>();
                last_break = None;
            } else {
                // Hard break (no whitespace seen on this line).
                push_continuation(&mut lines, &mut cur_w);
                is_first_line = false;
                seen_content = false;
                last_break = None;
            }
        }

        let cur = lines.last_mut().unwrap();
        cur.push((ch, style));
        cur_w += cw;
        if ch == ' ' {
            if seen_content {
                last_break = Some(cur.len() - 1);
            }
        } else {
            seen_content = true;
        }
        i += 1;
    }

    // Recombine each line's chars into spans, merging consecutive same-style runs.
    let mut out: Vec<StyledLine> = Vec::with_capacity(lines.len());
    for line_chars in lines {
        let mut spans: Vec<StyledSpan> = Vec::new();
        for (ch, style) in line_chars {
            if let Some(last) = spans.last_mut()
                && last.style == style
            {
                last.text.push(ch);
                continue;
            }
            spans.push(StyledSpan {
                text: ch.to_string(),
                style,
            });
        }
        out.push(StyledLine { spans });
    }
    if out.is_empty() {
        out.push(line.clone());
    }
    out
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
    /// Cap on the content column width (0 = no cap). See [`DEFAULT_MAX_CONTENT_WIDTH`].
    pub(crate) max_content_width: u16,
    /// Effective content column width: `min(terminal_width, max_content_width)`.
    content_width: usize,
    /// Columns of left margin before the content column (centered).
    gutter: usize,
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
        max_content_width: u16,
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
            max_content_width,
            content_width: 0,
            gutter: 0,
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
        let terminal_width = self.terminal_width as usize;

        // Content column: capped at max_content_width and centered. Text wraps
        // at the column width; the gutter is applied at draw time.
        self.content_width = content_width(self.terminal_width, self.max_content_width) as usize;
        self.gutter = (terminal_width - self.content_width) / 2;
        let wrap_width = self.content_width;

        for (block_index, block) in self.content.iter().enumerate() {
            match block {
                RenderedBlock::Lines(lines) => {
                    for line in lines {
                        for wrapped in wrap_styled_line(line, wrap_width) {
                            self.flat_lines.push(FlatLine::Styled(wrapped));
                        }
                    }
                }
                RenderedBlock::Diagram {
                    lines,
                    node_count,
                    edge_count,
                    kind,
                } => {
                    // Collapse only when genuinely unmanageable: taller than
                    // the full terminal height, or more than twice as wide as
                    // the terminal (measured in columns, not bytes).
                    let diagram_width = lines.iter().map(line_width).max().unwrap_or(0);
                    let is_tall = lines.len() > height_threshold;
                    let is_wide = diagram_width > terminal_width * 2;
                    let is_large = is_tall || is_wide;

                    // Diagrams that fit the content column share the text
                    // gutter. Wider ones are centered on the terminal if they
                    // fit it, and flush-left otherwise (h-scroll takes over).
                    let indent = if diagram_width <= self.content_width {
                        self.gutter
                    } else {
                        terminal_width.saturating_sub(diagram_width) / 2
                    };

                    // Pad with a blank line above so collapsed indicators (and
                    // expanded diagrams) don't run flush against neighboring text.
                    self.flat_lines.push(FlatLine::Styled(StyledLine::empty()));

                    if is_large && !self.expanded.contains(&block_index) {
                        let flat_line_index = self.flat_lines.len();
                        self.flat_lines.push(FlatLine::DiagramCollapsed {
                            block_index,
                            node_count: *node_count,
                            edge_count: *edge_count,
                            kind: *kind,
                        });
                        self.interactive_blocks.push(InteractiveEntry {
                            block_index,
                            flat_line_index,
                            flat_line_end: flat_line_index + 1,
                        });
                    } else if is_large {
                        let flat_line_index = self.flat_lines.len();
                        for line in lines {
                            self.flat_lines.push(FlatLine::DiagramAscii {
                                line: line.clone(),
                                indent,
                            });
                        }
                        self.interactive_blocks.push(InteractiveEntry {
                            block_index,
                            flat_line_index,
                            flat_line_end: flat_line_index + lines.len(),
                        });
                    } else {
                        for line in lines {
                            self.flat_lines.push(FlatLine::DiagramAscii {
                                line: line.clone(),
                                indent,
                            });
                        }
                    }

                    // Trailing blank for breathing room below the diagram.
                    self.flat_lines.push(FlatLine::Styled(StyledLine::empty()));
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

    /// Convert a flat line to its ratatui form, returning `(indent, line)`.
    /// The indent is the number of blank columns the caller must emit before
    /// the line (the content gutter, or a per-diagram indent). Keeping it
    /// separate lets `draw_content` highlight search matches on the content
    /// only, not on the margin.
    pub(crate) fn flat_line_to_ratatui(
        &self,
        flat: &FlatLine,
        flat_line_index: usize,
    ) -> (usize, Line<'static>) {
        let collapsed_color = color_to_ratatui(&self.theme.diagram_collapsed);

        match flat {
            FlatLine::Styled(line) => (self.gutter, styled_line_to_ratatui(line)),
            FlatLine::DiagramAscii {
                line: styled_line,
                indent,
            } => {
                if self.is_in_active_block(flat_line_index) {
                    // Put the active marker in the last gutter column so the
                    // diagram itself doesn't shift when it becomes active.
                    let mut spans = vec![Span::styled("▎", Style::default().fg(collapsed_color))];
                    spans.extend(styled_line.spans.iter().map(span_to_ratatui));
                    (indent.saturating_sub(1), Line::from(spans))
                } else {
                    (*indent, styled_line_to_ratatui(styled_line))
                }
            }
            FlatLine::DiagramCollapsed {
                node_count,
                edge_count,
                kind,
                ..
            } => {
                let (n_noun, e_noun) = kind.count_nouns();
                let body = format!(
                    "[{}: {} {}, {} {} — Enter to expand]",
                    kind.label(),
                    node_count,
                    n_noun,
                    edge_count,
                    e_noun,
                );
                let line = if self.is_active_indicator_line(flat_line_index) {
                    let text = format!("▸ {}", body);
                    Line::from(Span::styled(text, Style::default().fg(collapsed_color)))
                } else {
                    let text = format!("  {}", body);
                    Line::from(Span::styled(
                        text,
                        Style::default()
                            .fg(collapsed_color)
                            .add_modifier(Modifier::DIM),
                    ))
                };
                (self.gutter, line)
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
                let line = Line::from(Span::styled(
                    text,
                    Style::default().fg(collapsed_color).add_modifier(modifier),
                ));
                (self.gutter, line)
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
                let (indent, mut line) = self.flat_line_to_ratatui(fl, idx);
                if self.is_current_search_match(idx) {
                    for span in &mut line.spans {
                        span.style = span.style.bg(RColor::DarkGray);
                    }
                }
                if indent > 0 {
                    let mut spans = vec![Span::raw(" ".repeat(indent))];
                    spans.extend(line.spans);
                    line = Line::from(spans);
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
            FlatLine::Styled(line) | FlatLine::DiagramAscii { line, .. } => {
                line.spans.iter().map(|s| s.text.as_str()).collect()
            }
            FlatLine::DiagramCollapsed {
                node_count,
                edge_count,
                kind,
                ..
            } => {
                let (n_noun, e_noun) = kind.count_nouns();
                format!(
                    "{}: {} {}, {} {}",
                    kind.label(),
                    node_count,
                    n_noun,
                    edge_count,
                    e_noun
                )
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

pub fn run_pager(
    content: Vec<RenderedBlock>,
    max_content_width: u16,
    theme: &'static crate::theme::Theme,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let mut state = PagerState::new(content, size.height, size.width, max_content_width, theme);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{SpanStyle, StyledLine, StyledSpan};

    fn line_text(line: &StyledLine) -> String {
        line.spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn wrap_short_line_unchanged() {
        let line = StyledLine {
            spans: vec![StyledSpan::plain("hello world")],
        };
        let out = wrap_styled_line(&line, 80);
        assert_eq!(out.len(), 1);
        assert_eq!(line_text(&out[0]), "hello world");
    }

    #[test]
    fn wrap_breaks_at_word_boundary() {
        let line = StyledLine {
            spans: vec![StyledSpan::plain("the quick brown fox jumps over")],
        };
        let out = wrap_styled_line(&line, 12);
        assert!(out.len() >= 2, "expected multiple wrapped lines");
        for l in &out {
            let w = unicode_width::UnicodeWidthStr::width(line_text(l).as_str());
            assert!(w <= 12, "line `{}` width {} exceeds 12", line_text(l), w);
        }
    }

    #[test]
    fn wrap_preserves_leading_indent_on_continuation() {
        let line = StyledLine {
            spans: vec![StyledSpan::plain(
                "  * a list item with enough text to need wrapping at this width",
            )],
        };
        let out = wrap_styled_line(&line, 20);
        assert!(out.len() >= 2);
        // Continuation lines should start with at least 2 spaces of indent.
        for l in out.iter().skip(1) {
            let t = line_text(l);
            assert!(t.starts_with("  "), "continuation lacks indent: `{}`", t);
        }
    }

    #[test]
    fn wrap_preserves_span_styles() {
        let line = StyledLine {
            spans: vec![
                StyledSpan {
                    text: "bold word ".to_string(),
                    style: SpanStyle {
                        bold: true,
                        ..Default::default()
                    },
                },
                StyledSpan {
                    text: "and italic that wraps around".to_string(),
                    style: SpanStyle {
                        italic: true,
                        ..Default::default()
                    },
                },
            ],
        };
        let out = wrap_styled_line(&line, 12);
        assert!(out.len() >= 2);
        let saw_bold = out
            .iter()
            .flat_map(|l| l.spans.iter())
            .any(|s| s.style.bold);
        let saw_italic = out
            .iter()
            .flat_map(|l| l.spans.iter())
            .any(|s| s.style.italic);
        assert!(saw_bold && saw_italic, "styles dropped during wrap");
    }

    // ─── Headless pager rendering (ratatui TestBackend) ────────────────────
    //
    // These tests drive PagerState exactly as the event loop does, but render
    // into an in-memory buffer instead of a real terminal. That lets us assert
    // on what actually lands on screen (margins, indents, collapse markers)
    // rather than on intermediate data structures.

    use crate::render::{DiagramKind, RenderedBlock};
    use ratatui::backend::TestBackend;

    fn theme() -> &'static crate::theme::Theme {
        crate::theme::Theme::default_theme()
    }

    fn para(text: &str) -> RenderedBlock {
        RenderedBlock::Lines(vec![StyledLine {
            spans: vec![StyledSpan::plain(text)],
        }])
    }

    /// A fake flowchart: `rows` lines, each `cols` box-drawing characters wide.
    fn box_diagram(cols: usize, rows: usize) -> RenderedBlock {
        let line = StyledLine {
            spans: vec![StyledSpan::plain("─".repeat(cols))],
        };
        RenderedBlock::Diagram {
            lines: vec![line; rows],
            node_count: 3,
            edge_count: 2,
            kind: DiagramKind::Flowchart,
        }
    }

    /// Render the pager into a `w`×`h` buffer and return one String per row.
    fn screen(state: &PagerState, w: u16, h: u16) -> Vec<String> {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| state.draw_content(f, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
                    .collect::<String>()
            })
            .collect()
    }

    fn leading_spaces(row: &str) -> usize {
        row.chars().take_while(|c| *c == ' ').count()
    }

    #[test]
    fn content_width_helper_caps_and_uncaps() {
        assert_eq!(content_width(80, 100), 80);
        assert_eq!(content_width(200, 100), 100);
        assert_eq!(content_width(200, 0), 200);
    }

    #[test]
    fn wide_terminal_centers_content_between_equal_margins() {
        let long = "word ".repeat(60); // 300 cols, must wrap at the column width
        let state = PagerState::new(vec![para("Title"), para(&long)], 40, 200, 100, theme());
        assert_eq!(state.content_width, 100);
        assert_eq!(state.gutter, 50);

        let rows = screen(&state, 200, 40);
        assert_eq!(leading_spaces(&rows[0]), 50, "row: {:?}", rows[0]);
        assert!(rows[0].trim_start().starts_with("Title"));
        // Every wrapped prose row starts at the gutter and ends inside the column.
        for row in rows.iter().filter(|r| !r.trim().is_empty()) {
            assert_eq!(leading_spaces(row), 50, "row not at gutter: {:?}", row);
            let right_edge = row.trim_end().chars().count();
            assert!(right_edge <= 150, "row overflows column: {:?}", row);
        }
    }

    #[test]
    fn narrow_terminal_has_no_margin() {
        let state = PagerState::new(vec![para("hello")], 20, 80, 100, theme());
        assert_eq!(state.content_width, 80);
        assert_eq!(state.gutter, 0);
        let rows = screen(&state, 80, 20);
        assert!(rows[0].starts_with("hello"));
    }

    #[test]
    fn max_width_zero_disables_margins() {
        let state = PagerState::new(vec![para("hello")], 20, 200, 0, theme());
        assert_eq!(state.content_width, 200);
        assert_eq!(state.gutter, 0);
    }

    #[test]
    fn resize_recomputes_margins() {
        let mut state = PagerState::new(vec![para("hello")], 20, 80, 100, theme());
        assert_eq!(state.gutter, 0);
        state.terminal_width = 160;
        state.rebuild_flat_lines();
        assert_eq!(state.content_width, 100);
        assert_eq!(state.gutter, 30);
        state.terminal_width = 60;
        state.rebuild_flat_lines();
        assert_eq!(state.content_width, 60);
        assert_eq!(state.gutter, 0);
    }

    #[test]
    fn collapse_measures_columns_not_bytes() {
        // 70 box-drawing chars = 210 bytes but only 70 columns. On an 80-col
        // terminal this must stay expanded (the old byte-based check tripped
        // the "wider than 2× terminal" rule at 160 bytes).
        let state = PagerState::new(vec![box_diagram(70, 5)], 40, 80, 0, theme());
        let collapsed = state
            .flat_lines
            .iter()
            .any(|f| matches!(f, FlatLine::DiagramCollapsed { .. }));
        assert!(
            !collapsed,
            "70-column diagram was collapsed on an 80-col terminal"
        );

        // Genuinely wide (more than 2× terminal) still collapses.
        let state = PagerState::new(vec![box_diagram(170, 5)], 40, 80, 0, theme());
        let collapsed = state
            .flat_lines
            .iter()
            .any(|f| matches!(f, FlatLine::DiagramCollapsed { .. }));
        assert!(
            collapsed,
            "170-column diagram should collapse on an 80-col terminal"
        );
    }

    #[test]
    fn collapsed_indicator_sits_at_the_gutter() {
        // Taller than the terminal → collapsed; indicator must be indented
        // like the surrounding prose.
        let state = PagerState::new(
            vec![para("intro"), box_diagram(20, 50)],
            10,
            160,
            100,
            theme(),
        );
        let rows = screen(&state, 160, 10);
        let indicator = rows
            .iter()
            .find(|r| r.contains("Enter to expand"))
            .expect("collapsed indicator not drawn");
        // It is the only interactive block in view, so it is active ("▸ ...")
        // and its marker sits exactly at the gutter.
        assert_eq!(leading_spaces(indicator), 30, "indicator: {:?}", indicator);
        assert!(indicator.trim_start().starts_with("▸ [Flowchart"));
    }

    #[test]
    fn wrap_indented_line_without_spaces_never_exceeds_width() {
        // Regression: the leading indent used to count as a break point, so a
        // 2-space-indented code line with no other spaces produced a 1-column
        // first row and a continuation row `indent` columns too wide.
        let line = StyledLine {
            spans: vec![StyledSpan::plain(format!("  {}", "+".repeat(90)))],
        };
        let out = wrap_styled_line(&line, 40);
        assert!(out.len() >= 3);
        for l in &out {
            let w = unicode_width::UnicodeWidthStr::width(line_text(l).as_str());
            assert!(w <= 40, "row `{}` width {} exceeds 40", line_text(l), w);
            assert!(!line_text(l).trim().is_empty(), "produced an empty row");
        }
        assert_eq!(line_text(&out[0]).chars().count(), 40);
    }

    #[test]
    fn diagram_indent_follows_its_width() {
        // Terminal 120, cap 100 → gutter 10.
        let blocks = vec![box_diagram(60, 2), box_diagram(110, 2), box_diagram(130, 2)];
        let state = PagerState::new(blocks, 40, 120, 100, theme());
        let indents: Vec<usize> = state
            .flat_lines
            .iter()
            .filter_map(|f| match f {
                FlatLine::DiagramAscii { indent, .. } => Some(*indent),
                _ => None,
            })
            .collect();
        // Fits the column → shares the gutter; fits the terminal → centered
        // on the terminal; wider than the terminal → flush left.
        assert_eq!(indents, vec![10, 10, 5, 5, 0, 0]);

        let rows = screen(&state, 120, 40);
        let diagram_rows: Vec<&String> = rows.iter().filter(|r| r.contains('─')).collect();
        assert_eq!(leading_spaces(diagram_rows[0]), 10);
        assert_eq!(leading_spaces(diagram_rows[2]), 5);
        assert_eq!(leading_spaces(diagram_rows[4]), 0);
        // The 130-col diagram is clipped by the screen, never pushed off it.
        assert!(diagram_rows[4].chars().all(|c| c == '─'));
    }

    #[test]
    fn active_marker_uses_gutter_column_without_shifting_diagram() {
        // Expanded large diagram (taller than terminal) on a wide screen.
        let mut state = PagerState::new(vec![box_diagram(20, 50)], 10, 160, 100, theme());
        state.active = Some(0);
        state.activate_current(); // expand
        let rows = screen(&state, 160, 10);
        let row = rows.iter().find(|r| r.contains('─')).unwrap();
        assert_eq!(
            leading_spaces(row),
            29,
            "marker should occupy the last gutter column"
        );
        assert!(row.trim_start().starts_with('▎'));
        assert_eq!(row.chars().position(|c| c == '─'), Some(30));
    }

    /// Fixture sweep: every example document, at several terminal widths,
    /// must produce prose rows that fit the content column, with the
    /// gutter symmetric. This is the width-invariant that margins depend on.
    #[test]
    fn fixture_sweep_prose_fits_content_column_at_every_width() {
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/examples");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let input = std::fs::read_to_string(&path).unwrap();
            let blocks = crate::parser::parse_markdown(&input);
            for &term_w in &[40u16, 80, 120, 200] {
                let cw = content_width(term_w, DEFAULT_MAX_CONTENT_WIDTH);
                let rendered = crate::render::render_blocks(
                    &blocks,
                    cw,
                    &highlighter,
                    theme(),
                    crate::render::MermaidMode::Render,
                );
                let state =
                    PagerState::new(rendered, 50, term_w, DEFAULT_MAX_CONTENT_WIDTH, theme());
                assert_eq!(
                    state.gutter,
                    (term_w as usize - cw as usize) / 2,
                    "{}: gutter at width {}",
                    path.display(),
                    term_w
                );
                for fl in &state.flat_lines {
                    if let FlatLine::Styled(line) = fl {
                        let w = line_width(line);
                        assert!(
                            w <= cw as usize,
                            "{} @ {}: prose row {} cols exceeds column {}: {:?}",
                            path.display(),
                            term_w,
                            w,
                            cw,
                            line.spans
                                .iter()
                                .map(|s| s.text.as_str())
                                .collect::<String>()
                        );
                    }
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "no fixtures found in {}", dir.display());
    }

    #[test]
    fn diagram_kind_label_uses_kind_specific_nouns() {
        assert_eq!(DiagramKind::Flowchart.count_nouns(), ("nodes", "edges"));
        assert_eq!(DiagramKind::Er.count_nouns(), ("entities", "relationships"));
        assert_eq!(
            DiagramKind::Sequence.count_nouns(),
            ("participants", "events")
        );
        assert_eq!(DiagramKind::Er.label(), "ER Diagram");
    }
}
