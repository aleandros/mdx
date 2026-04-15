# Syntax Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add syntax highlighting to fenced code blocks using syntect, with ANSI color mapping by default and a `--theme` flag for named themes.

**Architecture:** New `src/highlight.rs` module wraps syntect behind a `Highlighter` struct. The renderer calls into it for code blocks. Default mode maps syntect RGB colors to the 16 ANSI colors; `--theme` uses RGB directly via a new `Color::Rgb` variant.

**Tech Stack:** syntect 5 (with `regex-fancy` pure-Rust backend), existing render/pager infrastructure.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `Cargo.toml` | Modify | Add syntect dependency |
| `src/highlight.rs` | Create | Syntect wrapper: Highlighter struct, highlight_code, ANSI color mapping |
| `src/render.rs` | Modify | Add `Color::Rgb`, update `render_code_block_lines` to use Highlighter, update `render_blocks` signature |
| `src/pager.rs` | Modify | Handle `Color::Rgb` in `color_to_ratatui` |
| `src/main.rs` | Modify | Add `--theme` flag, construct Highlighter, pass to render pipeline, declare `mod highlight` |
| `tests/integration.rs` | Modify | Add syntax highlighting integration tests |

---

### Task 1: Add `Color::Rgb` variant and update output functions

**Files:**
- Modify: `src/render.rs:6-85` (Color enum, color_ansi_code)
- Modify: `src/pager.rs:43-56` (color_to_ratatui)

This is foundational — highlight.rs will produce Rgb colors, so the rendering pipeline must handle them first.

- [ ] **Step 1: Write failing test for Rgb ANSI output**

Add to the `#[cfg(test)] mod tests` block in `src/render.rs`:

```rust
#[test]
fn test_ansi_output_rgb_color() {
    let line = StyledLine {
        spans: vec![StyledSpan {
            text: "colored".to_string(),
            style: SpanStyle {
                fg: Some(Color::Rgb(255, 100, 50)),
                ..Default::default()
            },
        }],
    };
    let output = styled_line_to_ansi(&line, false);
    assert!(output.contains("\x1b[38;2;255;100;50m"), "Should use 24-bit color escape: {}", output);
    assert!(output.contains("colored"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_ansi_output_rgb_color -- --nocapture 2>&1`
Expected: compilation error — `Color::Rgb` doesn't exist yet.

- [ ] **Step 3: Add Rgb variant to Color enum**

In `src/render.rs`, add the variant to the `Color` enum (after `DarkGray`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightYellow,
    BrightCyan,
    BrightMagenta,
    DarkGray,
    Rgb(u8, u8, u8),
}
```

Update `color_ansi_code` — since Rgb needs a dynamic string (not `&'static str`), change the return type. Replace the function with one that writes directly into the codes vec. Alternatively, handle Rgb separately in `styled_line_to_ansi`. The cleaner approach: handle Rgb as a special case in `styled_line_to_ansi`.

Replace `styled_line_to_ansi` in `src/render.rs`:

```rust
pub fn styled_line_to_ansi(line: &StyledLine, no_color: bool) -> String {
    if no_color {
        return line.spans.iter().map(|s| s.text.as_str()).collect();
    }

    let mut result = String::new();
    for span in &line.spans {
        let style = &span.style;
        let mut codes: Vec<String> = Vec::new();

        if style.bold {
            codes.push("1".to_string());
        }
        if style.italic {
            codes.push("3".to_string());
        }
        if style.dim {
            codes.push("2".to_string());
        }
        if let Some(ref color) = style.fg {
            match color {
                Color::Rgb(r, g, b) => codes.push(format!("38;2;{};{};{}", r, g, b)),
                other => codes.push(color_ansi_code(other).to_string()),
            }
        }

        if codes.is_empty() {
            result.push_str(&span.text);
        } else {
            let code_str = codes.join(";");
            result.push_str(&format!("\x1b[{}m{}\x1b[0m", code_str, span.text));
        }
    }
    result
}
```

Update `color_to_ratatui` in `src/pager.rs` — add the Rgb arm:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_ansi_output_rgb_color -- --nocapture 2>&1`
Expected: PASS

- [ ] **Step 5: Run full test suite to check for regressions**

Run: `cargo test 2>&1`
Expected: all existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/render.rs src/pager.rs
git commit -m "feat: add Color::Rgb variant for 24-bit color support"
```

---

### Task 2: Create `highlight.rs` with ANSI color mapping

**Files:**
- Create: `src/highlight.rs`
- Modify: `src/main.rs:1` (add `mod highlight;`)

