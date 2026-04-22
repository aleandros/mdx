# Embed Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `mdx embed` subcommand that renders markdown into a bounded width/height ANSI stream, so other TUIs can shell out and drop the output into a widget.

**Architecture:** New `src/embed.rs` module with three pure functions (`flatten_blocks`, `truncate_line_width`, `run`) plus a `Commands::Embed(EmbedArgs)` clap variant in `src/main.rs`. Reuses existing parser, renderer, highlighter, and `styled_line_to_ansi`. Shared helpers (width resolution, theme/mermaid resolution, stdin reading) get lifted out of the default path so both code paths call the same code.

**Tech Stack:** Rust 2024, clap derive, anyhow, `unicode-width` (new dep).

**Spec:** `docs/superpowers/specs/2026-04-22-embed-mode-design.md`

---

## File Structure

- **Create:** `src/embed.rs` — embed module (`EmbedOptions` + `flatten_blocks` + `truncate_line_width` + `run`)
- **Modify:** `src/main.rs` — add module declaration, add `Commands::Embed(EmbedArgs)`, extract shared helpers, add dispatch branch
- **Modify:** `Cargo.toml` — add `unicode-width` dependency
- **Modify:** `tests/integration.rs` — add embed integration tests
- **Modify:** `README.md` — document the embed subcommand

---

## Task 1: Add `unicode-width` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add the dep**

Edit `Cargo.toml`, insert `unicode-width = "0.1"` alphabetically in `[dependencies]`:

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
crossterm = "0.28"
notify = "6"
pulldown-cmark = "0.12"
ratatui = "0.29"
syntect = { version = "5", default-features = false, features = ["parsing", "default-themes", "dump-load", "regex-fancy"] }
unicode-width = "0.1"
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: success, `Cargo.lock` updated with `unicode-width`.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add unicode-width dependency for embed mode"
```

---

## Task 2: Extract shared helpers in `src/main.rs`

Pure refactor so the new embed dispatch branch doesn't duplicate default-path logic. Behavior of the existing `mdx` command must not change.

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Run baseline tests**

Run: `cargo test`
Expected: all current tests pass. Record the count so you can confirm parity after the refactor.

- [ ] **Step 2: Replace `read_input` / `read_input_with_tty_check` with a field-based helper**

In `src/main.rs`, delete the two existing `read_input*` functions and replace with one helper keyed on the file path directly:

```rust
fn read_input_from(file: Option<&std::path::Path>, stdin_is_terminal: bool) -> Result<String> {
    match file {
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None if !stdin_is_terminal => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        None => anyhow::bail!("No input: provide a file argument or pipe markdown to stdin"),
    }
}
```

Update the call site in `main()` from `read_input(&args)?` to `read_input_from(args.file.as_deref(), std::io::stdin().is_terminal())?`.

Update the two existing unit tests (`test_read_input_file`, `test_read_input_no_file_no_stdin`) to call the new signature:

```rust
#[test]
fn test_read_input_from_file() {
    let dir = std::env::temp_dir().join("mdx_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.md");
    std::fs::write(&path, "# Hello").unwrap();
    let input = read_input_from(Some(&path), true).unwrap();
    assert_eq!(input, "# Hello");
}

#[test]
fn test_read_input_from_no_file_no_stdin_errors() {
    // TTY stdin with no file → must error
    assert!(read_input_from(None, true).is_err());
}
```

- [ ] **Step 3: Extract width resolution**

Replace `fn get_width(args: &Args) -> u16` with a field-based version and update the call site:

```rust
fn resolve_width(override_width: Option<u16>) -> u16 {
    if let Some(w) = override_width {
        return w;
    }
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}
```

Call site: `let width = resolve_width(args.width);`

- [ ] **Step 4: Extract mermaid mode + UI theme resolution into helpers**

Add these helpers in `src/main.rs`:

```rust
fn resolve_mermaid_mode(no_mermaid: bool, split_mermaid: bool) -> render::MermaidMode {
    if no_mermaid {
        render::MermaidMode::Raw
    } else if split_mermaid {
        render::MermaidMode::Split
    } else {
        render::MermaidMode::Render
    }
}

