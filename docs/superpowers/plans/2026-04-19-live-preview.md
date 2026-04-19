# Live Preview Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--watch` / `-W` flag that watches a file and re-renders in the pager with block-level diffing, mermaid caching, and viewport preservation.

**Architecture:** File watcher (notify crate, polling fallback) sends events via `mpsc::channel` to a non-blocking pager loop. On change, debounce 100ms, hash content to skip no-ops, diff old vs new AST blocks positionally, re-render only changed blocks, and update pager state preserving scroll position. Mermaid diagrams are cached so failed re-renders show the last good version.

**Tech Stack:** Rust, notify (file watching), crossterm (terminal events), ratatui (TUI rendering), pulldown-cmark (markdown parsing)

**Spec:** `docs/superpowers/specs/2026-04-19-live-preview-design.md`

---

## File Structure

### Files to Create
- `src/watch.rs` — Watch-mode event loop, file watcher setup, block diffing, mermaid cache, content hashing, status bar

### Files to Modify
- `Cargo.toml:6-7` — Add `notify` dependency
- `src/main.rs:1-6` — Add `mod watch`
- `src/main.rs:19-53` — Add `--watch` field to `Args`, add `watch: false` to test structs
- `src/main.rs:117-177` — Add validation, extract panic hook, dispatch to `watch::run_watch`
- `src/parser.rs:3-4,14-15` — Add `Eq` derive to `InlineElement` and `Block`
- `src/render.rs:57` — Add `PartialEq` derive to `RenderedBlock`
- `src/pager.rs:88,106-113,116-133,135,175,181,188,246` — Add `pub(crate)` visibility, add `draw_content` method
- `tests/integration.rs` — Add watch-mode CLI validation tests

---

### Task 1: Foundation Changes

**Files:**
- Modify: `src/parser.rs:3,14`
- Modify: `src/render.rs:57`
- Modify: `Cargo.toml:6-7`
- Modify: `src/pager.rs:88,106-113,116-133,135,175,181,188,246`
- Modify: `src/main.rs:1`

These are mechanical changes that unlock the rest of the implementation. No new behavior.

- [ ] **Step 1: Add `Eq` derive to parser types**

In `src/parser.rs`, `InlineElement` and `Block` already derive `PartialEq`. Add `Eq` for correctness (all fields are `Eq` types):

```rust
// Line 3 — change:
#[derive(Debug, Clone, PartialEq)]
// to:
#[derive(Debug, Clone, PartialEq, Eq)]
```

```rust
// Line 14 — change:
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
// to:
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
```

- [ ] **Step 2: Add `PartialEq` derive to `RenderedBlock`**

In `src/render.rs`, line 57:

```rust
// Change:
#[derive(Debug, Clone)]
pub enum RenderedBlock {
// to:
#[derive(Debug, Clone, PartialEq)]
pub enum RenderedBlock {
```

All variants contain types that implement `PartialEq` (`Vec<StyledLine>`, `Vec<String>`, `String`, `usize`), so this compiles.

- [ ] **Step 3: Add `notify` dependency**

In `Cargo.toml`, add after the `crossterm` line:

```toml
notify = "6"
```

- [ ] **Step 4: Make pager types and methods `pub(crate)`**

In `src/pager.rs`, apply visibility changes:

```rust
// Line 88 — change:
enum FlatLine {
// to:
pub(crate) enum FlatLine {
```

```rust
// Line 106 — change:
struct PagerState {
    content: Vec<RenderedBlock>,
    flat_lines: Vec<FlatLine>,
    scroll: usize,
    expanded: HashSet<usize>,
    terminal_height: u16,
    opener: Option<&'static str>,
    theme: &'static crate::theme::Theme,
}
// to:
pub(crate) struct PagerState {
    pub(crate) content: Vec<RenderedBlock>,
    pub(crate) flat_lines: Vec<FlatLine>,
    pub(crate) scroll: usize,
    pub(crate) expanded: HashSet<usize>,
    pub(crate) terminal_height: u16,
    opener: Option<&'static str>,
    theme: &'static crate::theme::Theme,
}
```

Make methods `pub(crate)` — change `fn` to `pub(crate) fn` on these methods in `impl PagerState`:

