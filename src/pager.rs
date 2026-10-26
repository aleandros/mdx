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

/// Row `row` of image `id` as Kitty placeholder cells, colored with the id.
fn image_cells(id: u32, row: u16, cols: u16) -> Span<'static> {
    let (r, g, b) = crate::kitty::id_color(id);
    Span::styled(
        crate::kitty::placeholder_row(id, row, cols),
        Style::default().fg(RColor::Rgb(r, g, b)),
    )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleDirection {
    Forward,
    Backward,
}

pub(crate) enum KeyAction {
    Quit,
    Redraw,
    Nothing,
}

// ─── FlatLine ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
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
        block_index: usize,
        node_count: usize,
        edge_count: usize,
        kind: DiagramKind,
    },
    /// One row of a diagram drawn as image `id`. `line` is the ASCII row
    /// the image covers, kept for search.
    DiagramImage {
        line: StyledLine,
        indent: usize,
        id: u32,
        row: u16,
        cols: u16,
    },
    ImagePlaceholder {
        alt: String,
        url: String,
        block_index: usize,
    },
    /// One row of Kitty Unicode placeholders; the terminal draws row `row`
    /// of image `id` there. An image takes one of these per reserved row,
    /// followed by its `ImagePlaceholder` caption.
    ImageRow {
        block_index: usize,
        id: u32,
        row: u16,
        cols: u16,
    },
}

