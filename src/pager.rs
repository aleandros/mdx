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

enum FlatLine {
    Styled(StyledLine),
    DiagramAscii(String),
    DiagramCollapsed {
        block_index: usize,
        node_count: usize,
        edge_count: usize,
    },
}

// ─── PagerState ────────────────────────────────────────────────────────────

struct PagerState {
    content: Vec<RenderedBlock>,
    flat_lines: Vec<FlatLine>,
    scroll: usize,
    expanded: HashSet<usize>,
    terminal_height: u16,
}

impl PagerState {
    fn new(content: Vec<RenderedBlock>, terminal_height: u16) -> Self {
        let mut state = PagerState {
            content,
            flat_lines: Vec::new(),
            scroll: 0,
            expanded: HashSet::new(),
            terminal_height,
        };
        state.rebuild_flat_lines();
        state
    }

    fn rebuild_flat_lines(&mut self) {
        self.flat_lines.clear();
        let threshold = (self.terminal_height as usize) / 2;

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
                    let is_large = lines.len() > threshold;
                    if is_large && !self.expanded.contains(&block_index) {
                        self.flat_lines.push(FlatLine::DiagramCollapsed {
                            block_index,
                            node_count: *node_count,
                            edge_count: *edge_count,
                        });
                    } else {
                        for line in lines {
                            self.flat_lines.push(FlatLine::DiagramAscii(line.clone()));
                        }
                    }
                }
                RenderedBlock::Image { alt, url } => {
                    let text = if alt.is_empty() {
                        format!("[Image]({})", url)
                    } else {
                        format!("[Image: {}]({})", alt, url)
                    };
                    self.flat_lines.push(FlatLine::DiagramAscii(text));
                }
            }
        }
    }

    fn max_scroll(&self) -> usize {
        let total = self.flat_lines.len();
        let height = self.terminal_height as usize;
        total.saturating_sub(height)
    }

    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll > max {
            self.scroll = max;
        }
    }

    fn toggle_diagram_at_scroll(&mut self) {
        let height = self.terminal_height as usize;
        let start = self.scroll;
        let end = (self.scroll + height).min(self.flat_lines.len());

        // Find the first DiagramCollapsed in the viewport
        let mut found_index: Option<usize> = None;
        for i in start..end {
            if let FlatLine::DiagramCollapsed { block_index, .. } = &self.flat_lines[i] {
                found_index = Some(*block_index);
                break;
            }
        }

        if let Some(block_index) = found_index {
            if self.expanded.contains(&block_index) {
                self.expanded.remove(&block_index);
            } else {
                self.expanded.insert(block_index);
            }
            self.rebuild_flat_lines();
            self.clamp_scroll();
            return;
        }

        // If no collapsed diagram found, look for expanded DiagramAscii lines to collapse
        // by scanning block boundaries — find any diagram block visible in viewport
        // and if it's expanded, collapse it
        let mut found_expanded: Option<usize> = None;
        for (block_index, block) in self.content.iter().enumerate() {
            if matches!(block, RenderedBlock::Diagram { .. })
                && self.expanded.contains(&block_index)
            {
                found_expanded = Some(block_index);
                break;
            }
        }
        if let Some(block_index) = found_expanded {
            self.expanded.remove(&block_index);
            self.rebuild_flat_lines();
            self.clamp_scroll();
        }
    }

    fn flat_line_to_ratatui(flat: &FlatLine) -> Line<'static> {
        match flat {
            FlatLine::Styled(line) => styled_line_to_ratatui(line),
            FlatLine::DiagramAscii(text) => Line::raw(text.clone()),
            FlatLine::DiagramCollapsed {
                node_count,
                edge_count,
                ..
            } => {
                let text = format!(
                    "  [Flowchart: {} nodes, {} edges — Tab to expand]",
                    node_count, edge_count
                );
                Line::from(Span::styled(
                    text,
                    Style::default()
                        .fg(RColor::Cyan)
                        .add_modifier(Modifier::DIM),
                ))
            }
        }
    }
}

// ─── Public entry point ────────────────────────────────────────────────────

pub fn run_pager(content: Vec<RenderedBlock>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let term_height = terminal.size()?.height;
    let mut state = PagerState::new(content, term_height);

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let height = area.height as usize;
            let lines: Vec<Line> = state
                .flat_lines
                .iter()
                .skip(state.scroll)
                .take(height)
                .map(PagerState::flat_line_to_ratatui)
                .collect();
            let paragraph = Paragraph::new(lines);
            f.render_widget(paragraph, area);
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
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.scroll = state.scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        let max = state.max_scroll();
                        state.scroll = (state.scroll + page).min(max);
                    }
                    KeyCode::PageUp => {
                        state.scroll = state.scroll.saturating_sub(page);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        state.scroll = 0;
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        state.scroll = state.max_scroll();
                    }
                    KeyCode::Tab => {
                        state.toggle_diagram_at_scroll();
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => {
                    let max = state.max_scroll();
                    state.scroll = (state.scroll + 3).min(max);
                }
                MouseEventKind::ScrollUp => {
                    state.scroll = state.scroll.saturating_sub(3);
                }
                _ => {}
            },
            Event::Resize(_, h) => {
                state.terminal_height = h;
                state.rebuild_flat_lines();
                state.clamp_scroll();
            }
            _ => {}
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