- `fn new(` → `pub(crate) fn new(` (line 117)
- `fn rebuild_flat_lines(` → `pub(crate) fn rebuild_flat_lines(` (line 135)
- `fn max_scroll(` → `pub(crate) fn max_scroll(` (line 175)
- `fn clamp_scroll(` → `pub(crate) fn clamp_scroll(` (line 181)
- `fn toggle_diagram_at_scroll(` → `pub(crate) fn toggle_diagram_at_scroll(` (line 188)
- `fn flat_line_to_ratatui(` → `pub(crate) fn flat_line_to_ratatui(` (line 246)

- [ ] **Step 5: Add `draw_content` method to `PagerState`**

In `src/pager.rs`, add this method inside `impl PagerState`, after `flat_line_to_ratatui` (after line 280):

```rust
    pub(crate) fn draw_content(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let height = area.height as usize;
        let lines: Vec<Line> = self
            .flat_lines
            .iter()
            .skip(self.scroll)
            .take(height)
            .map(|fl| self.flat_line_to_ratatui(fl))
            .collect();
        let paragraph = Paragraph::new(lines);
        f.render_widget(paragraph, area);
    }
```

Then update `run_pager`'s draw call (lines 297-309) to use the new method:

```rust
        terminal.draw(|f| {
            state.draw_content(f, f.area());
        })?;
```

- [ ] **Step 6: Add `mod watch` to `main.rs`**

In `src/main.rs`, add after line 1 (`mod highlight;`):

```rust
mod watch;
```

Create an empty `src/watch.rs`:

```rust
// Watch mode: live-preview with file watching and block-level diffing.
```

- [ ] **Step 7: Run tests to verify nothing broke**

Run: `cargo test`

Expected: All existing tests pass. No behavior changes.

- [ ] **Step 8: Commit**

```bash
git add src/parser.rs src/render.rs src/pager.rs src/main.rs src/watch.rs Cargo.toml Cargo.lock
git commit -m "refactor: prepare codebase for watch mode

Add Eq derives to parser types, PartialEq to RenderedBlock, pub(crate)
visibility to pager internals, draw_content method, notify dependency,
and empty watch module."
```

---

### Task 2: CLI --watch Flag with Validation

**Files:**
- Modify: `src/main.rs:19-53` (Args struct)
- Modify: `src/main.rs:117-177` (main function)
- Test: `src/main.rs` (unit tests)

- [ ] **Step 1: Write failing tests for --watch flag parsing**