fn resolve_ui_theme(name: Option<&str>) -> Result<&'static theme::Theme> {
    match name {
        Some(n) => theme::Theme::by_name(n).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown UI theme '{}'. Available: {}",
                n,
                theme::Theme::available_names().join(", ")
            )
        }),
        None => Ok(theme::Theme::default_theme()),
    }
}
```

Replace the inline blocks in `main()` that compute `mermaid_mode` and `ui_theme` with calls to these helpers:

```rust
let ui_theme = resolve_ui_theme(args.ui_theme.as_deref())?;
let mermaid_mode = resolve_mermaid_mode(args.no_mermaid_rendering, args.split_mermaid_rendering);
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: same tests pass as in Step 1, including the renamed `test_read_input_from_*` tests.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "refactor: extract field-based helpers in main for embed reuse"
```

---

## Task 3: Scaffold `src/embed.rs` with `flatten_blocks` (TDD)

**Files:**
- Create: `src/embed.rs`
- Modify: `src/main.rs` (add `mod embed;`)
- Test: unit tests inside `src/embed.rs`

- [ ] **Step 1: Register the module**

In `src/main.rs`, add `mod embed;` alongside the other `mod` lines at the top:

```rust
mod embed;
mod highlight;
mod mermaid;
mod pager;
mod parser;
mod render;
mod self_update;
mod theme;
mod watch;
```

- [ ] **Step 2: Create `src/embed.rs` with failing tests**

```rust
use crate::render::{RenderedBlock, StyledLine, StyledSpan};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{StyledLine, StyledSpan};

    fn line_text(line: &StyledLine) -> String {
        line.spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn flatten_lines_preserves_order_and_styles() {
        let block = RenderedBlock::Lines(vec![
            StyledLine {
                spans: vec![StyledSpan::plain("first")],
            },
            StyledLine {
                spans: vec![StyledSpan::plain("second")],
            },
        ]);
        let out = flatten_blocks(&[block]);
        assert_eq!(out.len(), 2);
        assert_eq!(line_text(&out[0]), "first");
        assert_eq!(line_text(&out[1]), "second");
    }

    #[test]
    fn flatten_diagram_appends_trailing_blank() {
        let block = RenderedBlock::Diagram {
            lines: vec![
                StyledLine { spans: vec![StyledSpan::plain("aaa")] },
                StyledLine { spans: vec![StyledSpan::plain("bbb")] },
            ],
            node_count: 2,
            edge_count: 1,
        };
        let out = flatten_blocks(&[block]);
        assert_eq!(out.len(), 3, "diagram lines + one trailing blank");
        assert_eq!(line_text(&out[0]), "aaa");
        assert_eq!(line_text(&out[1]), "bbb");
        assert!(out[2].spans.is_empty(), "trailing blank is empty");
    }

    #[test]
    fn flatten_image_with_alt() {
        let block = RenderedBlock::Image {
            alt: "diagram".to_string(),
            url: "x.png".to_string(),
        };
        let out = flatten_blocks(&[block]);
        assert_eq!(out.len(), 1);
        assert_eq!(line_text(&out[0]), "[Image: diagram](x.png)");
    }

    #[test]
    fn flatten_image_without_alt() {
        let block = RenderedBlock::Image {
            alt: String::new(),
            url: "x.png".to_string(),
        };
        let out = flatten_blocks(&[block]);
        assert_eq!(line_text(&out[0]), "[Image](x.png)");
    }

    #[test]
    fn flatten_mixed_sequence_preserves_order() {
        let blocks = vec![
            RenderedBlock::Lines(vec![StyledLine {
                spans: vec![StyledSpan::plain("para")],
            }]),
            RenderedBlock::Image {
                alt: "a".to_string(),
                url: "b".to_string(),
            },
            RenderedBlock::Diagram {
                lines: vec![StyledLine {
                    spans: vec![StyledSpan::plain("D1")],
                }],
                node_count: 1,
                edge_count: 0,
            },
        ];
        let out = flatten_blocks(&blocks);
        assert_eq!(out.len(), 4);
        assert_eq!(line_text(&out[0]), "para");
        assert_eq!(line_text(&out[1]), "[Image: a](b)");
        assert_eq!(line_text(&out[2]), "D1");
        assert!(out[3].spans.is_empty());
    }
}
```

- [ ] **Step 3: Run tests to confirm they fail**

Run: `cargo test --lib embed::`
Expected: compile error — `flatten_blocks` is not defined.

- [ ] **Step 4: Implement `flatten_blocks`**

At the top of `src/embed.rs` (above the `#[cfg(test)]` block), add:

```rust
pub fn flatten_blocks(blocks: &[RenderedBlock]) -> Vec<StyledLine> {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            RenderedBlock::Lines(lines) => {
                for line in lines {
                    out.push(line.clone());
                }
            }
            RenderedBlock::Diagram { lines, .. } => {
                for line in lines {
                    out.push(line.clone());
                }
                out.push(StyledLine::empty());
            }
            RenderedBlock::Image { alt, url } => {
                let text = if alt.is_empty() {
                    format!("[Image]({})", url)
                } else {
                    format!("[Image: {}]({})", alt, url)
                };
                out.push(StyledLine {
                    spans: vec![StyledSpan::plain(text)],
                });
            }
        }
    }
    out
}
```

- [ ] **Step 5: Run tests to confirm they pass**

Run: `cargo test --lib embed::`
Expected: all five tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/embed.rs src/main.rs
git commit -m "feat(embed): add flatten_blocks helper"
```

---

## Task 4: Add `truncate_line_width` (TDD)

**Files:**
- Modify: `src/embed.rs`

- [ ] **Step 1: Add failing tests**

Append these tests inside the existing `#[cfg(test)] mod tests` block in `src/embed.rs`:

```rust
#[test]
fn truncate_ascii_exact_boundary_unchanged() {
    let line = StyledLine {
        spans: vec![StyledSpan::plain("hello")],
    };
    let out = truncate_line_width(&line, 5);
    assert_eq!(line_text(&out), "hello");
}

#[test]
fn truncate_ascii_below_boundary_cuts_mid_span() {
    let line = StyledLine {
        spans: vec![
            StyledSpan::plain("ab"),
            StyledSpan {
                text: "cdef".to_string(),
                style: crate::render::SpanStyle {
                    bold: true,
                    ..Default::default()
                },
            },
        ],
    };
    let out = truncate_line_width(&line, 4);
    assert_eq!(line_text(&out), "abcd");
    // Style of the cut span is preserved on the surviving prefix.
    assert!(out.spans[1].style.bold);
}

#[test]
fn truncate_cjk_before_wide_char_on_boundary() {
    // "a漢b" has display width 1+2+1=4. Truncating to 2 must cut BEFORE "漢"
    // (cutting through a double-width glyph is not allowed), yielding "a"
    // with display width 1.
    let line = StyledLine {
        spans: vec![StyledSpan::plain("a漢b")],
    };
    let out = truncate_line_width(&line, 2);
    assert_eq!(line_text(&out), "a");
}

#[test]
fn truncate_empty_line_stays_empty() {
    let out = truncate_line_width(&StyledLine::empty(), 10);
    assert!(out.spans.is_empty());
}

#[test]
fn truncate_zero_width_produces_empty_line() {
    let line = StyledLine {
        spans: vec![StyledSpan::plain("anything")],
    };
    let out = truncate_line_width(&line, 0);
    assert!(out.spans.is_empty());
}

#[test]
fn truncate_preserves_line_already_shorter() {
    let line = StyledLine {
        spans: vec![StyledSpan::plain("hi")],
    };
    let out = truncate_line_width(&line, 100);
    assert_eq!(line_text(&out), "hi");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib embed::`
Expected: compile error — `truncate_line_width` is not defined.

- [ ] **Step 3: Implement `truncate_line_width`**

At the top of `src/embed.rs`, add the `use` for unicode-width and the function:

```rust
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
```

Add above the `#[cfg(test)]` block:

```rust
pub fn truncate_line_width(line: &StyledLine, max_cols: usize) -> StyledLine {
    if max_cols == 0 {
        return StyledLine::empty();
    }

    let mut acc_cols: usize = 0;
    let mut out_spans: Vec<StyledSpan> = Vec::new();

    for span in &line.spans {
        let span_w = UnicodeWidthStr::width(span.text.as_str());
        if acc_cols + span_w <= max_cols {
            out_spans.push(span.clone());
            acc_cols += span_w;
            if acc_cols == max_cols {
                break;
            }
            continue;
        }

        // Cut inside this span. Walk chars until the next one would overflow.
        let remaining = max_cols - acc_cols;
        let mut taken_cols = 0usize;
        let mut take_bytes = 0usize;
        for (byte_idx, ch) in span.text.char_indices() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if taken_cols + cw > remaining {
                break;
            }
            taken_cols += cw;
            take_bytes = byte_idx + ch.len_utf8();
        }
        if take_bytes > 0 {
            out_spans.push(StyledSpan {
                text: span.text[..take_bytes].to_string(),
                style: span.style.clone(),
            });
        }
        break;
    }

    StyledLine { spans: out_spans }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test --lib embed::`