- [ ] **Step 1: Write failing test for ANSI color mapping**

Create `src/highlight.rs` with only the test and minimal type stubs:

```rust
use crate::render::Color;

/// Maps an RGB color to the nearest ANSI Color using Euclidean distance in RGB space.
fn rgb_to_ansi_color(_r: u8, _g: u8, _b: u8) -> Color {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_ansi_pure_red() {
        let color = rgb_to_ansi_color(255, 0, 0);
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn test_rgb_to_ansi_pure_green() {
        let color = rgb_to_ansi_color(0, 255, 0);
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn test_rgb_to_ansi_pure_blue() {
        let color = rgb_to_ansi_color(0, 0, 255);
        assert_eq!(color, Color::Blue);
    }

    #[test]
    fn test_rgb_to_ansi_white() {
        let color = rgb_to_ansi_color(255, 255, 255);
        assert_eq!(color, Color::White);
    }

    #[test]
    fn test_rgb_to_ansi_dark_gray() {
        let color = rgb_to_ansi_color(100, 100, 100);
        assert_eq!(color, Color::DarkGray);
    }
}
```

Add `mod highlight;` to `src/main.rs` (after `mod render;`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test rgb_to_ansi 2>&1`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement rgb_to_ansi_color**

Replace the `todo!()` stub in `src/highlight.rs`:

```rust
use crate::render::Color;

/// Reference RGB values for the 16 standard ANSI colors.
const ANSI_COLORS: &[(Color, u8, u8, u8)] = &[
    (Color::Red, 205, 0, 0),
    (Color::Green, 0, 205, 0),
    (Color::Yellow, 205, 205, 0),
    (Color::Blue, 0, 0, 238),
    (Color::Magenta, 205, 0, 205),
    (Color::Cyan, 0, 205, 205),
    (Color::White, 229, 229, 229),
    (Color::BrightYellow, 255, 255, 85),
    (Color::BrightCyan, 85, 255, 255),
    (Color::BrightMagenta, 255, 85, 255),
    (Color::DarkGray, 127, 127, 127),
];

/// Maps an RGB color to the nearest ANSI Color using Euclidean distance in RGB space.
fn rgb_to_ansi_color(r: u8, g: u8, b: u8) -> Color {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    ANSI_COLORS
        .iter()
        .map(|(color, cr, cg, cb)| {
            let dr = r - *cr as i32;
            let dg = g - *cg as i32;
            let db = b - *cb as i32;
            let dist = dr * dr + dg * dg + db * db;
            (dist, color)
        })
        .min_by_key(|(dist, _)| *dist)
        .unwrap()
        .1
        .clone()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test rgb_to_ansi 2>&1`
Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/highlight.rs src/main.rs
git commit -m "feat: add highlight module with ANSI color mapping"
```

---

### Task 3: Implement Highlighter struct and `highlight_code`

**Files:**
- Modify: `Cargo.toml` (add syntect)
- Modify: `src/highlight.rs`

- [ ] **Step 1: Add syntect dependency**

Add to `[dependencies]` in `Cargo.toml`:

```toml
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "regex-fancy"] }
```

- [ ] **Step 2: Write failing tests for Highlighter**

Add to the test module in `src/highlight.rs`:

```rust
#[test]
fn test_highlighter_new_default() {
    let h = Highlighter::new(None);
    // Should not panic, should construct successfully
    assert!(h.theme_name.is_none());
}

#[test]
fn test_highlight_rust_code() {
    let h = Highlighter::new(None);
    let code = "fn main() {\n    println!(\"hello\");\n}\n";
    let result = h.highlight_code(code, Some("rust"));
    assert!(result.is_some(), "Rust should be a recognized language");
    let lines = result.unwrap();
    assert_eq!(lines.len(), 3, "Should have 3 lines of code");
    // At least some spans should have color (not all plain)
    let has_color = lines.iter().any(|line| {
        line.iter().any(|span| span.style.fg.is_some())
    });
    assert!(has_color, "Highlighted code should have colored spans");
}

#[test]
fn test_highlight_unknown_language_returns_none() {
    let h = Highlighter::new(None);
    let result = h.highlight_code("some text", Some("not_a_real_language_xyz"));
    assert!(result.is_none(), "Unknown language should return None");
}

#[test]
fn test_highlight_no_language_returns_none() {
    let h = Highlighter::new(None);
    let result = h.highlight_code("some text", None);
    assert!(result.is_none(), "No language should return None");
}

#[test]
fn test_highlight_with_named_theme() {
    let h = Highlighter::new(Some("base16-ocean.dark".to_string()));
    let code = "fn main() {}\n";
    let result = h.highlight_code(code, Some("rust"));
    assert!(result.is_some());
    let lines = result.unwrap();
    // With a named theme, should produce Rgb colors
    let has_rgb = lines.iter().any(|line| {
        line.iter().any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
    });
    assert!(has_rgb, "Named theme should produce Rgb colors");
}

#[test]
fn test_highlight_default_theme_uses_ansi_colors() {
    let h = Highlighter::new(None);
    let code = "fn main() {}\n";
    let result = h.highlight_code(code, Some("rust"));
    assert!(result.is_some());
    let lines = result.unwrap();
    // Default (ANSI) mode should NOT produce Rgb colors
    let has_rgb = lines.iter().any(|line| {
        line.iter().any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
    });
    assert!(!has_rgb, "Default ANSI mode should not produce Rgb colors");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p mdx test_highlighter -- 2>&1 && cargo test -p mdx test_highlight_ -- 2>&1`
Expected: compilation errors — `Highlighter` struct doesn't exist yet.

- [ ] **Step 4: Implement Highlighter struct and highlight_code**

Add to `src/highlight.rs` (above the existing `rgb_to_ansi_color` function, below the `use` statements):

```rust
use crate::render::{Color, SpanStyle, StyledSpan};
use syntect::highlighting::ThemeSet;
use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: Option<String>,
}

impl Highlighter {
    pub fn new(theme_name: Option<String>) -> Self {
        Highlighter {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name,
        }
    }

    /// Highlight code, returning styled spans per line.
    /// Returns None if the language is unrecognized or absent.
    pub fn highlight_code(&self, code: &str, language: Option<&str>) -> Option<Vec<Vec<StyledSpan>>> {
        let lang = language?;
        let syntax = self.syntax_set.find_syntax_by_token(lang)?;

        let use_rgb = self.theme_name.is_some();
        let theme_key = self.theme_name.as_deref().unwrap_or("base16-ocean.dark");
        let theme = self.theme_set.themes.get(theme_key)?;

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut result = Vec::new();

        for line in code.lines() {
            let ranges = highlighter.highlight_line(line, &self.syntax_set).ok()?;
            let spans: Vec<StyledSpan> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = if style.foreground.a == 0 {
                        None
                    } else if use_rgb {
                        Some(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b))
                    } else {
                        Some(rgb_to_ansi_color(style.foreground.r, style.foreground.g, style.foreground.b))
                    };
                    StyledSpan {
                        text: format!("  {}", text),
                        style: SpanStyle {
                            fg,
                            ..Default::default()
                        },
                    }
                })
                .collect();
            result.push(spans);
        }

        Some(result)
    }
}
```

Note: `highlight_code` returns raw tokens without indent — the renderer adds the 2-space indent when consuming the result.

```rust
                .map(|(style, text)| {
                    let fg = if style.foreground.a == 0 {
                        None
                    } else if use_rgb {
                        Some(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b))
                    } else {
                        Some(rgb_to_ansi_color(style.foreground.r, style.foreground.g, style.foreground.b))
                    };
                    StyledSpan {
                        text: text.to_string(),
                        style: SpanStyle {
                            fg,
                            ..Default::default()
                        },
                    }
                })
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mdx highlight 2>&1`
Expected: all highlight tests PASS.

- [ ] **Step 6: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests pass (highlight module is created but not yet wired into render).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/highlight.rs
git commit -m "feat: implement Highlighter with syntect integration"
```

---

### Task 4: Wire Highlighter into the renderer

**Files:**
- Modify: `src/render.rs:140-171` (render_code_block_lines)
- Modify: `src/render.rs:175` (render_blocks signature)
- Modify: `src/render.rs:324-511` (existing tests)

- [ ] **Step 1: Write failing test for highlighted code block rendering**

Add to the test module in `src/render.rs`:

```rust
#[test]
fn test_render_code_block_with_highlighting() {
    use crate::highlight::Highlighter;

    let highlighter = Highlighter::new(None);
    let blocks = vec![Block::CodeBlock {
        language: Some("rust".to_string()),
        content: "fn main() {}\n".to_string(),
    }];
    let rendered = render_blocks(&blocks, 80, &highlighter);
    assert_eq!(rendered.len(), 1);
    if let RenderedBlock::Lines(lines) = &rendered[0] {
        // Should have language label + code line(s) + blank line
        // The code lines should have some colored spans (not all DarkGray dim)
        let code_lines: Vec<_> = lines.iter().filter(|l| {
            l.spans.iter().any(|s| s.text.contains("fn") || s.text.contains("main"))
        }).collect();
        assert!(!code_lines.is_empty(), "Should have code lines");
        let has_non_gray_color = code_lines.iter().any(|line| {
            line.spans.iter().any(|s| {
                matches!(s.style.fg, Some(ref c) if *c != Color::DarkGray)
            })
        });
        assert!(has_non_gray_color, "Highlighted Rust code should have colors beyond DarkGray");
    } else {
        panic!("Expected Lines variant");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_render_code_block_with_highlighting 2>&1`
Expected: compilation error — `render_blocks` doesn't accept a `&Highlighter` parameter yet.

- [ ] **Step 3: Update render_blocks and render_code_block_lines**

Update `render_code_block_lines` signature in `src/render.rs` to accept the highlighter:

```rust
fn render_code_block_lines(
    language: &Option<String>,
    content: &str,
    highlighter: &crate::highlight::Highlighter,
) -> Vec<StyledLine> {
    let mut lines = Vec::new();

    // Optional language label
    if let Some(lang) = language {
        lines.push(StyledLine {
            spans: vec![StyledSpan {
                text: format!("  [{}]", lang),
                style: SpanStyle {
                    dim: true,
                    fg: Some(Color::DarkGray),
                    ..Default::default()
                },
            }],
        });
    }

    // Try syntax highlighting
    if let Some(highlighted) = highlighter.highlight_code(content, language.as_deref()) {
        for spans in highlighted {
            let mut indented = vec![StyledSpan::plain("  ")];
            indented.extend(spans);
            lines.push(StyledLine { spans: indented });
        }
    } else {
        // Fallback: dim monochrome
        for line in content.lines() {
            lines.push(StyledLine {
                spans: vec![StyledSpan {
                    text: format!("  {}", line),
                    style: SpanStyle {
                        dim: true,
                        fg: Some(Color::DarkGray),
                        ..Default::default()
                    },
                }],
            });
        }
    }

    lines
}
```

Update `render_blocks` signature:

```rust
pub fn render_blocks(
    blocks: &[Block],
    width: u16,
    highlighter: &crate::highlight::Highlighter,
) -> Vec<RenderedBlock> {
```

Update the `Block::CodeBlock` arm inside `render_blocks`:

```rust
Block::CodeBlock { language, content } => {
    let lines = render_code_block_lines(language, content, highlighter);
    let mut all_lines = lines;
    all_lines.push(StyledLine::empty());
    out.push(RenderedBlock::Lines(all_lines));
}
```

Also update the `Block::MermaidBlock` error fallback (line 241) which calls `render_code_block_lines`:

```rust
let code_lines = render_code_block_lines(&None, content, highlighter);
```

- [ ] **Step 4: Update existing render tests to pass Highlighter**

All existing tests in `src/render.rs` that call `render_blocks(&blocks, 80)` need updating to `render_blocks(&blocks, 80, &highlighter)`. Add this at the top of each test that calls `render_blocks`:

```rust
let highlighter = crate::highlight::Highlighter::new(None);
```

Tests to update:
- `test_render_header`
- `test_render_paragraph_with_bold`
- `test_render_code_block`
- `test_render_horizontal_rule`
- `test_render_list`
- `test_render_mermaid_block`
- `test_render_malformed_mermaid_falls_back`

The `test_render_code_block` test currently checks for `dim` styling — update it to check for colored spans instead:

```rust
#[test]
fn test_render_code_block() {
    let highlighter = crate::highlight::Highlighter::new(None);
    let blocks = vec![Block::CodeBlock {
        language: Some("rust".to_string()),
        content: "fn main() {}".to_string(),
    }];
    let rendered = render_blocks(&blocks, 80, &highlighter);
    assert_eq!(rendered.len(), 1);
    if let RenderedBlock::Lines(lines) = &rendered[0] {
        let code_line = lines.iter().find(|l| {
            l.spans.iter().any(|s| s.text.contains("fn main()"))
        });
        assert!(code_line.is_some(), "Should have code line with text");
    } else {
        panic!("Expected Lines variant");
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p mdx render::tests 2>&1`
Expected: all render tests PASS.

- [ ] **Step 6: Run full test suite**

Run: `cargo test 2>&1`
Expected: compilation errors in `main.rs` and `tests/integration.rs` because `render_blocks` signature changed. That's expected — we fix those in the next task.

- [ ] **Step 7: Commit**

```bash
git add src/render.rs
git commit -m "feat: wire syntax highlighting into code block rendering"
```

---

### Task 5: Add `--theme` CLI flag and wire through `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update Args struct and main function**

In `src/main.rs`, add the theme field to `Args`:

```rust
#[derive(Parser)]
#[command(
    name = "mdx",
    version,
    about = "Terminal markdown renderer with mermaid diagrams"
)]
struct Args {
    /// Markdown file to render
    file: Option<PathBuf>,

    /// Force pager mode even when piped
    #[arg(short, long)]
    pager: bool,

    /// Force plain output even on TTY
    #[arg(long)]
    no_pager: bool,

    /// Override terminal width for wrapping
    #[arg(short, long)]
    width: Option<u16>,

    /// Syntax highlighting theme (use --theme=list to see available themes)
    #[arg(long)]
    theme: Option<String>,
}
```

Update `main()` to construct and pass the Highlighter:

```rust
fn main() -> Result<()> {
    let args = Args::parse();
    let input = read_input(&args)?;
    let width = get_width(&args);
    let no_color = std::env::var("NO_COLOR").is_ok();

    // Handle --theme=list
    if args.theme.as_deref() == Some("list") {
        let h = highlight::Highlighter::new(None);
        for name in h.available_themes() {
            println!("{}", name);
        }
        return Ok(());
    }

    let highlighter = highlight::Highlighter::new(args.theme);
    let blocks = parser::parse_markdown(&input);
    let rendered = render::render_blocks(&blocks, width, &highlighter);
    if use_pager(&args) {
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
        pager::run_pager(rendered)?;
    } else {
        pipe_output(&rendered, no_color)?;
    }
    Ok(())
}
```

- [ ] **Step 2: Add `available_themes` to Highlighter**

In `src/highlight.rs`, add this method to the `impl Highlighter` block:

```rust
pub fn available_themes(&self) -> Vec<&str> {
    let mut names: Vec<&str> = self.theme_set.themes.keys().map(|s| s.as_str()).collect();
    names.sort();
    names
}
```

- [ ] **Step 3: Update existing main.rs tests**

Update `test_read_input_file` and `test_read_input_no_file_no_stdin` — add `theme: None` to the `Args` struct literal:

```rust
let args = Args {
    file: Some(path),
    pager: false,
    no_pager: false,
    width: None,
    theme: None,
};
```

Do the same for `test_read_input_no_file_no_stdin`.

- [ ] **Step 4: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests pass except integration tests (which still use the old binary — need rebuild). Check that unit tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/highlight.rs
git commit -m "feat: add --theme CLI flag for syntax highlighting themes"
```

---

### Task 6: Integration tests

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration test for syntax highlighting**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_syntax_highlighting_produces_colors() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight.md");
    std::fs::write(
        &path,
        "# Code\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain ANSI escape codes for syntax highlighting
    assert!(
        stdout.contains("\x1b["),
        "Highlighted output should contain ANSI escapes: {}",
        stdout
    );
    assert!(stdout.contains("fn"), "Should contain the code text");
    assert!(stdout.contains("main"), "Should contain the code text");
}
```

- [ ] **Step 2: Write integration test for NO_COLOR**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_no_color_strips_highlighting() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight_nocolor.md");
    std::fs::write(
        &path,
        "```rust\nfn main() {}\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "NO_COLOR output should have no ANSI escapes: {}",
        stdout
    );
    assert!(stdout.contains("fn main()"), "Should still contain code text");
}
```

- [ ] **Step 3: Write integration test for --theme flag**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_theme_flag_produces_output() {
    let dir = std::env::temp_dir().join("mdx_integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("highlight_theme.md");
    std::fs::write(
        &path,
        "```rust\nfn main() {}\n```\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(&path)
        .arg("--no-pager")
        .arg("--theme=base16-ocean.dark")
        .output()
        .expect("failed to run mdx");
    assert!(output.status.success(), "Should succeed with valid theme");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Named theme uses 24-bit color escapes (38;2;r;g;b)
    assert!(
        stdout.contains("38;2;"),
        "Named theme should use 24-bit RGB colors: {}",
        stdout
    );
}

#[test]
fn test_theme_list() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("--theme=list")
        .output()
        .expect("failed to run mdx");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("base16-ocean.dark"),
        "Theme list should include base16-ocean.dark: {}",
        stdout
    );
}
```

- [ ] **Step 4: Run all integration tests**

Run: `cargo test --test integration 2>&1`
Expected: all integration tests PASS.

- [ ] **Step 5: Run full test suite**

Run: `cargo test 2>&1`
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/integration.rs
git commit -m "test: add integration tests for syntax highlighting"
```