In `src/main.rs`, add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn test_args_watch_flag() {
        let args = Args::parse_from(["mdx", "--watch", "file.md"]);
        assert!(args.watch);
    }

    #[test]
    fn test_args_watch_short_flag() {
        let args = Args::parse_from(["mdx", "-W", "file.md"]);
        assert!(args.watch);
    }

    #[test]
    fn test_args_watch_default_false() {
        let args = Args::parse_from(["mdx", "file.md"]);
        assert!(!args.watch);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_args_watch -- --nocapture`

Expected: Compilation error — `watch` field doesn't exist on `Args`.

- [ ] **Step 3: Add --watch field to Args and fix existing tests**

In `src/main.rs`, add to the `Args` struct after the `no_pager` field (after line 29):

```rust
    /// Watch file for changes and re-render live
    #[arg(short = 'W', long)]
    watch: bool,
```

Update existing test structs that construct `Args` directly. In `test_read_input_file` (line 205) and `test_read_input_no_file_no_stdin` (line 222), add `watch: false` to the Args literal:

```rust
        let args = Args {
            file: Some(path),
            pager: false,
            no_pager: false,
            watch: false,
            width: None,
            // ... rest unchanged
        };
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`

Expected: All tests pass, including the new watch flag tests.

- [ ] **Step 5: Write failing tests for validation**

Add to `src/main.rs` tests:

```rust
    #[test]
    fn test_watch_requires_file() {
        let args = Args {
            file: None,
            pager: false,
            no_pager: false,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a file"));
    }

    #[test]
    fn test_watch_conflicts_with_no_pager() {
        let args = Args {
            file: Some(PathBuf::from("test.md")),
            pager: false,
            no_pager: true,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("incompatible"));
    }

    #[test]
    fn test_watch_valid_args() {
        let args = Args {
            file: Some(PathBuf::from("test.md")),
            pager: false,
            no_pager: false,
            watch: true,
            width: None,
            theme: None,
            ui_theme: None,
            no_mermaid_rendering: false,
            split_mermaid_rendering: false,
        };
        let result = validate_watch_args(&args);
        assert!(result.is_ok());
    }
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cargo test test_watch_requires_file test_watch_conflicts test_watch_valid -- --nocapture`

Expected: Compilation error — `validate_watch_args` doesn't exist.

- [ ] **Step 7: Add validation function and wire into main**

In `src/main.rs`, add before `fn main()`:

```rust
fn validate_watch_args(args: &Args) -> Result<()> {
    if !args.watch {
        return Ok(());
    }
    if args.file.is_none() {
        anyhow::bail!("--watch requires a file argument (cannot watch stdin)");
    }
    if args.no_pager {
        anyhow::bail!("--watch and --no-pager are incompatible");
    }
    Ok(())
}

fn setup_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
        original_hook(info);
    }));
}
```

Update `main()` to validate and dispatch early for watch mode. Replace the current `main` function body (lines 117-177) with:

```rust
fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --theme=list before reading input
    if args.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }

    // Handle --ui-theme=list before reading input
    if args.ui_theme.as_deref() == Some("list") {
        for name in theme::Theme::available_names() {
            println!("{}", name);
        }
        return Ok(());
    }

    // Validate watch args early
    validate_watch_args(&args)?;

    let width = get_width(&args);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter =
        highlight::Highlighter::new(args.theme.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let ui_theme = match args.ui_theme.as_deref() {
        Some(name) => theme::Theme::by_name(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown UI theme '{}'. Available: {}",
                name,
                theme::Theme::available_names().join(", ")
            )
        })?,
        None => theme::Theme::default_theme(),
    };
    let mermaid_mode = if args.no_mermaid_rendering {
        render::MermaidMode::Raw
    } else if args.split_mermaid_rendering {
        render::MermaidMode::Split
    } else {
        render::MermaidMode::Render
    };

    // Watch mode — dispatch before reading input
    if args.watch {
        let path = args.file.as_ref().unwrap();
        setup_panic_hook();
        return watch::run_watch(path, width, &highlighter, ui_theme, mermaid_mode);
    }

    let input = read_input(&args)?;
    let blocks = parser::parse_markdown(&input);
    let rendered = render::render_blocks(&blocks, width, &highlighter, ui_theme, mermaid_mode);
    if use_pager(&args) {
        setup_panic_hook();
        pager::run_pager(rendered, ui_theme)?;
    } else {
        pipe_output(&rendered, no_color)?;
    }
    Ok(())
}
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test`

Expected: All tests pass. `watch::run_watch` doesn't exist yet so this won't compile — temporarily stub it in `src/watch.rs`:

```rust
use std::path::Path;
use anyhow::Result;
use crate::render::MermaidMode;

pub fn run_watch(
    _path: &Path,
    _width: u16,
    _highlighter: &crate::highlight::Highlighter,
    _theme: &'static crate::theme::Theme,
    _mermaid_mode: MermaidMode,
) -> Result<()> {
    anyhow::bail!("watch mode not yet implemented")
}
```

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/main.rs src/watch.rs
git commit -m "feat: add --watch CLI flag with validation

Adds --watch/-W flag that requires a file argument and conflicts with
--no-pager. Extracts panic hook setup into shared function. Watch mode
dispatches to watch::run_watch (stubbed for now)."
```

---

### Task 3: File Watcher + Content Hashing

**Files:**
- Modify: `src/watch.rs`

- [ ] **Step 1: Write failing tests for content_hash**

In `src/watch.rs`, replace the stub with:

```rust
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};

use crate::render::MermaidMode;

pub fn content_hash(content: &str) -> u64 {
    todo!()
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test watch::tests -- --nocapture`

Expected: Panics with `todo!()`.

- [ ] **Step 3: Implement content_hash**

Replace the `todo!()` in `content_hash`:

```rust
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test watch::tests -- --nocapture`

Expected: All 3 hash tests pass.

- [ ] **Step 5: Implement start_watcher and read_file_with_retry**

Add to `src/watch.rs`, before `run_watch`:

```rust
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
```

- [ ] **Step 6: Run compile check**

Run: `cargo check`

Expected: Compiles successfully. `start_watcher` and `read_file_with_retry` are unused (warning) but compile.

- [ ] **Step 7: Commit**

```bash
git add src/watch.rs Cargo.lock
git commit -m "feat: add file watcher with polling fallback and content hashing

Implements start_watcher (notify RecommendedWatcher with PollWatcher
fallback), content_hash (DefaultHasher), and read_file_with_retry
(single retry after 50ms for atomic saves)."
```

---

### Task 4: Block-Level Diffing + Mermaid Cache