Expected: all eleven `embed::` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/embed.rs
git commit -m "feat(embed): add unicode-aware truncate_line_width"
```

---

## Task 5: Implement `embed::run` (TDD)

`run` takes a writer so we can unit-test the full pipeline without touching stdout.

**Files:**
- Modify: `src/embed.rs`

- [ ] **Step 1: Add failing tests**

Append to the `tests` module in `src/embed.rs`:

```rust
#[test]
fn run_emits_only_height_lines_when_capped() {
    let input = "# A\n\nparagraph one\n\nparagraph two\n";
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let theme = crate::theme::Theme::default_theme();
    let opts = EmbedOptions {
        width: 80,
        height: Some(2),
        no_color: true,
    };
    let mut buf: Vec<u8> = Vec::new();
    run(
        input,
        opts,
        &highlighter,
        theme,
        crate::render::MermaidMode::Render,
        &mut buf,
    )
    .unwrap();
    let s = String::from_utf8(buf).unwrap();
    let line_count = s.matches('\n').count();
    assert_eq!(line_count, 2, "exactly 2 newlines emitted, got: {:?}", s);
}

#[test]
fn run_crops_each_line_to_width() {
    let input = "a very long paragraph that will be wrapped or truncated depending on width";
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let theme = crate::theme::Theme::default_theme();
    let opts = EmbedOptions {
        width: 10,
        height: None,
        no_color: true,
    };
    let mut buf: Vec<u8> = Vec::new();
    run(
        input,
        opts,
        &highlighter,
        theme,
        crate::render::MermaidMode::Render,
        &mut buf,
    )
    .unwrap();
    let s = String::from_utf8(buf).unwrap();
    for line in s.lines() {
        assert!(
            UnicodeWidthStr::width(line) <= 10,
            "line exceeded width 10: {:?}",
            line
        );
    }
}