/// Word-wrap a styled line at `width` columns, preserving span styles.
/// Continuation lines are prefixed with the original leading-space indent
/// (capped) so wrapped paragraphs and list items keep their visual rhythm.
#[expect(
    clippy::too_many_lines,
    reason = "word wrapping tracks styled spans and display-cell widths together"
)]
fn wrap_styled_line(line: &StyledLine, width: usize) -> Vec<StyledLine> {
    if width == 0 {
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
            let cur = lines
                .last_mut()
                .expect("wrapping starts with one line and never removes it");
            if let Some(brk) = last_break {
                let tail: Vec<(char, SpanStyle)> = cur.drain(brk..).collect();
                // Drop the leading whitespace from tail
                let tail: Vec<(char, SpanStyle)> =
                    tail.into_iter().skip_while(|(c, _)| *c == ' ').collect();
                push_continuation(&mut lines, &mut cur_w);
                is_first_line = false;
                seen_content = !tail.is_empty();
                let cur = lines
                    .last_mut()
                    .expect("wrapping starts with one line and never removes it");
                for entry in tail {
                    cur.push(entry);
                }
                cur_w = lines
                    .last()
                    .expect("wrapping starts with one line and never removes it")
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

        let cur = lines
            .last_mut()
            .expect("wrapping starts with one line and never removes it");
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
    /// Diagrams (by block index) the user switched away from the session
    /// default with `i`: shown as ASCII when `diagram_images`, as images
    /// otherwise.
    pub(crate) toggled_diagrams: HashSet<usize>,
    /// Whether diagrams start as images when graphics are on
    /// (`--mermaid-graphics`).
    diagram_images: bool,
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
    /// Startup graphics decision.
    graphics: crate::graphics::GraphicsCaps,
    /// Directory that relative image paths are resolved against.
    base_dir: std::path::PathBuf,
    /// Inline images; present only when graphics are on.
    images: Option<crate::images::ImageStore>,
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
            toggled_diagrams: HashSet::new(),
            diagram_images: true,
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
            graphics: crate::graphics::GraphicsCaps::unsupported("not detected"),
            base_dir: std::path::PathBuf::from("."),
            images: None,
        };
        state.rebuild_flat_lines();
        state.update_active_from_viewport();
        state
    }

    /// Attach the startup graphics decision and the directory relative image
    /// paths start from. With graphics on, local images are shown inline.
    pub(crate) fn with_graphics(
        mut self,
        graphics: crate::graphics::GraphicsCaps,
        base_dir: std::path::PathBuf,
    ) -> Self {
        self.graphics = graphics;
        let no_color = std::env::var_os("NO_COLOR");
        if graphics.supported && crate::images::allowed_with_no_color(no_color.as_deref()) {
            self.images = Some(crate::images::ImageStore::new(
                base_dir.clone(),
                graphics.cell_px,
            ));
        }
        self.base_dir = base_dir;
        self.rebuild_flat_lines();
        self.update_active_from_viewport();
        self
    }

    /// Start diagrams as images (`On`) or as ASCII (`Off`) when graphics are
    /// on.
    pub(crate) fn with_mermaid_graphics(
        mut self,
        mermaid_graphics: crate::graphics::MermaidGraphics,
    ) -> Self {
        self.diagram_images = mermaid_graphics == crate::graphics::MermaidGraphics::On;
        self.rebuild_flat_lines();
        self.update_active_from_viewport();
        self
    }

    /// Use `images` for inline images, as `with_graphics` does with graphics on.
    #[cfg(test)]
    pub(crate) fn with_images(mut self, images: crate::images::ImageStore) -> Self {
        self.images = Some(images);
        self.rebuild_flat_lines();
        self.update_active_from_viewport();
        self
    }

    /// Most rows an image may take: 60% of the pager height.
    fn max_image_rows(&self) -> u16 {
        u16::try_from(u32::from(self.terminal_height) * 3 / 5)
            .unwrap_or(u16::MAX)
            .max(1)
    }

    /// Take finished image decodes and re-lay out if any changed. Returns
    /// whether a redraw is needed.
    pub(crate) fn poll_images(&mut self) -> bool {
        if !self
            .images
            .as_mut()
            .is_some_and(crate::images::ImageStore::poll)
        {
            return false;
        }
        self.rebuild_flat_lines();
        self.clamp_scroll();
        if self.active.is_none() {
            self.update_active_from_viewport();
        }
        true
    }

    /// Whether image decodes are in flight, so the event loop should wake up
    /// to collect them.
    pub(crate) fn images_loading(&self) -> bool {
        self.images
            .as_ref()
            .is_some_and(crate::images::ImageStore::loading)
    }

    /// Graphics escape sequences to write before the next frame.
    pub(crate) fn take_graphics_output(&mut self) -> Vec<u8> {
        self.images
            .as_mut()
            .map(crate::images::ImageStore::take_output)
            .unwrap_or_default()
    }

    /// Escape sequences that delete every transmitted image, for exit.
    pub(crate) fn delete_images(&mut self) -> Vec<u8> {
        self.images
            .as_mut()
            .map(crate::images::ImageStore::delete_all)
            .unwrap_or_default()
    }

    /// Check whether shown image files changed on disk (watch mode); changed
    /// ones decode again and show up through [`Self::poll_images`].
    pub(crate) fn refresh_images(&mut self) {
        if let Some(images) = &mut self.images {
            images.refresh();
        }
    }

    /// Record a new cell size in pixels, e.g. after a font size change.
    pub(crate) fn set_cell_px(&mut self, cell_px: (u16, u16)) {
        if let Some(images) = &mut self.images {
            images.set_cell_px(cell_px);
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match lays out every block kind in document order"
    )]
    pub(crate) fn rebuild_flat_lines(&mut self) {
        let prev_block_index = self.active.map(|i| self.interactive_blocks[i].block_index);

        self.flat_lines.clear();
        self.interactive_blocks.clear();
        let height_threshold = usize::from(self.terminal_height);
        let terminal_width = usize::from(self.terminal_width);

        // Content column: capped at max_content_width and centered. Text wraps
        // at the column width; the gutter is applied at draw time.
        self.content_width =
            usize::from(content_width(self.terminal_width, self.max_content_width));
        self.gutter = (terminal_width - self.content_width) / 2;
        let wrap_width = self.content_width;
        let max_image_rows = self.max_image_rows();

        for (block_index, block) in self.content.iter().enumerate() {
            match block {
                RenderedBlock::Lines(lines) => {
                    if lines.is_empty() {
                        self.flat_lines.push(FlatLine::Styled(StyledLine::empty()));
                    }
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
                    image,
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

                    // With graphics on, `i` switches a diagram between its
                    // image and ASCII, so every such diagram is selectable.
                    let switchable = image.is_some() && self.images.is_some();
                    let show_image =
                        self.diagram_images != self.toggled_diagrams.contains(&block_index);

                    // Pad with a blank line above so collapsed indicators (and
                    // expanded diagrams) don't run flush against neighboring
                    // text. The active diagram's image/ASCII hint goes here.
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
                    } else {
                        let flat_line_index = self.flat_lines.len();
                        // With graphics on, the diagram is drawn as an image
                        // over the same cells, once it has been rasterized.
                        let placed = image
                            .as_deref()
                            .filter(|_| show_image)
                            .zip(self.images.as_mut())
                            .and_then(|(image, images)| images.diagram(image));
                        for (row, line) in lines.iter().enumerate() {
                            let line = line.clone();
                            self.flat_lines.push(match placed {
                                Some(placed) => FlatLine::DiagramImage {
                                    line,
                                    indent,
                                    id: placed.id,
                                    row: u16::try_from(row).unwrap_or(u16::MAX),
                                    cols: placed.cols,
                                },
                                None => FlatLine::DiagramAscii { line, indent },
                            });
                        }
                        if is_large || switchable {
                            self.interactive_blocks.push(InteractiveEntry {
                                block_index,
                                flat_line_index,
                                flat_line_end: flat_line_index + lines.len(),
                            });
                        }
                    }

                    // Trailing blank for breathing room below the diagram.
                    self.flat_lines.push(FlatLine::Styled(StyledLine::empty()));
                }
                RenderedBlock::Image { alt, url } => {
                    let flat_line_index = self.flat_lines.len();
                    let max_cols = u16::try_from(self.content_width).unwrap_or(u16::MAX);
                    let layout = self
                        .images
                        .as_mut()
                        .and_then(|images| images.layout(url, max_cols, max_image_rows));
                    if let Some(layout) = layout {
                        for row in 0..layout.rows {
                            self.flat_lines.push(FlatLine::ImageRow {
                                block_index,
                                id: layout.id,
                                row,
                                cols: layout.cols,
                            });
                        }
                    }
                    self.flat_lines.push(FlatLine::ImagePlaceholder {
                        alt: alt.clone(),
                        url: url.clone(),
                        block_index,
                    });
                    self.interactive_blocks.push(InteractiveEntry {
                        block_index,
                        flat_line_index,
                        flat_line_end: self.flat_lines.len(),
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

    pub(crate) fn resize(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;
        self.rebuild_flat_lines();
        self.clamp_scroll();
        if self.active.is_none() {
            self.update_active_from_viewport();
        }
    }

    pub(crate) fn replace_content(&mut self, content: Vec<RenderedBlock>) {
        self.content = content;
        self.expanded
            .retain(|&i| matches!(self.content.get(i), Some(RenderedBlock::Diagram { .. })));
        self.toggled_diagrams
            .retain(|&i| matches!(self.content.get(i), Some(RenderedBlock::Diagram { .. })));
        if let Some(images) = &mut self.images {
            images.retain(self.content.iter().filter_map(|block| match block {
                RenderedBlock::Image { url, .. } => Some(url.as_str()),
                _ => None,
            }));
            images.retain_diagrams(self.content.iter().filter_map(|block| match block {
                RenderedBlock::Diagram {
                    image: Some(image), ..
                } => Some(image.key()),
                _ => None,
            }));
            images.refresh();
        }
        self.rebuild_flat_lines();
        self.clamp_scroll();
        self.update_active_from_viewport();
    }

    pub(crate) fn max_scroll(&self) -> usize {
        let total = self.flat_lines.len();
        let height = usize::from(self.terminal_height);
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
        let end = (self.scroll + usize::from(self.terminal_height)).min(self.flat_lines.len());

        self.active = self
            .interactive_blocks
            .iter()
            .position(|entry| entry.flat_line_index >= start && entry.flat_line_index < end);
    }

    pub(crate) fn cycle_active(&mut self, direction: CycleDirection) {
        if self.interactive_blocks.is_empty() {
            return;
        }

        let len = self.interactive_blocks.len();
        self.active = Some(match self.active {
            None => 0,
            Some(i) if direction == CycleDirection::Forward => (i + 1) % len,
            Some(i) => (i + len - 1) % len,
        });

        // Scroll to make active block visible
        if let Some(idx) = self.active {
            let flat_idx = self.interactive_blocks[idx].flat_line_index;
            let height = usize::from(self.terminal_height);
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
                // Open local files from the markdown file's directory.
                let target = crate::images::resolve(url, &self.base_dir)
                    .filter(|path| path.exists())
                    .map_or_else(
                        || std::ffi::OsString::from(url),
                        std::path::PathBuf::into_os_string,
                    );
                if let Some(opener) = self.opener.as_ref() {
                    let _ = std::process::Command::new(opener)
                        .arg(&target)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
            }
            RenderedBlock::Lines(_) => {}
        }
    }

    /// Whether `i` can switch block `block_index` between image and ASCII:
    /// graphics are on and the block is a diagram the SVG backend can draw.
    fn can_switch_diagram(&self, block_index: usize) -> bool {
        self.images.is_some()
            && matches!(
                self.content.get(block_index),
                Some(RenderedBlock::Diagram { image: Some(_), .. })
            )
    }

    /// Whether diagram `block_index` is (or, collapsed, would be) drawn as
    /// an image rather than ASCII.
    fn diagram_shows_image(&self, block_index: usize) -> bool {
        self.diagram_images != self.toggled_diagrams.contains(&block_index)
    }

    /// Switch diagram `block_index` between image and ASCII. Image rows and
    /// ASCII rows map one to one, so neither the scroll position nor the
    /// search matches move.
    fn switch_diagram(&mut self, block_index: usize) {
        if !self.toggled_diagrams.remove(&block_index) {
            self.toggled_diagrams.insert(block_index);
        }
        let current = self.search_current;
        self.rebuild_flat_lines();
        if current.is_some_and(|c| c < self.search_matches.len()) {
            self.search_current = current;
        }
    }

    /// The `i` key: switch the active diagram between its image and its
    /// ASCII drawing. A collapsed diagram stays collapsed and shows the
    /// chosen form when expanded. Returns whether anything changed; without
    /// graphics it never does.
    pub(crate) fn toggle_active_diagram_image(&mut self) -> bool {
        let Some(block_index) = self.active_entry().map(|e| e.block_index) else {
            return false;
        };
        if !self.can_switch_diagram(block_index) {
            return false;
        }
        self.switch_diagram(block_index);
        true
    }

    /// Switch the diagram holding flat line `line` to ASCII if it is drawn
    /// (or waiting to be drawn) as an image, so a search match inside it is
    /// visible.
    fn show_ascii_at(&mut self, line: usize) {
        let block_index = self
            .interactive_blocks
            .iter()
            .find(|e| (e.flat_line_index..e.flat_line_end).contains(&line))
            .map(|e| e.block_index);
        if let Some(block_index) = block_index
            && self.can_switch_diagram(block_index)
            && self.diagram_shows_image(block_index)
        {
            self.switch_diagram(block_index);
        }
    }

    /// The hint drawn on the blank row above the active diagram, naming the
    /// form it is in and the key that switches it. `None` for every other
    /// row, and whenever `i` would do nothing.
    fn diagram_hint(&self, flat_line_index: usize) -> Option<Line<'static>> {
        let entry = self.active_entry()?;
        if entry.flat_line_index != flat_line_index + 1
            || !self.can_switch_diagram(entry.block_index)
        {
            return None;
        }
        let text = if !self.diagram_shows_image(entry.block_index) {
            "[ASCII — i for image]"
        } else if matches!(
            self.flat_lines.get(entry.flat_line_index),
            Some(FlatLine::DiagramAscii { .. })
        ) {
            let pending = match (&self.content[entry.block_index], &self.images) {
                (
                    RenderedBlock::Diagram {
                        image: Some(image), ..
                    },
                    Some(images),
                ) => images.diagram_pending(image),
                _ => false,
            };
            if pending {
                "[image loading — i for ASCII]"
            } else {
                "[image unavailable — i for ASCII]"
            }
        } else {
            "[image — i for ASCII]"
        };
        let color = color_to_ratatui(&self.theme.diagram_collapsed);
        Some(Line::from(Span::styled(
            format!("  {text}"),
            Style::default().fg(color).add_modifier(Modifier::DIM),
        )))
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
            FlatLine::Styled(line) => (
                self.gutter,
                self.diagram_hint(flat_line_index)
                    .unwrap_or_else(|| styled_line_to_ratatui(line)),
            ),
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
                    let text = format!("▸ {body}");
                    Line::from(Span::styled(text, Style::default().fg(collapsed_color)))
                } else {
                    let text = format!("  {body}");
                    Line::from(Span::styled(
                        text,
                        Style::default()
                            .fg(collapsed_color)
                            .add_modifier(Modifier::DIM),
                    ))
                };
                (self.gutter, line)
            }
            FlatLine::DiagramImage {
                indent,
                id,
                row,
                cols,
                ..
            } => {
                let cells = image_cells(*id, *row, *cols);
                // The active marker goes in the margin, as for an image.
                if *indent > 0 && self.is_in_active_block(flat_line_index) {
                    let marker = Span::styled("▎", Style::default().fg(collapsed_color));
                    (indent - 1, Line::from(vec![marker, cells]))
                } else {
                    (*indent, Line::from(cells))
                }
            }
            FlatLine::ImageRow { id, row, cols, .. } => {
                let cells = image_cells(*id, *row, *cols);
                // Same gutter marker as an active expanded diagram, but only
                // in a margin: without one it would push the last column off.
                if self.gutter > 0 && self.is_in_active_block(flat_line_index) {
                    let marker = Span::styled("▎", Style::default().fg(collapsed_color));
                    (
                        self.gutter.saturating_sub(1),
                        Line::from(vec![marker, cells]),
                    )
                } else {
                    (self.gutter, Line::from(cells))
                }
            }
            FlatLine::ImagePlaceholder { alt, .. } => {
                let (prefix, modifier) = if self.is_in_active_block(flat_line_index) {
                    ("▸", Modifier::empty())
                } else {
                    (" ", Modifier::DIM)
                };
                let text = if alt.is_empty() {
                    format!("{prefix} [Image — Enter to open]")
                } else {
                    format!("{prefix} [Image: {alt} — Enter to open]")
                };
                let line = Line::from(Span::styled(
                    text,
                    Style::default().fg(collapsed_color).add_modifier(modifier),
                ));
                (self.gutter, line)
            }
        }
    }

    pub(crate) fn draw_content(&self, f: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect) {
        let height = usize::from(area.height);
        let lines: Vec<Line<'_>> = self
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
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "preserve the pager's existing u16 horizontal-scroll behavior at the ratatui boundary"
        )]
        let paragraph = Paragraph::new(lines).scroll((0, self.h_scroll as u16));
        f.render_widget(paragraph, area);
    }
    // ─── Search ────────────────────────────────────────────────────────────

    fn flat_line_text(flat: &FlatLine) -> String {
        match flat {
            FlatLine::Styled(line)
            | FlatLine::DiagramAscii { line, .. }
            | FlatLine::DiagramImage { line, .. } => {
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
            FlatLine::ImageRow { .. } => String::new(),
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
                    let viewport_end = self.scroll + usize::from(self.terminal_height);
                    let pos = self.search_matches.iter().rposition(|&m| m < viewport_end);
                    self.search_current = Some(pos.unwrap_or(self.search_matches.len() - 1));
                }
            }
            self.scroll_to_current_match();
        }
    }

    /// Scroll the current match into view. A match inside a diagram drawn
    /// as an image switches that diagram to ASCII, where the text shows.
    fn scroll_to_current_match(&mut self) {
        if let Some(idx) = self.search_current {
            let line = self.search_matches[idx];
            self.show_ascii_at(line);
            let height = usize::from(self.terminal_height);
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
            let current = self.search_current.map_or(0, |c| c + 1);
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

    #[expect(
        clippy::too_many_lines,
        reason = "key dispatch keeps each navigation action visible in one match"
    )]
    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> KeyAction {
        if key.kind != KeyEventKind::Press {
            return KeyAction::Nothing;
        }

        if self.search_mode.is_some() {
            return self.handle_search_input(key);
        }

        let page = usize::from(self.terminal_height).saturating_sub(1).max(1);
        let half_page = (page / 2).max(1);

        // Check Ctrl combinations first
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('d') => {
                    let max = self.max_scroll();
                    let new = (self.scroll + half_page).min(max);
                    if new == self.scroll {
                        KeyAction::Nothing
                    } else {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    }
                }
                KeyCode::Char('u') => {
                    let new = self.scroll.saturating_sub(half_page);
                    if new == self.scroll {
                        KeyAction::Nothing
                    } else {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    }
                }
                KeyCode::Char('f') => {
                    let max = self.max_scroll();
                    let new = (self.scroll + page).min(max);
                    if new == self.scroll {
                        KeyAction::Nothing
                    } else {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
                    }
                }
                KeyCode::Char('b') => {
                    let new = self.scroll.saturating_sub(page);
                    if new == self.scroll {
                        KeyAction::Nothing
                    } else {
                        self.scroll = new;
                        self.update_active_from_viewport();
                        KeyAction::Redraw
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
                if new == self.scroll {
                    KeyAction::Nothing
                } else {
                    self.scroll = new;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
                }
            }
            KeyCode::PageUp => {
                let new = self.scroll.saturating_sub(page);
                if new == self.scroll {
                    KeyAction::Nothing
                } else {
                    self.scroll = new;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
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
                if self.scroll == max {
                    KeyAction::Nothing
                } else {
                    self.scroll = max;
                    self.update_active_from_viewport();
                    KeyAction::Redraw
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
                self.cycle_active(CycleDirection::Forward);
                KeyAction::Redraw
            }
            KeyCode::BackTab => {
                self.cycle_active(CycleDirection::Backward);
                KeyAction::Redraw
            }
            KeyCode::Enter => {
                self.activate_current();
                KeyAction::Redraw
            }
            KeyCode::Char('i') => {
                if self.toggle_active_diagram_image() {
                    KeyAction::Redraw
                } else {
                    KeyAction::Nothing
                }
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

/// A terminal that ignores the graphics query may have printed part of it;
/// clear the alternate screen so the first frame does not show it. Call it
/// before creating the ratatui `Terminal`, whose `clear` would query the
/// cursor position.
pub(crate) fn clear_after_probe(
    out: &mut impl std::io::Write,
    detection: &crate::graphics::Detection,
) -> std::io::Result<()> {
    if matches!(detection.probe, crate::graphics::Probe::NotSent) {
        Ok(())
    } else {
        execute!(
            out,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
        )
    }
}

/// Cell size in pixels from the terminal's window size, if it reports one.
pub(crate) fn current_cell_px() -> Option<(u16, u16)> {
    let size = crossterm::terminal::window_size().ok()?;
    if size.columns == 0 || size.rows == 0 || size.width == 0 || size.height == 0 {
        return None;
    }
    Some((size.width / size.columns, size.height / size.rows))
}

/// Write queued graphics escape sequences (image data, placements) to the
/// terminal. Call it before drawing a frame so placements exist before
/// their placeholder cells do.
pub(crate) fn write_graphics(out: &mut impl std::io::Write, bytes: &[u8]) -> std::io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    out.write_all(bytes)?;
    out.flush()
}

pub(crate) fn run_pager(
    content: Vec<RenderedBlock>,
    max_content_width: u16,
    theme: &'static crate::theme::Theme,
    graphics: crate::graphics::GraphicsSetting,
    mermaid_graphics: crate::graphics::MermaidGraphics,
    base_dir: std::path::PathBuf,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;
    let detection = crate::graphics::detect(graphics, crate::graphics::OutputTarget::Pager);
    clear_after_probe(&mut stdout, &detection)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let mut state = PagerState::new(content, size.height, size.width, max_content_width, theme)
        .with_graphics(detection.caps, base_dir)
        .with_mermaid_graphics(mermaid_graphics);

    let result = event_loop(&mut terminal, &mut state);
    // Free transmitted images before leaving the alternate screen.
    let _ = write_graphics(terminal.backend_mut(), &state.delete_images());
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state: &mut PagerState,
) -> Result<()> {
    loop {
        state.poll_images();
        write_graphics(terminal.backend_mut(), &state.take_graphics_output())?;
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

        // While images decode, wake up to show them as they arrive.
        if state.images_loading() && !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => match state.handle_key_event(key) {
                KeyAction::Quit => return Ok(()),
                KeyAction::Redraw | KeyAction::Nothing => {}
            },
            Event::Mouse(mouse) => {
                state.handle_mouse_event(mouse);
            }
            Event::Resize(w, h) => {
                if let Some(cell_px) = current_cell_px() {
                    state.set_cell_px(cell_px);
                }
                state.resize(w, h);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{SpanStyle, StyledLine, StyledSpan};

    #[test]
    fn clear_after_probe_only_clears_and_never_queries() {
        let detection = |probe| crate::graphics::Detection {
            caps: crate::graphics::GraphicsCaps::unsupported("test"),
            probe,
        };
        let mut out = Vec::new();
        clear_after_probe(&mut out, &detection(crate::graphics::Probe::NotSent)).unwrap();
        assert!(out.is_empty());
        clear_after_probe(&mut out, &detection(crate::graphics::Probe::TimedOut)).unwrap();
        // A clear only: no cursor-position or other query the pager would have to read.
        assert_eq!(out, b"\x1b[2J");
    }

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
            assert!(t.starts_with("  "), "continuation lacks indent: `{t}`");
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
            image: None,
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
                    .map(|x| buf.cell((x, y)).map_or(" ", ratatui::buffer::Cell::symbol))
                    .collect::<String>()
            })
            .collect()
    }

    fn leading_spaces(row: &str) -> usize {
        row.chars().take_while(|c| *c == ' ').count()
    }

    #[test]
    fn math_source_survives_pager_rendering() {
        let input = include_str!("../docs/examples/math.md");
        let blocks = crate::parser::parse_markdown(input);
        let highlighter = crate::highlight::Highlighter::new(None).unwrap();
        let rendered = crate::render::render_blocks(
            &blocks,
            80,
            &highlighter,
            theme(),
            crate::render::MermaidMode::Render,
        );
        let state = PagerState::new(rendered, 40, 80, 100, theme());
        let rows: Vec<String> = screen(&state, 80, 40)
            .into_iter()
            .map(|row| row.trim_end().to_string())
            .collect();
        let text = rows.join("\n");
        assert!(text.contains("Before $a^2$ after."), "{text}");
        assert!(text.contains("  $$\n  a^2 + b^2 = c^2\n  $$"), "{text}");
        assert!(text.contains("2. Next $y^2$"), "{text}");
        assert!(text.contains("Inline math across lines: $x y$."), "{text}");
    }

    #[test]
    fn halfwidth_sound_marks_do_not_swallow_following_text() {
        // Terminals draw each ﾞ/ﾟ mark in its own column. ratatui 0.29 gave
        // the base+mark grapheme a single cell, so the terminal shifted the
        // rest of the line and later cells overwrote it.
        let state = PagerState::new(vec![para("ｶﾞｷﾞ ﾊﾟ and more")], 5, 40, 100, theme());
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| state.draw_content(f, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        let and_col = (0..40u16).find(|&x| buf.cell((x, 0)).is_some_and(|c| c.symbol() == "a"));
        // "ｶﾞｷﾞ ﾊﾟ " is 2+2+1+2+1 columns wide.
        assert_eq!(and_col, Some(8));
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
            assert_eq!(leading_spaces(row), 50, "row not at gutter: {row:?}");
            let right_edge = row.trim_end().chars().count();
            assert!(right_edge <= 150, "row overflows column: {row:?}");
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
        assert_eq!(leading_spaces(indicator), 30, "indicator: {indicator:?}");
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
                    (usize::from(term_w) - usize::from(cw)) / 2,
                    "{}: gutter at width {}",
                    path.display(),
                    term_w
                );
                for fl in &state.flat_lines {
                    if let FlatLine::Styled(line) = fl {
                        let w = line_width(line);
                        assert!(
                            w <= usize::from(cw),
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

    // ─── Inline images (Kitty Unicode placeholders) ───────────────────────

    fn image(alt: &str, url: &str) -> RenderedBlock {
        RenderedBlock::Image {
            alt: alt.into(),
            url: url.into(),
        }
    }

    /// A pager whose store already holds `a.png` at `px` pixels, with
    /// 10x20-pixel cells. Returns the image id.
    fn image_pager(
        content: Vec<RenderedBlock>,
        h: u16,
        w: u16,
        px: (u32, u32),
    ) -> (PagerState, u32) {
        let mut store = crate::images::ImageStore::for_tests("/docs".into(), (10, 20));
        let id = store.insert_ready("a.png", px);
        let mut state = PagerState::new(content, h, w, 100, theme()).with_images(store);
        state.opener = None;
        (state, id)
    }

    fn image_rows(state: &PagerState) -> Vec<(usize, u32, u16, u16)> {
        state
            .flat_lines
            .iter()
            .filter_map(|f| match f {
                FlatLine::ImageRow {
                    block_index,
                    id,
                    row,
                    cols,
                } => Some((*block_index, *id, *row, *cols)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn image_reserves_one_placeholder_row_per_cell_row() {
        // 400x200 px in 10x20 cells: 40 columns, 10 rows.
        let (state, id) = image_pager(
            vec![para("intro"), image("cat", "a.png"), para("outro")],
            40,
            80,
            (400, 200),
        );
        let rows = image_rows(&state);
        assert_eq!(rows.len(), 10);
        for (r, &(block_index, row_id, row, cols)) in rows.iter().enumerate() {
            assert_eq!((block_index, row_id, cols), (1, id, 40));
            assert_eq!(usize::from(row), r);
        }
        // The caption follows the image, and the block spans both.
        assert!(matches!(
            state.flat_lines[11],
            FlatLine::ImagePlaceholder { block_index: 1, .. }
        ));
        let entry = &state.interactive_blocks[0];
        assert_eq!((entry.flat_line_index, entry.flat_line_end), (1, 12));
    }

    #[test]
    fn image_cells_carry_placeholders_and_the_id_color() {
        let (state, id) = image_pager(vec![image("cat", "a.png")], 40, 120, (400, 200));
        // Terminal 120, cap 100 → gutter 10.
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| state.draw_content(f, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        let (r, g, b) = crate::kitty::id_color(id);
        for y in 0..10u16 {
            for x in 0..40u16 {
                let cell = buf.cell((10 + x, y)).unwrap();
                let expected = crate::kitty::placeholder_row(id, y, 40)
                    .chars()
                    .skip(usize::from(x) * 3)
                    .take(3)
                    .collect::<String>();
                assert_eq!(cell.symbol(), expected, "cell ({x}, {y})");
                assert_eq!(cell.fg, RColor::Rgb(r, g, b));
            }
            assert_eq!(
                buf.cell((50, y)).unwrap().symbol(),
                " ",
                "nothing past the image"
            );
        }
        let caption: String = (0..120u16)
            .map(|x| buf.cell((x, 10)).unwrap().symbol().to_string())
            .collect();
        assert!(
            caption.contains("[Image: cat — Enter to open]"),
            "{caption}"
        );
    }

    #[test]
    fn active_image_marks_the_gutter_without_shifting_cells() {
        let (mut state, _) = image_pager(vec![image("cat", "a.png")], 40, 120, (400, 200));
        state.active = Some(0);
        let rows = screen(&state, 120, 40);
        for row in &rows[..10] {
            assert_eq!(leading_spaces(row), 9, "{row:?}");
            assert_eq!(row.chars().nth(9), Some('▎'));
            assert_eq!(row.chars().nth(10), Some(crate::kitty::PLACEHOLDER));
        }
        assert!(rows[10].trim_start().starts_with("▸ [Image: cat"));
    }

    #[test]
    fn active_image_without_a_margin_has_no_marker() {
        // 80 columns, cap 100 → no gutter; a full-width image must not shift.
        let (mut state, _) = image_pager(vec![image("wide", "a.png")], 40, 80, (2000, 400));
        state.active = Some(0);
        let rows = screen(&state, 80, 40);
        assert!(
            rows[0].chars().all(|c| c == crate::kitty::PLACEHOLDER)
                || rows[0].starts_with(crate::kitty::PLACEHOLDER)
        );
        assert_eq!(
            rows[0]
                .chars()
                .filter(|&c| c == crate::kitty::PLACEHOLDER)
                .count(),
            80
        );
        assert!(!rows[0].contains('▎'));
    }

    #[test]
    fn image_height_is_capped_at_60_percent_of_the_pager() {
        // 200x2000 px would be 100 rows; a 20-row pager allows 12.
        let (state, _) = image_pager(vec![image("tall", "a.png")], 20, 80, (200, 2000));
        let rows = image_rows(&state);
        assert_eq!(rows.len(), 12);
        // 12 rows of 20 px = 240 px high → 24 px wide, rounded up to 3 cells.
        assert_eq!(rows[0].3, 3, "width scaled with the height");
    }

    #[test]
    fn resize_refits_the_image_and_places_it_again() {
        let (mut state, id) = image_pager(vec![image("wide", "a.png")], 40, 120, (2000, 400));
        // 2000x400 px into 100 columns (1000 px) → 200 px = 10 rows.
        assert_eq!(image_rows(&state).len(), 10);
        assert_eq!(image_rows(&state)[0].3, 100);
        assert_eq!(
            state.take_graphics_output(),
            crate::kitty::place(id, 100, 10)
        );
        state.resize(60, 40);
        // 60 columns (600 px) → 120 px = 6 rows.
        assert_eq!(image_rows(&state).len(), 6);
        assert_eq!(image_rows(&state)[0].3, 60);
        assert_eq!(state.take_graphics_output(), crate::kitty::place(id, 60, 6));
        // Same size again: no new placement.
        state.resize(60, 40);
        assert!(state.take_graphics_output().is_empty());
    }

    #[test]
    fn search_and_tab_work_across_image_rows() {
        let mut content = vec![para("top")];
        content.extend((0..30).map(|i| para(&format!("filler {i}"))));
        content.push(image("needle", "a.png"));
        content.push(para("after the needle"));
        let (mut state, _) = image_pager(content, 10, 80, (400, 200));
        // 10-row pager → at most 6 image rows.
        assert_eq!(image_rows(&state).len(), 6);
        state.search_query = "needle".into();
        state.recompute_matches();
        // The caption and the following paragraph match; image rows do not.
        assert_eq!(state.search_matches, vec![37, 38]);

        state.scroll = 0;
        state.update_active_from_viewport();
        assert_eq!(state.active, None);
        state.cycle_active(CycleDirection::Forward);
        assert_eq!(state.active, Some(0));
        let first_row = state.interactive_blocks[0].flat_line_index;
        assert_eq!(first_row, 31);
        assert!(state.scroll <= first_row && first_row < state.scroll + 10);
        for i in 31..38 {
            assert!(state.is_in_active_block(i), "line {i} in the active image");
        }
    }

    #[test]
    fn graphics_off_and_unusable_images_keep_the_text_placeholder() {
        let content = vec![
            image("remote", "https://example.com/a.png"),
            image("loading", "b.png"),
            image("ready", "a.png"),
        ];
        let plain = PagerState::new(content.clone(), 40, 80, 100, theme());
        assert!(image_rows(&plain).is_empty());
        let plain_screen = screen(&plain, 80, 40);

        let (state, _) = image_pager(content, 40, 80, (400, 200));
        // Only the ready local image gets rows; the others render as today.
        assert_eq!(image_rows(&state).len(), 10);
        let rows = screen(&state, 80, 40);
        assert_eq!(rows[0], plain_screen[0]);
        assert_eq!(rows[1], plain_screen[1]);
    }

    /// A real diagram block, with its SVG-backend image.
    fn mermaid(source: &str) -> RenderedBlock {
        let diagram = crate::mermaid::render_diagram(source, theme(), 80).unwrap();
        RenderedBlock::Diagram {
            lines: diagram.lines,
            node_count: diagram.primary_count,
            edge_count: diagram.secondary_count,
            kind: DiagramKind::Flowchart,
            image: Some(std::sync::Arc::new(diagram.image)),
        }
    }

    /// A pager over `content` with graphics on and every diagram rasterized.
    fn diagram_pager(content: Vec<RenderedBlock>, h: u16, w: u16) -> (PagerState, Vec<u32>) {
        let mut store = crate::images::ImageStore::for_tests("/docs".into(), (10, 20));
        let ids = content
            .iter()
            .filter_map(|block| match block {
                RenderedBlock::Diagram {
                    image: Some(image), ..
                } => Some(store.insert_ready_diagram(image)),
                _ => None,
            })
            .collect();
        let state = PagerState::new(content, h, w, 100, theme()).with_images(store);
        (state, ids)
    }

    fn diagram_image_rows(state: &PagerState) -> Vec<(usize, u32, u16, u16)> {
        state
            .flat_lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| match line {
                FlatLine::DiagramImage { id, row, cols, .. } => Some((i, *id, *row, *cols)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn diagram_image_takes_exactly_the_ascii_cells() {
        let block = mermaid("graph LR\n    A[Start] --> B[End]\n");
        let plain = PagerState::new(vec![block.clone()], 40, 80, 100, theme());
        let (mut state, ids) = diagram_pager(vec![block], 40, 80);
        // Same flat lines, rows swapped for image rows one for one.
        assert_eq!(plain.flat_lines.len(), state.flat_lines.len());
        let rows = diagram_image_rows(&state);
        let ascii: Vec<(usize, usize, usize)> = plain
            .flat_lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| match line {
                FlatLine::DiagramAscii { line, indent } => Some((i, line_width(line), *indent)),
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), ascii.len());
        let cols = ascii.iter().map(|a| a.1).max().unwrap();
        for (n, ((i, id, row, image_cols), (ascii_i, _, _))) in rows.iter().zip(&ascii).enumerate()
        {
            assert_eq!(i, ascii_i);
            assert_eq!(*id, ids[0]);
            assert_eq!(usize::from(*row), n);
            assert_eq!(usize::from(*image_cols), cols);
        }
        assert_eq!(
            state.take_graphics_output(),
            crate::kitty::place(
                ids[0],
                u16::try_from(cols).unwrap(),
                u16::try_from(ascii.len()).unwrap()
            )
        );
        // On screen: placeholder cells at the diagram's indent, in place of
        // the ASCII rows.
        let screen_rows = screen(&state, 80, 40);
        let plain_rows = screen(&plain, 80, 40);
        let first = rows[0].0;
        let indent = ascii[0].2;
        assert!(screen_rows[first][indent..].starts_with(crate::kitty::PLACEHOLDER));
        assert_eq!(
            screen_rows[first]
                .chars()
                .filter(|&c| c == crate::kitty::PLACEHOLDER)
                .count(),
            cols
        );
        let label_row = rows
            .iter()
            .map(|r| r.0)
            .find(|&i| plain_rows[i].contains("Start"));
        let label_row = label_row.expect("ASCII diagram shows its label");
        assert!(!screen_rows[label_row].contains("Start"));
        // The blank row above names the form of the active diagram; without
        // graphics it stays blank.
        assert_eq!(screen_rows[0].trim(), "[image — i for ASCII]");
        assert_eq!(plain_rows[0].trim(), "");
    }

    #[test]
    fn diagrams_stay_ascii_until_rasterized_and_without_graphics() {
        let block = mermaid("graph LR\n    A --> B\n");
        let plain = PagerState::new(vec![block.clone()], 40, 80, 100, theme());
        // Graphics on, but nothing rasterized yet (and no worker to do it).
        let store = crate::images::ImageStore::for_tests("/docs".into(), (10, 20));
        let state = PagerState::new(vec![block], 40, 80, 100, theme()).with_images(store);
        assert!(diagram_image_rows(&state).is_empty());
        assert_eq!(state.flat_lines, plain.flat_lines);
        assert!(diagram_image_rows(&plain).is_empty());
    }

    #[test]
    fn search_finds_text_in_diagram_image_rows() {
        let (mut state, _) = diagram_pager(
            vec![mermaid("graph LR\n    A[Start] --> B[Finish]\n")],
            40,
            80,
        );
        let rows = diagram_image_rows(&state);
        for c in "/Finish".chars() {
            state.handle_key_event(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        state.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(state.search_matches.len(), 1);
        assert!(rows.iter().any(|r| r.0 == state.search_matches[0]));
        // The match landed in the image, so the diagram switched to ASCII
        // to show it, in the same rows.
        assert!(diagram_image_rows(&state).is_empty());
        assert!(matches!(
            state.flat_lines[state.search_matches[0]],
            FlatLine::DiagramAscii { .. }
        ));
        assert_eq!(state.search_current, Some(0));
    }

    fn press(state: &mut PagerState, c: char) -> KeyAction {
        state.handle_key_event(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn i_switches_the_active_diagram_and_keeps_the_scroll() {
        let content = vec![
            para("intro"),
            mermaid("graph LR\n    A[Start] --> B[End]\n"),
            para("between"),
            mermaid("graph LR\n    C[Left] --> D[Right]\n"),
        ];
        let (mut state, ids) = diagram_pager(content, 40, 80);
        let image_rows = diagram_image_rows(&state);
        let original = state.flat_lines.clone();
        let before = original.len();
        state.scroll = 1;
        // Both diagrams are selectable; the first one in view is active.
        assert_eq!(state.interactive_blocks.len(), 2);
        state.update_active_from_viewport();
        assert_eq!(state.active, Some(0));

        assert!(matches!(press(&mut state, 'i'), KeyAction::Redraw));
        assert_eq!(state.scroll, 1);
        assert_eq!(state.flat_lines.len(), before);
        assert_eq!(state.toggled_diagrams, HashSet::from([1]));
        // Only the active diagram turned to ASCII, in the same rows.
        let rows = diagram_image_rows(&state);
        assert!(rows.iter().all(|r| r.1 == ids[1]));
        assert_eq!(
            rows,
            image_rows
                .iter()
                .copied()
                .filter(|r| r.1 == ids[1])
                .collect::<Vec<_>>()
        );
        let screen_rows = screen(&state, 80, 40);
        assert!(screen_rows.iter().any(|r| r.contains("Start")));
        assert!(!screen_rows.iter().any(|r| r.contains("Right")));
        assert!(
            screen_rows
                .iter()
                .any(|r| r.trim() == "[ASCII — i for image]")
        );

        // Again: back to the image, and the same flat lines as before.
        let toggled = state.flat_lines.clone();
        press(&mut state, 'i');
        assert!(state.toggled_diagrams.is_empty());
        assert_eq!(diagram_image_rows(&state), image_rows);
        assert_ne!(state.flat_lines, toggled);
        assert_eq!(state.flat_lines, original);
        assert_eq!(state.scroll, 1);
    }

    #[test]
    fn mermaid_graphics_off_starts_diagrams_as_ascii() {
        let (state, _) = diagram_pager(vec![mermaid("graph LR\n    A --> B\n")], 40, 80);
        let mut state = state.with_mermaid_graphics(crate::graphics::MermaidGraphics::Off);
        assert!(diagram_image_rows(&state).is_empty());
        assert_eq!(screen(&state, 80, 40)[0].trim(), "[ASCII — i for image]");
        // `i` still draws this one as an image.
        press(&mut state, 'i');
        assert_eq!(diagram_image_rows(&state).len(), state.flat_lines.len() - 2);
    }

    #[test]
    fn i_does_nothing_without_graphics() {
        let block = mermaid("graph TD\n    A --> B\n    B --> C\n");
        // Collapsed in a short pager, so the diagram is selectable.
        let mut state = PagerState::new(vec![block], 6, 80, 100, theme());
        state.active = Some(0);
        let before = state.flat_lines.clone();
        assert!(matches!(press(&mut state, 'i'), KeyAction::Nothing));
        assert!(state.toggled_diagrams.is_empty());
        assert_eq!(state.flat_lines, before);
        state.activate_current();
        assert!(matches!(press(&mut state, 'i'), KeyAction::Nothing));
        // No hint above it either.
        assert_eq!(screen(&state, 80, 6)[0].trim(), "");
        // Small diagrams stay unselectable, as before.
        let small = PagerState::new(
            vec![mermaid("graph LR\n    A --> B\n")],
            40,
            80,
            100,
            theme(),
        );
        assert!(small.interactive_blocks.is_empty());
    }

    #[test]
    fn collapsed_diagram_stays_collapsed_and_expands_in_the_chosen_form() {
        let block = mermaid("graph TD\n    A --> B\n    B --> C\n");
        let (mut state, _) = diagram_pager(vec![block], 6, 80);
        state.active = Some(0);
        let collapsed = state.flat_lines.clone();
        assert_eq!(screen(&state, 80, 6)[0].trim(), "[image — i for ASCII]");
        press(&mut state, 'i');
        assert_eq!(state.flat_lines, collapsed);
        assert_eq!(screen(&state, 80, 6)[0].trim(), "[ASCII — i for image]");
        state.activate_current();
        assert!(diagram_image_rows(&state).is_empty());
        assert!(
            state
                .flat_lines
                .iter()
                .any(|l| matches!(l, FlatLine::DiagramAscii { .. }))
        );
    }

    #[test]
    fn diagram_choice_survives_resize_and_reload() {
        let a = mermaid("graph LR\n    A[Start] --> B[End]\n");
        let b = mermaid("graph LR\n    C[Left] --> D[Right]\n");
        let (mut state, ids) = diagram_pager(vec![a.clone(), para("text"), b.clone()], 40, 80);
        state.active = Some(1);
        press(&mut state, 'i');
        let only_a = |state: &PagerState| {
            diagram_image_rows(state).iter().all(|r| r.1 == ids[0])
                && !diagram_image_rows(state).is_empty()
        };
        assert!(only_a(&state));

        state.resize(90, 30);
        assert!(only_a(&state));
        assert!(screen(&state, 90, 30).iter().any(|r| r.contains("Right")));

        // Watch-mode reload with the same blocks keeps the choice.
        state.replace_content(vec![a.clone(), para("edited"), b]);
        assert!(only_a(&state));
        assert_eq!(state.toggled_diagrams, HashSet::from([2]));
        assert!(screen(&state, 90, 30).iter().any(|r| r.contains("Right")));

        // A block that is no longer a diagram forgets it.
        state.replace_content(vec![a, para("text"), para("gone")]);
        assert!(state.toggled_diagrams.is_empty());
    }

    #[test]
    fn search_in_ascii_diagram_keeps_it_ascii_and_leaves_images_alone() {
        let content = vec![
            mermaid("graph LR\n    A[Start] --> B[End]\n"),
            mermaid("graph LR\n    C[Start] --> D[Right]\n"),
        ];
        let (mut state, ids) = diagram_pager(content, 40, 80);
        for c in "/Right".chars() {
            press(&mut state, c);
        }
        state.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Only the diagram the match landed in switched.
        assert_eq!(state.toggled_diagrams, HashSet::from([1]));
        assert!(diagram_image_rows(&state).iter().all(|r| r.1 == ids[0]));
        // `n` stays put in the now-ASCII diagram.
        press(&mut state, 'n');
        assert_eq!(state.toggled_diagrams, HashSet::from([1]));
    }

    #[test]
    fn large_diagram_images_collapse_and_expand_like_ascii() {
        // Taller than a 6-row pager: collapsed until activated.
        let block = mermaid("graph TD\n    A --> B\n    B --> C\n");
        let (mut state, _) = diagram_pager(vec![block], 6, 80);
        assert!(diagram_image_rows(&state).is_empty());
        assert!(
            state
                .flat_lines
                .iter()
                .any(|l| matches!(l, FlatLine::DiagramCollapsed { .. }))
        );
        state.active = Some(0);
        state.activate_current();
        let rows = diagram_image_rows(&state);
        assert!(!rows.is_empty());
        let entry = &state.interactive_blocks[0];
        assert_eq!(entry.flat_line_index, rows[0].0);
        assert_eq!(entry.flat_line_end, rows.last().unwrap().0 + 1);
    }

    #[test]
    fn reload_deletes_diagrams_that_are_gone() {
        let a = mermaid("graph LR\n    A --> B\n");
        let b = mermaid("graph LR\n    C --> D\n");
        let (mut state, ids) = diagram_pager(vec![a.clone(), b], 40, 80);
        state.take_graphics_output();
        state.replace_content(vec![a]);
        assert_eq!(state.take_graphics_output(), crate::kitty::delete(ids[1]));
        assert_eq!(diagram_image_rows(&state)[0].1, ids[0]);
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

#[cfg(test)]
mod model;
#[cfg(test)]
mod properties;