**Files:**
- Modify: `src/watch.rs`

- [ ] **Step 1: Write failing tests for diff_and_render**

Add types and a `todo!()` stub for `diff_and_render`, then tests. In `src/watch.rs`, add after `read_file_with_retry`:

```rust
struct MermaidCacheEntry {
    lines: Vec<String>,
    node_count: usize,
    edge_count: usize,
}

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
    todo!()
}

fn flatten_groups(groups: &[Vec<crate::render::RenderedBlock>]) -> Vec<crate::render::RenderedBlock> {
    groups.iter().flat_map(|g| g.iter().cloned()).collect()
}
```

Add tests to the `#[cfg(test)] mod tests` block:

```rust
    use crate::parser::{Block, InlineElement};
    use crate::render::{self, RenderedBlock};

    fn test_highlighter() -> crate::highlight::Highlighter {
        crate::highlight::Highlighter::new(None).unwrap()
    }

    fn test_theme() -> &'static crate::theme::Theme {
        crate::theme::Theme::default_theme()
    }

    fn render_groups(blocks: &[Block], h: &crate::highlight::Highlighter, theme: &'static crate::theme::Theme) -> Vec<Vec<RenderedBlock>> {
        blocks
            .iter()
            .map(|b| render::render_blocks(std::slice::from_ref(b), 80, h, theme, MermaidMode::Render))
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

        let result = diff_and_render(&blocks, &groups, &blocks, 80, &h, theme, MermaidMode::Render, &mut cache);

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
            &old_blocks, &old_groups, &new_blocks, 80, &h, theme, MermaidMode::Render, &mut cache,
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
            &old_blocks, &old_groups, &new_blocks, 80, &h, theme, MermaidMode::Render, &mut cache,
        );

        assert_eq!(result.len(), 2);
        // First block unchanged
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
            &old_blocks, &old_groups, &new_blocks, 80, &h, theme, MermaidMode::Render, &mut cache,
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

        diff_and_render(&[], &old_groups, &blocks, 80, &h, theme, MermaidMode::Render, &mut cache);

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

        // Each block produces at least 1 RenderedBlock
        assert!(flat.len() >= 2);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test watch::tests::test_diff -- --nocapture`

Expected: Panics with `todo!()`.

- [ ] **Step 3: Implement diff_and_render**

Replace the `todo!()` body:

```rust
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
            let has_diagram = group.iter().any(|rb| matches!(rb, RenderedBlock::Diagram { .. }));

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test watch::tests -- --nocapture`

Expected: All diff tests and hash tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/watch.rs
git commit -m "feat: add block-level diffing with mermaid diagram cache

Positional diff reuses rendered blocks for unchanged AST blocks.
Mermaid cache stores last successful render; on parse failure, shows
cached diagram with error indicator instead of raw source."
```

---

### Task 5: Watch-Mode Event Loop + Status Bar

**Files:**
- Modify: `src/watch.rs` (replace `run_watch` stub)

This is the main integration task. The event loop handles keyboard/mouse input, file change events (via channel), debouncing, content hashing, diffing, and status bar rendering.

- [ ] **Step 1: Add imports and status bar types**

At the top of `src/watch.rs`, update the imports to the full set needed:

```rust
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::stdout;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
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