#[test]
fn run_respects_no_color_flag() {
    let input = "# Heading\n\nbody text\n";
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let theme = crate::theme::Theme::default_theme();
    let opts = EmbedOptions {
        width: 80,
        height: None,
        no_color: true,
    };
    let mut buf: Vec<u8> = Vec::new();
    run(
        input,
        opts,
        &highlighter,
        theme,
        crate::render::MermaidMode::Render,
        &mut buf,
    )
    .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(!s.contains('\x1b'), "no-color output must not contain ESC");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib embed::`
Expected: compile errors — `EmbedOptions` and `run` are not defined.

- [ ] **Step 3: Implement `EmbedOptions` and `run`**

Add to the top of `src/embed.rs` (below the `use` lines):

```rust
pub struct EmbedOptions {
    pub width: u16,
    pub height: Option<usize>,
    pub no_color: bool,
}

pub fn run<W: std::io::Write>(
    input: &str,
    opts: EmbedOptions,
    highlighter: &crate::highlight::Highlighter,
    ui_theme: &crate::theme::Theme,
    mermaid_mode: crate::render::MermaidMode,
    writer: &mut W,
) -> anyhow::Result<()> {
    let blocks = crate::parser::parse_markdown(input);
    let rendered =
        crate::render::render_blocks(&blocks, opts.width, highlighter, ui_theme, mermaid_mode);
    let lines = flatten_blocks(&rendered);
    let capped: Box<dyn Iterator<Item = StyledLine>> = match opts.height {
        Some(h) => Box::new(lines.into_iter().take(h)),
        None => Box::new(lines.into_iter()),
    };
    let max_cols = opts.width as usize;
    for line in capped {
        let cut = truncate_line_width(&line, max_cols);
        writeln!(
            writer,
            "{}",
            crate::render::styled_line_to_ansi(&cut, opts.no_color)
        )?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cargo test --lib embed::`
Expected: all fourteen `embed::` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/embed.rs
git commit -m "feat(embed): add run pipeline with width/height truncation"
```

---

## Task 6: Wire `Commands::Embed` into clap (TDD)

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add failing CLI parse tests**

Append to the existing `#[cfg(test)] mod tests` block in `src/main.rs`:

```rust
#[test]
fn test_embed_subcommand_basic() {
    let cli = Cli::parse_from(["mdx", "embed", "file.md"]);
    match cli.command {
        Some(Commands::Embed(args)) => {
            assert_eq!(args.file, Some(PathBuf::from("file.md")));
            assert_eq!(args.width, None);
            assert_eq!(args.height, None);
        }
        _ => panic!("expected Embed subcommand"),
    }
}

#[test]
fn test_embed_subcommand_width_height() {
    let cli = Cli::parse_from(["mdx", "embed", "--width", "40", "--height", "10", "file.md"]);
    match cli.command {
        Some(Commands::Embed(args)) => {
            assert_eq!(args.width, Some(40));
            assert_eq!(args.height, Some(10));
        }
        _ => panic!("expected Embed subcommand"),
    }
}

#[test]
fn test_embed_subcommand_rejects_pager_flag() {
    let result = Cli::try_parse_from(["mdx", "embed", "--pager", "file.md"]);
    assert!(result.is_err(), "embed must not accept --pager");
}

#[test]
fn test_embed_subcommand_rejects_watch_flag() {
    let result = Cli::try_parse_from(["mdx", "embed", "--watch", "file.md"]);
    assert!(result.is_err(), "embed must not accept --watch");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --bin mdx test_embed_subcommand`
Expected: compile errors — `Commands::Embed` and `EmbedArgs` do not exist.

- [ ] **Step 3: Add `EmbedArgs` and `Commands::Embed` variant**

In `src/main.rs`, extend the `Commands` enum:

```rust
#[derive(clap::Subcommand)]
enum Commands {
    /// Update mdx to the latest version
    Update,
    /// Render markdown into a bounded ANSI stream for embedding in other TUIs
    Embed(EmbedArgs),
}
```

Add the `EmbedArgs` struct right after the existing `Args` struct:

```rust
#[derive(clap::Args)]
struct EmbedArgs {
    /// Markdown file to render (stdin if omitted)
    file: Option<PathBuf>,

    /// Output width in columns; each line is cropped to fit
    #[arg(short, long)]
    width: Option<u16>,

    /// Maximum number of output lines
    #[arg(long)]
    height: Option<usize>,

    /// Syntax highlighting theme (use `list` to see options)
    #[arg(long)]
    theme: Option<String>,

    /// UI color theme (use `list` to see options)
    #[arg(long)]
    ui_theme: Option<String>,

    /// Show raw mermaid source without rendering
    #[arg(long)]
    no_mermaid_rendering: bool,

    /// Show mermaid source followed by rendered diagram
    #[arg(long)]
    split_mermaid_rendering: bool,
}
```

- [ ] **Step 4: Add dispatch branch in `main()`**

In `main()`, *before* the existing `if let Some(Commands::Update)` branch, match on `Commands::Embed` too. Replace the `if let Some(Commands::Update) = cli.command` section with a full match:

```rust
    match cli.command {
        Some(Commands::Update) => return self_update::run(),
        Some(Commands::Embed(eargs)) => return run_embed(eargs),
        None => {}
    }
```

Add the `run_embed` helper at the end of the file (above `#[cfg(test)]`):

```rust
fn run_embed(eargs: EmbedArgs) -> Result<()> {
    // theme=list / ui-theme=list short-circuits
    if eargs.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }
    if eargs.ui_theme.as_deref() == Some("list") {
        for name in theme::Theme::available_names() {
            println!("{}", name);
        }
        return Ok(());
    }

    let width = resolve_width(eargs.width);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter =
        highlight::Highlighter::new(eargs.theme.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let ui_theme = resolve_ui_theme(eargs.ui_theme.as_deref())?;
    let mermaid_mode =
        resolve_mermaid_mode(eargs.no_mermaid_rendering, eargs.split_mermaid_rendering);

    let input = read_input_from(eargs.file.as_deref(), std::io::stdin().is_terminal())?;
    let opts = embed::EmbedOptions {
        width,
        height: eargs.height,
        no_color,
    };
    let mut stdout = std::io::stdout().lock();
    embed::run(&input, opts, &highlighter, ui_theme, mermaid_mode, &mut stdout)
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all existing tests plus the four new embed CLI tests pass.

- [ ] **Step 6: Smoke test the binary**

Run: `echo '# Hi\n\npara' | cargo run --quiet -- embed --width 20 --height 3`
Expected: at most three lines of output, the first is a colored header.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat(embed): add mdx embed subcommand"
```

---

## Task 7: Integration tests for `mdx embed`

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: Append integration tests**

Add at the bottom of `tests/integration.rs`:

```rust
// ─── mdx embed subcommand ─────────────────────────────────────────────────

#[test]
fn embed_honors_height_cap() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_height.md");
    std::fs::write(
        &path,
        "# One\n\nPara one.\n\n# Two\n\nPara two.\n\n# Three\n\nPara three.\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40", "--height", "4"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success(), "mdx embed should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line_count = stdout.matches('\n').count();
    assert_eq!(line_count, 4, "output was: {:?}", stdout);
}

#[test]
fn embed_honors_width_cap() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_width.md");
    std::fs::write(
        &path,
        "# Heading\n\nA fairly long paragraph that should wrap at narrow widths in normal rendering.\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "20"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        assert!(
            line.chars().count() <= 20,
            "line exceeded width 20 (chars): {:?}",
            line
        );
    }
}

#[test]
fn embed_no_color_env_strips_escape_codes() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_nocolor.md");
    std::fs::write(&path, "# Heading\n\nBody.\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('\x1b'),
        "no ESC bytes allowed with NO_COLOR: {:?}",
        stdout
    );
}

#[test]
fn embed_emits_color_by_default() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_color.md");
    std::fs::write(&path, "# Heading\n\nBody.\n").unwrap();
    // Pipe stdout (not a TTY) — embed must still emit color.
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "40"])
        .arg(&path)
        .env_remove("NO_COLOR")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains('\x1b'),
        "embed must emit ANSI even when stdout is a pipe"
    );
}

#[test]
fn embed_rejects_pager_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--pager", "/tmp/whatever.md"])
        .output()
        .expect("failed to run mdx embed");
    assert!(
        !output.status.success(),
        "embed must reject --pager, got: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn embed_theme_list_prints_and_exits() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--theme", "list"])
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "theme list should print names");
}

#[test]
fn embed_diagram_crops_without_exceeding_width() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("embed_diagram.md");
    std::fs::write(
        &path,
        "```mermaid\ngraph LR\n    A[First node with a long label] --> B[Second node with a long label] --> C[Third node with a long label]\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .args(["embed", "--width", "30"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx embed");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        assert!(
            line.chars().count() <= 30,
            "diagram line exceeded width 30: {:?}",
            line
        );
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integration`
Expected: all seven new `embed_*` tests pass, plus the existing integration tests.

- [ ] **Step 3: Commit**

```bash
git add tests/integration.rs
git commit -m "test(embed): add integration tests for mdx embed"
```

---

## Task 8: Document in README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add an Embed section**

Insert a new section between the `## Usage` block (after the initial flags) and `### Interactive Pager`. Use this exact content:

```markdown
### Embedding in other programs

Use `mdx embed` when another program needs the rendered output as a bounded ANSI stream. It never opens a pager, never touches the alternate screen, and always emits ANSI color (respect `NO_COLOR` to disable).

```bash
# Render into a 40-column, 10-row box (caller handles scrolling)
mdx embed --width 40 --height 10 README.md

# From stdin
cat README.md | mdx embed --width 60

# Drop color for plain output
NO_COLOR=1 mdx embed --width 40 README.md
```

Output contract:

- Every line ends with `\n`; each line's display width ≤ `--width`.
- Total lines ≤ `--height` when provided.
- No pager, no alt-screen, no terminal escape sequences other than SGR color/style.
- Mermaid diagrams wider than `--width` are cropped (not reflowed).
```

- [ ] **Step 2: Verify markdown renders correctly through mdx itself**

Run: `cargo run --quiet -- embed --width 80 --height 20 README.md`
Expected: output is clean, no truncation artifacts, embed section visible.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document mdx embed subcommand"
```

---

## Final verification

- [ ] **Run full test suite**

Run: `cargo test`
Expected: all tests green, no clippy warnings, no `cargo fmt --check` diffs.

- [ ] **Run clippy explicitly**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Smoke test end-to-end**

Run: `cargo run --quiet -- embed --width 50 --height 10 README.md | head -20`
Expected: colored output, every line ≤ 50 cols, exactly 10 newlines.