use crate::pager::PagerState;
use crate::render::{self, MermaidMode, RenderedBlock};
```

- [ ] **Step 2: Add StatusState struct**

Add after `flatten_groups`, before `run_watch`:

```rust
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
        if let Some(t) = self.message_time {
            if t.elapsed() >= Duration::from_secs(3) && self.message.is_some() {
                self.message = None;
                self.message_time = None;
                return true; // needs redraw
            }
        }
        false
    }
}
```

- [ ] **Step 3: Implement run_watch**

Replace the stub `run_watch` with the full implementation:

```rust
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
            render::render_blocks(std::slice::from_ref(b), width, highlighter, theme, mermaid_mode)
        })
        .collect();
    let flat_rendered = flatten_groups(&rendered_groups);
    let mut mermaid_cache: HashMap<String, MermaidCacheEntry> = HashMap::new();

    // Populate initial mermaid cache
    for (block, group) in blocks.iter().zip(rendered_groups.iter()) {
        if let crate::parser::Block::MermaidBlock { content } = block {
            for rb in group {
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
        }
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let term_height = terminal.size()?.height;
    let content_height = term_height.saturating_sub(1); // reserve 1 row for status bar
    let mut pager = PagerState::new(flat_rendered, content_height, theme);

    // Start file watcher
    let (_watcher, rx) = start_watcher(path)?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("?");
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
                    let page = (content_height as usize).saturating_sub(1).max(1);
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Down | KeyCode::Char('j') => {
                            let max = pager.max_scroll();
                            if pager.scroll < max {
                                pager.scroll += 1;
                                needs_redraw = true;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if pager.scroll > 0 {
                                pager.scroll = pager.scroll.saturating_sub(1);
                                needs_redraw = true;
                            }
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
                        KeyCode::Home | KeyCode::Char('g') => {
                            if pager.scroll != 0 {
                                pager.scroll = 0;
                                needs_redraw = true;
                            }
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

        // 4. Debounce check — act once 100ms has passed since last event
        if pending_change {
            if let Some(t) = last_event_time {
                if t.elapsed() >= Duration::from_millis(100) {
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

                            // Clear expanded set if block count changed
                            if new_blocks.len() != blocks.len() {
                                pager.expanded.clear();
                            }

                            // Update pager content and preserve scroll
                            pager.content = flat;
                            pager.rebuild_flat_lines();
                            pager.clamp_scroll();

                            // Update state
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
            }
        }

        // 5. Clear expired status messages
        if status.tick() {
            needs_redraw = true;
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
```

- [ ] **Step 4: Run compile check**

Run: `cargo check`

Expected: Compiles without errors. There may be unused import warnings for items only used in tests — suppress with `#[allow(unused_imports)]` on the test module if needed.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 6: Manual test**

Create a test file and run watch mode:

```bash
echo "# Hello\n\nSome text." > /tmp/test_watch.md
cargo run -- --watch /tmp/test_watch.md
```

In another terminal, edit the file:

```bash
echo "# Hello\n\nSome text.\n\n## Updated\n\nNew section." > /tmp/test_watch.md
```

Verify:
- The pager updates with the new content
- Status bar shows filename and "updated" flash
- Scroll position is preserved
- `q` exits cleanly

- [ ] **Step 7: Commit**

```bash
git add src/watch.rs
git commit -m "feat: implement watch-mode event loop with status bar

Non-blocking pager loop polls crossterm events (50ms timeout) and drains
file change notifications from notify watcher via mpsc channel. 100ms
debounce + content hash dedup prevents redundant re-renders. Status bar
shows filename, watch indicator, and flashes update/error messages."
```

---

### Task 6: Integration Tests

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration tests for --watch validation**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_watch_requires_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--watch")
        .output()
        .expect("failed to run mdx");
    assert!(
        !output.status.success(),
        "Should fail without file argument"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires a file"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn test_watch_conflicts_with_no_pager() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("watch_conflict.md");
    std::fs::write(&path, "# Test").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--watch")
        .arg("--no-pager")
        .arg(&path)
        .output()
        .expect("failed to run mdx");
    assert!(
        !output.status.success(),
        "Should fail with --no-pager"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("incompatible"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn test_watch_short_flag_accepted() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("watch_short.md");
    std::fs::write(&path, "# Test").unwrap();
    // Run with a timeout — watch mode will block, so we just verify it starts
    // by checking it doesn't immediately fail with a validation error
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("-W")
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start mdx");

    // Give it a moment to start, then kill it
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut child = child;
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();

    // If it had a validation error, stderr would contain it
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("requires a file"),
        "Should not have file error: {}",
        stderr
    );
    assert!(
        !stderr.contains("incompatible"),
        "Should not have conflict error: {}",
        stderr
    );
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test integration`

Expected: All integration tests pass (existing + new).

- [ ] **Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add integration tests for watch mode CLI validation

Tests --watch without file (error), --watch with --no-pager (error),
and -W short flag acceptance (starts without validation error)."
```

---

## Known Simplifications

These are acceptable trade-offs for the initial implementation:

1. **Scroll clamping instead of viewport anchoring** — When blocks above the viewport change height, the view may shift by a few lines. Full anchor-to-first-visible-block can be added later if this is noticeable in practice.

2. **Expanded set cleared on block count change** — If blocks are added/removed, diagram expand/collapse state resets. Remapping expanded indices based on diff results is possible but adds complexity.

3. **Keyboard handling duplication** — The watch loop duplicates the key handling from `run_pager`. Extracting a shared handler is possible but adds indirection for ~30 lines of straightforward match arms.
