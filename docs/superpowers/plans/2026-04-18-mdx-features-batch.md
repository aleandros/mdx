# mdx Feature Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add theming, mermaid rendering modes, image support, snapshot tests, versioning/changelog, and git hooks to mdx.

**Architecture:** Six features implemented in dependency order. Theming first (others depend on Theme struct). Mermaid modes and image support modify the render pipeline. Snapshot tests lock output after rendering changes. Versioning and git hooks are infrastructure, independent of code features.

**Tech Stack:** Rust (edition 2024), clap 4, pulldown-cmark 0.12, ratatui 0.29, syntect 5, insta (snapshot testing)

---

## File Structure

### New Files
- `src/theme.rs` — Theme struct, built-in theme definitions, lookup
- `hooks/pre-commit` — Shell script: cargo fmt + clippy
- `hooks/pre-push` — Shell script: cargo test
- `hooks/install.sh` — Symlink installer
- `.github/workflows/tag.yml` — Auto-tag on version change
- `CHANGELOG.md` — Keep-a-Changelog format
- `tests/snapshots.rs` — Snapshot integration tests

### Modified Files
- `src/main.rs` — Add `--ui-theme`, `--no-mermaid-rendering`, `--split-mermaid-rendering` flags; pass theme/mode through pipeline; handle image opening
- `src/render.rs` — Accept `&Theme` in `render_blocks`; replace hardcoded colors; add `MermaidMode` enum; add `RenderedBlock::Image` variant
- `src/parser.rs` — Add `Block::Image` variant; capture image events
- `src/pager.rs` — Add `FlatLine::ImagePlaceholder`; Tab opens images; accept theme for styling
- `src/highlight.rs` — No changes
- `Cargo.toml` — Add `insta` dev-dependency

---

## Task 1: Theme Module

**Files:**
- Create: `src/theme.rs`
- Modify: `src/main.rs:1` (add `mod theme;`)

- [ ] **Step 1: Write tests for theme lookup**

Create `src/theme.rs` with tests only:

```rust
use crate::render::Color;

pub struct Theme {
    pub name: &'static str,
    pub heading: [Color; 6],
    pub body: Color,
    pub bold: Color,
    pub italic: Color,
    pub link: Color,
    pub inline_code: Color,
    pub horizontal_rule: Color,
    pub diagram_border: Color,
    pub diagram_collapsed: Color,
}

impl Theme {
    pub fn by_name(name: &str) -> Option<&'static Theme> {
        match name {
            "clay" => Some(&CLAY),
            "hearth" => Some(&HEARTH),
            _ => None,
        }
    }

    pub fn default_theme() -> &'static Theme {
        &CLAY
    }

    pub fn available_names() -> &'static [&'static str] {
        &["clay", "hearth"]
    }
}

static CLAY: Theme = Theme {
    name: "clay",
    heading: [
        Color::Rgb(210, 140, 40),  // H1: Dark Honey
        Color::Rgb(180, 90, 60),   // H2: Clay Red
        Color::Rgb(120, 160, 80),  // H3: Olive
        Color::Rgb(160, 110, 70),  // H4: Sienna
        Color::Rgb(130, 140, 110), // H5: Driftwood
        Color::Rgb(110, 115, 100), // H6: Slate Moss
    ],
    body: Color::Rgb(190, 180, 160),
    bold: Color::Rgb(190, 180, 160),
    italic: Color::Rgb(190, 180, 160),
    link: Color::Rgb(120, 150, 100),
    inline_code: Color::Rgb(160, 120, 60),
    horizontal_rule: Color::Rgb(90, 80, 60),
    diagram_border: Color::Rgb(160, 120, 60),
    diagram_collapsed: Color::Rgb(120, 150, 100),
};

static HEARTH: Theme = Theme {
    name: "hearth",
    heading: [
        Color::Rgb(240, 180, 60),  // H1: Sunflower
        Color::Rgb(200, 100, 50),  // H2: Rust
        Color::Rgb(100, 170, 90),  // H3: Forest
        Color::Rgb(190, 140, 90),  // H4: Caramel
        Color::Rgb(150, 140, 120), // H5: Sandstone
        Color::Rgb(130, 125, 110), // H6: Flint
    ],
    body: Color::Rgb(210, 200, 180),
    bold: Color::Rgb(210, 200, 180),
    italic: Color::Rgb(210, 200, 180),
    link: Color::Rgb(100, 170, 90),
    inline_code: Color::Rgb(200, 160, 80),
    horizontal_rule: Color::Rgb(110, 100, 80),
    diagram_border: Color::Rgb(170, 130, 70),
    diagram_collapsed: Color::Rgb(100, 170, 90),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clay_is_default() {
        let theme = Theme::default_theme();
        assert_eq!(theme.name, "clay");
    }

    #[test]
    fn test_lookup_by_name() {
        assert!(Theme::by_name("clay").is_some());
        assert!(Theme::by_name("hearth").is_some());
        assert!(Theme::by_name("nonexistent").is_none());
    }

    #[test]
    fn test_available_names() {
        let names = Theme::available_names();
        assert!(names.contains(&"clay"));
        assert!(names.contains(&"hearth"));
    }

    #[test]
    fn test_clay_heading_count() {
        let theme = Theme::by_name("clay").unwrap();
        assert_eq!(theme.heading.len(), 6);
    }

    #[test]
    fn test_hearth_heading_count() {
        let theme = Theme::by_name("hearth").unwrap();
        assert_eq!(theme.heading.len(), 6);
    }
}
```

- [ ] **Step 2: Register the module**

In `src/main.rs`, add `mod theme;` after the existing module declarations (line 5):

```rust
mod highlight;
mod mermaid;
mod pager;
mod parser;
mod render;
mod theme;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib theme`
Expected: 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/theme.rs src/main.rs
git commit -m "feat: add theme module with clay and hearth built-in themes"
```

---

## Task 2: Wire Theme Into Rendering

**Files:**
- Modify: `src/render.rs:86-95` (replace `header_color`), `src/render.rs:97-131` (replace `render_inline`), `src/render.rs:188-301` (update `render_blocks` signature and body)
- Modify: `src/main.rs:96-131` (pass theme through pipeline)

- [ ] **Step 1: Update `render_blocks` to accept `&Theme`**

In `src/render.rs`, change the `render_blocks` signature and replace all hardcoded colors:

```rust
pub fn render_blocks(
    blocks: &[Block],
    width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &crate::theme::Theme,
) -> Vec<RenderedBlock> {
```

Replace `header_color(level)` calls with theme lookup. Inside the `Block::Header` match arm, replace:
```rust
let color = header_color(*level);
```
with:
```rust
let color = theme.heading[(*level as usize - 1).min(5)].clone();
```

Replace inline element styling in `render_inline` — change the function signature to accept `&Theme`:
```rust
fn render_inline(elem: &InlineElement, theme: &crate::theme::Theme) -> StyledSpan {
```

Update the match arms:
- `InlineElement::Text` → style with `theme.body`:
  ```rust
  InlineElement::Text(t) => StyledSpan {
      text: t.clone(),
      style: SpanStyle {
          fg: Some(theme.body.clone()),
          ..Default::default()
      },
  },
  ```
- `InlineElement::Bold` → `theme.bold` + bold flag
- `InlineElement::Italic` → `theme.italic` + italic flag
- `InlineElement::Code` → `theme.inline_code` + dim
- `InlineElement::Link` → `theme.link`
- `InlineElement::SoftBreak` → unchanged

Update `render_inline_elements` to pass theme:
```rust
fn render_inline_elements(content: &[InlineElement], theme: &crate::theme::Theme) -> StyledLine {
    StyledLine {
        spans: content.iter().map(|e| render_inline(e, theme)).collect(),
    }
}
```

Update the `Block::HorizontalRule` arm to use `theme.horizontal_rule`:
```rust
Block::HorizontalRule => {
    let rule_char = '─';
    let rule_text: String = std::iter::repeat_n(rule_char, width as usize).collect();
    let rule_line = StyledLine {
        spans: vec![StyledSpan {
            text: rule_text,
            style: SpanStyle {
                dim: true,
                fg: Some(theme.horizontal_rule.clone()),
                ..Default::default()
            },
        }],
    };
    out.push(RenderedBlock::Lines(vec![rule_line]));
}
```

Update all call sites of `render_inline` and `render_inline_elements` within `render_blocks` to pass `theme`.

In the `Block::Paragraph` arm:
```rust
Block::Paragraph { content } => {
    let line = render_inline_elements(content, theme);
    out.push(RenderedBlock::Lines(vec![line, StyledLine::empty()]));
}
```

In the `Block::List` arm, update the `render_inline` call:
```rust
spans.extend(item.iter().map(|e| render_inline(e, theme)));
```

The `header_color` function can be removed entirely since theme handles it.

- [ ] **Step 2: Update call site in `main.rs`**

In `src/main.rs`, add theme resolution and pass it to `render_blocks`. After the `highlighter` line (around line 112), add:

```rust
let ui_theme = theme::Theme::default_theme();
```

Update the `render_blocks` call:
```rust
let rendered = render::render_blocks(&blocks, width, &highlighter, ui_theme);
```

- [ ] **Step 3: Fix existing tests in `render.rs`**

All existing `render_blocks` calls in tests need the theme parameter. Add this helper at the top of the test module:

```rust
fn test_theme() -> &'static crate::theme::Theme {
    crate::theme::Theme::default_theme()
}
```

Then update every `render_blocks` call in tests, e.g.:
```rust
let rendered = render_blocks(&blocks, 80, &highlighter, test_theme());
```

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass (unit + integration)

- [ ] **Step 5: Commit**

```bash
git add src/render.rs src/main.rs
git commit -m "feat: wire theme into rendering pipeline, replace hardcoded colors"
```

---

## Task 3: Add `--ui-theme` CLI Flag

**Files:**
- Modify: `src/main.rs:18-39` (add flag to Args struct), `src/main.rs:96-131` (handle theme flag)

- [ ] **Step 1: Write a test for the new flag parsing**

In the `tests` module of `src/main.rs`, add:

```rust
#[test]
fn test_args_parse_ui_theme() {
    let args = Args::parse_from(["mdx", "--ui-theme", "hearth", "test.md"]);
    assert_eq!(args.ui_theme, Some("hearth".to_string()));
}

#[test]
fn test_args_ui_theme_default_is_none() {
    let args = Args::parse_from(["mdx", "test.md"]);
    assert_eq!(args.ui_theme, None);
}
```

- [ ] **Step 2: Add the flag to Args**

In the `Args` struct in `src/main.rs`, add:

```rust
/// UI color theme for headers, text, and chrome [default: clay]
/// Use --ui-theme=list to see available themes
#[arg(long)]
ui_theme: Option<String>,
```

- [ ] **Step 3: Handle `--ui-theme=list` and theme validation in `main()`**

In `main()`, after the `--theme=list` early return block, add:

```rust
// Handle --ui-theme=list before reading input
if args.ui_theme.as_deref() == Some("list") {
    for name in theme::Theme::available_names() {
        println!("{}", name);
    }
    return Ok(());
}

// Resolve UI theme
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
```

Replace the earlier `let ui_theme = theme::Theme::default_theme();` line with this block.

- [ ] **Step 4: Update the `test_read_input_file` and `test_read_input_no_file_no_stdin` tests**

These tests construct `Args` directly. Add `ui_theme: None` to each:

```rust
let args = Args {
    file: Some(path),
    pager: false,
    no_pager: false,
    width: None,
    theme: None,
    ui_theme: None,
};
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add --ui-theme CLI flag with clay/hearth selection"
```

---

## Task 4: Mermaid Rendering Modes

**Files:**
- Modify: `src/render.rs` (add `MermaidMode` enum, update `MermaidBlock` arm)
- Modify: `src/main.rs` (add CLI flags, pass mode)

- [ ] **Step 1: Write tests for the three mermaid modes**

In the test module of `src/render.rs`, add:

```rust
#[test]
fn test_mermaid_raw_mode_produces_code_block() {
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let blocks = vec![Block::MermaidBlock {
        content: "graph TD\n    A --> B\n".to_string(),
    }];
    let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Raw);
    assert_eq!(rendered.len(), 1);
    assert!(
        matches!(rendered[0], RenderedBlock::Lines(_)),
        "Raw mode should produce Lines, not Diagram"
    );
}

#[test]
fn test_mermaid_split_mode_produces_both() {
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let blocks = vec![Block::MermaidBlock {
        content: "graph TD\n    A --> B\n".to_string(),
    }];
    let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Split);
    assert_eq!(rendered.len(), 2, "Split mode should produce 2 blocks");
    assert!(
        matches!(rendered[0], RenderedBlock::Lines(_)),
        "First block should be code (Lines)"
    );
    assert!(
        matches!(rendered[1], RenderedBlock::Diagram { .. }),
        "Second block should be Diagram"
    );
}

#[test]
fn test_mermaid_render_mode_is_default_behavior() {
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let blocks = vec![Block::MermaidBlock {
        content: "graph TD\n    A --> B\n".to_string(),
    }];
    let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
    assert_eq!(rendered.len(), 1);
    assert!(matches!(rendered[0], RenderedBlock::Diagram { .. }));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib render::tests::test_mermaid_raw`
Expected: FAIL — `MermaidMode` doesn't exist yet

- [ ] **Step 3: Add `MermaidMode` enum and update `render_blocks`**

At the top of `src/render.rs` (after the `RenderedBlock` enum), add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MermaidMode {
    Render,
    Raw,
    Split,
}
```

Update `render_blocks` signature:

```rust
pub fn render_blocks(
    blocks: &[Block],
    width: u16,
    highlighter: &crate::highlight::Highlighter,
    theme: &crate::theme::Theme,
    mermaid_mode: MermaidMode,
) -> Vec<RenderedBlock> {
```

Replace the `Block::MermaidBlock` match arm:

```rust
Block::MermaidBlock { content } => {
    let render_as_code = || {
        RenderedBlock::Lines({
            let mut lines = render_code_block_lines(
                &Some("mermaid".to_string()),
                content,
                highlighter,
            );
            lines.push(StyledLine::empty());
            lines
        })
    };

    match mermaid_mode {
        MermaidMode::Raw => {
            out.push(render_as_code());
        }
        MermaidMode::Render => {
            match mermaid::render_mermaid(content) {
                Ok((lines, node_count, edge_count)) => {
                    out.push(RenderedBlock::Diagram {
                        lines,
                        node_count,
                        edge_count,
                    });
                }
                Err(_) => {
                    let warning_line = StyledLine {
                        spans: vec![StyledSpan {
                            text: "[mermaid: parse error]".to_string(),
                            style: SpanStyle {
                                fg: Some(Color::Red),
                                ..Default::default()
                            },
                        }],
                    };
                    let code_lines =
                        render_code_block_lines(&None, content, highlighter);
                    let mut all_lines = vec![warning_line];
                    all_lines.extend(code_lines);
                    all_lines.push(StyledLine::empty());
                    out.push(RenderedBlock::Lines(all_lines));
                }
            }
        }
        MermaidMode::Split => {
            out.push(render_as_code());
            match mermaid::render_mermaid(content) {
                Ok((lines, node_count, edge_count)) => {
                    out.push(RenderedBlock::Diagram {
                        lines,
                        node_count,
                        edge_count,
                    });
                }
                Err(_) => {
                    let warning_line = StyledLine {
                        spans: vec![StyledSpan {
                            text: "[mermaid: parse error]".to_string(),
                            style: SpanStyle {
                                fg: Some(Color::Red),
                                ..Default::default()
                            },
                        }],
                    };
                    out.push(RenderedBlock::Lines(vec![
                        warning_line,
                        StyledLine::empty(),
                    ]));
                }
            }
        }
    }
}
```

- [ ] **Step 4: Fix all existing `render_blocks` calls in tests**

Add `MermaidMode::Render` as the last argument to every `render_blocks` call in the test module:

```rust
let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
```

- [ ] **Step 5: Update `main.rs` call site**

In `main()`, update the `render_blocks` call:

```rust
let rendered = render::render_blocks(&blocks, width, &highlighter, ui_theme, render::MermaidMode::Render);
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All pass including the three new mermaid mode tests

- [ ] **Step 7: Commit**

```bash
git add src/render.rs src/main.rs
git commit -m "feat: add MermaidMode enum with Render/Raw/Split rendering"
```

---

## Task 5: Mermaid Mode CLI Flags

**Files:**
- Modify: `src/main.rs` (add flags, parse into MermaidMode)

- [ ] **Step 1: Write tests for flag parsing**

In the test module of `src/main.rs`:

```rust
#[test]
fn test_args_no_mermaid_rendering() {
    let args = Args::parse_from(["mdx", "--no-mermaid-rendering", "test.md"]);
    assert!(args.no_mermaid_rendering);
    assert!(!args.split_mermaid_rendering);
}

#[test]
fn test_args_split_mermaid_rendering() {
    let args = Args::parse_from(["mdx", "--split-mermaid-rendering", "test.md"]);
    assert!(args.split_mermaid_rendering);
    assert!(!args.no_mermaid_rendering);
}
```

- [ ] **Step 2: Add the flags to Args struct**

```rust
/// Show raw mermaid source without rendering
#[arg(long)]
no_mermaid_rendering: bool,

/// Show mermaid source followed by rendered diagram
#[arg(long)]
split_mermaid_rendering: bool,
```

- [ ] **Step 3: Parse flags into MermaidMode in `main()`**

Before the `render_blocks` call, add:

```rust
let mermaid_mode = if args.no_mermaid_rendering {
    render::MermaidMode::Raw
} else if args.split_mermaid_rendering {
    render::MermaidMode::Split
} else {
    render::MermaidMode::Render
};
```

Update the `render_blocks` call:

```rust
let rendered = render::render_blocks(&blocks, width, &highlighter, ui_theme, mermaid_mode);
```

- [ ] **Step 4: Update Args construction in tests**

All tests that construct `Args` directly need the new fields:

```rust
let args = Args {
    file: ...,
    pager: false,
    no_pager: false,
    width: None,
    theme: None,
    ui_theme: None,
    no_mermaid_rendering: false,
    split_mermaid_rendering: false,
};
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add --no-mermaid-rendering and --split-mermaid-rendering CLI flags"
```

---

## Task 6: Image Parsing

**Files:**
- Modify: `src/parser.rs` (add `Block::Image`, capture image events)

- [ ] **Step 1: Write tests for image parsing**

In the test module of `src/parser.rs`:

```rust
#[test]
fn test_parse_image() {
    let blocks = parse_markdown("![Alt text](image.png)");
    assert_eq!(blocks.len(), 1);
    assert_eq!(
        blocks[0],
        Block::Image {
            alt: "Alt text".to_string(),
            url: "image.png".to_string(),
        }
    );
}

#[test]
fn test_parse_image_with_url() {
    let blocks = parse_markdown("![Logo](https://example.com/logo.png)");
    assert_eq!(blocks.len(), 1);
    if let Block::Image { alt, url } = &blocks[0] {
        assert_eq!(alt, "Logo");
        assert_eq!(url, "https://example.com/logo.png");
    } else {
        panic!("Expected Image block");
    }
}

#[test]
fn test_parse_image_empty_alt() {
    let blocks = parse_markdown("![](photo.jpg)");
    assert_eq!(blocks.len(), 1);
    if let Block::Image { alt, url } = &blocks[0] {
        assert_eq!(alt, "");
        assert_eq!(url, "photo.jpg");
    } else {
        panic!("Expected Image block");
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test --lib parser::tests::test_parse_image`
Expected: FAIL — `Block::Image` doesn't exist

- [ ] **Step 3: Add `Block::Image` variant and capture image events**

In `src/parser.rs`, add to the `Block` enum:

```rust
Image {
    alt: String,
    url: String,
},
```

In the parser function, add tracking state near the other state variables (around line 64):

```rust
let mut in_image: Option<String> = None; // stores the URL while collecting alt text
let mut image_alt_buf = String::new();
```

Add image event handling in the match block. Before the `_ => {}` catch-all, add:

```rust
Event::Start(Tag::Image { dest_url, .. }) => {
    in_image = Some(dest_url.to_string());
    image_alt_buf.clear();
}
Event::End(TagEnd::Image) => {
    if let Some(url) = in_image.take() {
        blocks.push(Block::Image {
            alt: std::mem::take(&mut image_alt_buf),
            url,
        });
    }
}
```

In the `Event::Text` handler, add an image check before the existing code block check. Change:

```rust
Event::Text(text) => {
    let text_str = text.to_string();
    let target_buf = if in_list_item {
        &mut list_item_buf
    } else {
        &mut inline_buf
    };

    if in_code_block.is_some() {
```

to:

```rust
Event::Text(text) => {
    let text_str = text.to_string();

    if in_image.is_some() {
        image_alt_buf.push_str(&text_str);
    } else if in_code_block.is_some() {
```

Adjust the remaining `else` block accordingly (the `target_buf` declaration moves into the final else branch).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib parser`
Expected: All pass including new image tests

- [ ] **Step 5: Commit**

```bash
git add src/parser.rs
git commit -m "feat: parse markdown images into Block::Image variant"
```

---

## Task 7: Image Rendering

**Files:**
- Modify: `src/render.rs` (add `RenderedBlock::Image`, handle `Block::Image`)

- [ ] **Step 1: Write tests for image rendering**

In the test module of `src/render.rs`:

```rust
#[test]
fn test_render_image_block() {
    let highlighter = crate::highlight::Highlighter::new(None).unwrap();
    let blocks = vec![Block::Image {
        alt: "A photo".to_string(),
        url: "photo.png".to_string(),
    }];
    let rendered = render_blocks(&blocks, 80, &highlighter, test_theme(), MermaidMode::Render);
    assert_eq!(rendered.len(), 1);
    if let RenderedBlock::Image { alt, url } = &rendered[0] {
        assert_eq!(alt, "A photo");
        assert_eq!(url, "photo.png");
    } else {
        panic!("Expected Image variant, got {:?}", rendered[0]);
    }
}
```

- [ ] **Step 2: Run test to confirm failure**

Run: `cargo test --lib render::tests::test_render_image`
Expected: FAIL — `RenderedBlock::Image` doesn't exist

- [ ] **Step 3: Add `RenderedBlock::Image` and render logic**

In `src/render.rs`, add to the `RenderedBlock` enum:

```rust
Image {
    alt: String,
    url: String,
},
```

In `render_blocks`, add a new match arm for `Block::Image` (before the closing `}` of the for loop):

```rust
Block::Image { alt, url } => {
    out.push(RenderedBlock::Image {
        alt: alt.clone(),
        url: url.clone(),
    });
}
```

- [ ] **Step 4: Update `pipe_output` in `main.rs` to handle images**

In `src/main.rs`, in the `pipe_output` function, add a match arm for the new variant:

```rust
render::RenderedBlock::Image { alt, url } => {
    if alt.is_empty() {
        writeln!(stdout, "[Image]({})", url)?;
    } else {
        writeln!(stdout, "[Image: {}]({})", alt, url)?;
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/render.rs src/main.rs
git commit -m "feat: render image blocks as placeholders in pipe output"
```

---

## Task 8: Image Support in Pager

**Files:**
- Modify: `src/pager.rs` (add `FlatLine::ImagePlaceholder`, Tab to open, platform detection)

- [ ] **Step 1: Add platform opener detection**

At the top of `src/pager.rs` (after the imports), add:

```rust
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
```

- [ ] **Step 2: Add `FlatLine::ImagePlaceholder`**

Add to the `FlatLine` enum:

```rust
ImagePlaceholder {
    alt: String,
    url: String,
    block_index: usize,
},
```

- [ ] **Step 3: Handle image blocks in `rebuild_flat_lines`**

In `PagerState::rebuild_flat_lines`, add a match arm for images inside the `for (block_index, block)` loop:

```rust
RenderedBlock::Image { alt, url } => {
    self.flat_lines.push(FlatLine::ImagePlaceholder {
        alt: alt.clone(),
        url: url.clone(),
        block_index,
    });
}
```

- [ ] **Step 4: Render image placeholders in `flat_line_to_ratatui`**

Add to the `flat_line_to_ratatui` match:

```rust
FlatLine::ImagePlaceholder { alt, .. } => {
    let text = if alt.is_empty() {
        "  [Image — Tab to open]".to_string()
    } else {
        format!("  [Image: {} — Tab to open]", alt)
    };
    Line::from(Span::styled(
        text,
        Style::default()
            .fg(RColor::Cyan)
            .add_modifier(Modifier::DIM),
    ))
}
```

Note: This uses hardcoded Cyan for now. We'll pass the theme to the pager in a later step if needed, but the pager currently doesn't have theme access, and adding it is out of scope for this task. The `diagram_collapsed` color from the theme would be ideal here — if time permits in a polish pass, thread the theme through `run_pager`.

- [ ] **Step 5: Handle Tab on image placeholders**

Update `toggle_diagram_at_scroll` to also check for image placeholders. Add this at the beginning of the method, before the diagram-collapsed search:

```rust
// Check for image placeholder first
for i in start..end {
    if let FlatLine::ImagePlaceholder { url, .. } = &self.flat_lines[i] {
        if let Some(opener) = self.opener.as_ref() {
            let _ = std::process::Command::new(opener)
                .arg(url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        return;
    }
}
```

- [ ] **Step 6: Add `opener` field to `PagerState`**

Add to the `PagerState` struct:

```rust
opener: Option<&'static str>,
```

Update `PagerState::new`:

```rust
fn new(content: Vec<RenderedBlock>, terminal_height: u16) -> Self {
    let mut state = PagerState {
        content,
        flat_lines: Vec::new(),
        scroll: 0,
        expanded: HashSet::new(),
        terminal_height,
        opener: detect_opener(),
    };
    state.rebuild_flat_lines();
    state
}
```

- [ ] **Step 7: Update placeholder text based on opener availability**

In `flat_line_to_ratatui`, this is a static method so it can't access `self.opener`. Instead, we'll handle this in `rebuild_flat_lines` — when no opener is available, use a different alt text format. Change the `ImagePlaceholder` construction in `rebuild_flat_lines`:

Actually, keep it simple. The `flat_line_to_ratatui` method doesn't need opener info — always show "Tab to open" since the Tab handler gracefully no-ops when opener is None. This avoids threading state through the static method.

- [ ] **Step 8: Run tests**

Run: `cargo test`
Expected: All pass (pager code has no unit tests — it's tested via integration tests)

- [ ] **Step 9: Commit**

```bash
git add src/pager.rs
git commit -m "feat: add image placeholder display and Tab-to-open in pager"
```

---

## Task 9: Snapshot Integration Tests

**Files:**
- Create: `tests/snapshots.rs`
- Modify: `Cargo.toml` (add `insta` dev-dependency)

- [ ] **Step 1: Add `insta` dependency**

In `Cargo.toml`, add:

```toml
[dev-dependencies]
insta = "1"
```

- [ ] **Step 2: Run `cargo check` to fetch the crate**

Run: `cargo check`
Expected: Downloads insta, compiles successfully

- [ ] **Step 3: Create the snapshot test file**

Create `tests/snapshots.rs`:

```rust
use std::process::Command;

fn run_mdx(file: &str, width: u16) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg(file)
        .arg("--no-pager")
        .arg("--width")
        .arg(width.to_string())
        .output()
        .unwrap_or_else(|e| panic!("failed to run mdx on {}: {}", file, e));
    assert!(
        output.status.success(),
        "mdx failed on {}: {}",
        file,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("non-utf8 output")
}

macro_rules! snapshot_test {
    ($name:ident, $file:expr, $width:expr) => {
        #[test]
        fn $name() {
            let output = run_mdx($file, $width);
            insta::assert_snapshot!(output);
        }
    };
}

// basic.md
snapshot_test!(snapshot_basic_w80, "docs/examples/basic.md", 80);
snapshot_test!(snapshot_basic_w120, "docs/examples/basic.md", 120);

// flowchart-simple.md
snapshot_test!(
    snapshot_flowchart_simple_w80,
    "docs/examples/flowchart-simple.md",
    80
);
snapshot_test!(
    snapshot_flowchart_simple_w120,
    "docs/examples/flowchart-simple.md",
    120
);

// flowchart-advanced.md
snapshot_test!(
    snapshot_flowchart_advanced_w80,
    "docs/examples/flowchart-advanced.md",
    80
);
snapshot_test!(
    snapshot_flowchart_advanced_w120,
    "docs/examples/flowchart-advanced.md",
    120
);

// mixed-content.md
snapshot_test!(
    snapshot_mixed_content_w80,
    "docs/examples/mixed-content.md",
    80
);
snapshot_test!(
    snapshot_mixed_content_w120,
    "docs/examples/mixed-content.md",
    120
);

// syntax-highlight.md
snapshot_test!(
    snapshot_syntax_highlight_w80,
    "docs/examples/syntax-highlight.md",
    80
);
snapshot_test!(
    snapshot_syntax_highlight_w120,
    "docs/examples/syntax-highlight.md",
    120
);

// test-seq-basic.md
snapshot_test!(
    snapshot_seq_basic_w80,
    "docs/examples/test-seq-basic.md",
    80
);
snapshot_test!(
    snapshot_seq_basic_w120,
    "docs/examples/test-seq-basic.md",
    120
);

// test-seq-complex.md
snapshot_test!(
    snapshot_seq_complex_w80,
    "docs/examples/test-seq-complex.md",
    80
);
snapshot_test!(
    snapshot_seq_complex_w120,
    "docs/examples/test-seq-complex.md",
    120
);
```

- [ ] **Step 4: Run snapshot tests to generate initial snapshots**

Run: `cargo test --test snapshots`
Expected: All tests fail with "new snapshot" — no accepted snapshots yet.

Run: `cargo insta accept`
(Or if `cargo-insta` is not installed: `cargo install cargo-insta` first)

This accepts all new snapshots, creating files in `tests/snapshots/`.

- [ ] **Step 5: Verify snapshots were created**

Run: `ls tests/snapshots/`
Expected: One `.snap` file per test function (14 files)

- [ ] **Step 6: Run tests again to confirm they pass**

Run: `cargo test --test snapshots`
Expected: All 14 tests pass

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock tests/snapshots.rs tests/snapshots/
git commit -m "test: add snapshot integration tests with insta for example files"
```

---

## Task 10: CHANGELOG and Version Bump

**Files:**
- Create: `CHANGELOG.md`

- [ ] **Step 1: Create CHANGELOG.md**

Create `CHANGELOG.md` in the repo root:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- UI theming with two built-in themes: clay (default) and hearth (`--ui-theme` flag)
- Mermaid rendering modes: `--no-mermaid-rendering` and `--split-mermaid-rendering` flags
- Image support with Tab-to-open in pager mode via xdg-open/open
- Snapshot integration tests with insta for all example files
- Pre-commit hook (cargo fmt + clippy) and pre-push hook (cargo test)
- Auto-tagging CI workflow on version bump

## [0.1.0] - 2026-04-17

### Added
- Terminal markdown rendering with pulldown-cmark
- Interactive pager with ratatui (j/k/arrows scroll, mouse support, q to exit)
- Mermaid flowchart rendering as ASCII art (graph TD/LR/BT/RL)
- All node shapes: rect, rounded, diamond, circle
- All edge styles: arrow, plain, dotted, thick with labels
- Mermaid sequence diagram rendering (participants, messages, activations, notes, fragments)
- Syntax highlighting via syntect with `--theme` flag
- Bundled syntax grammars: TOML, Bash, Dockerfile, Kotlin, Swift, Zig, Terraform, TypeScript, TSX, Svelte, Vue, SCSS, HCL
- `--width` flag for custom terminal width
- `--pager` / `--no-pager` flags for output mode control
- `NO_COLOR` environment variable support
- Large diagram collapse/expand with Tab key
- Graceful terminal restore on panic
- CI pipeline (check, test, clippy, fmt)
- Cross-platform release builds (x86_64/aarch64 Linux and macOS)
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG.md with 0.1.0 history and unreleased changes"
```

---

## Task 11: Auto-tagging CI Workflow

**Files:**
- Create: `.github/workflows/tag.yml`

- [ ] **Step 1: Create the workflow file**

Create `.github/workflows/tag.yml`:

```yaml
name: Auto-tag

on:
  push:
    branches: [main]

jobs:
  auto-tag:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Read version from Cargo.toml
        id: version
        run: |
          VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
          echo "version=$VERSION" >> "$GITHUB_OUTPUT"

      - name: Check if tag exists
        id: check
        run: |
          if git rev-parse "v${{ steps.version.outputs.version }}" >/dev/null 2>&1; then
            echo "exists=true" >> "$GITHUB_OUTPUT"
          else
            echo "exists=false" >> "$GITHUB_OUTPUT"
          fi

      - name: Create and push tag
        if: steps.check.outputs.exists == 'false'
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git tag "v${{ steps.version.outputs.version }}"
          git push origin "v${{ steps.version.outputs.version }}"
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/tag.yml
git commit -m "ci: add auto-tagging workflow on Cargo.toml version change"
```

---

## Task 12: Git Hooks

**Files:**
- Create: `hooks/pre-commit`
- Create: `hooks/pre-push`
- Create: `hooks/install.sh`

- [ ] **Step 1: Create `hooks/pre-commit`**

```bash
#!/usr/bin/env bash
set -e

echo "==> Running cargo fmt --check..."
cargo fmt -- --check

echo "==> Running cargo clippy..."
cargo clippy -- -D warnings

echo "==> Pre-commit checks passed."
```

- [ ] **Step 2: Create `hooks/pre-push`**

```bash
#!/usr/bin/env bash
set -e

echo "==> Running cargo test..."
cargo test

echo "==> Pre-push checks passed."
```

- [ ] **Step 3: Create `hooks/install.sh`**

```bash
#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

for hook in pre-commit pre-push; do
    src="$REPO_ROOT/hooks/$hook"
    dst="$HOOKS_DIR/$hook"

    if [ -e "$dst" ] && [ ! -L "$dst" ]; then
        echo "Warning: $dst exists and is not a symlink. Backing up to ${dst}.bak"
        mv "$dst" "${dst}.bak"
    fi

    ln -sf "$src" "$dst"
    chmod +x "$src"
    echo "Installed $hook hook"
done

echo "Done. Hooks installed."
```

- [ ] **Step 4: Make all scripts executable**

Run:
```bash
chmod +x hooks/pre-commit hooks/pre-push hooks/install.sh
```

- [ ] **Step 5: Test the install script**

Run: `./hooks/install.sh`
Expected: "Installed pre-commit hook", "Installed pre-push hook", "Done."

- [ ] **Step 6: Test the pre-commit hook**

Run: `.git/hooks/pre-commit`
Expected: fmt and clippy both pass

- [ ] **Step 7: Commit**

```bash
git add hooks/
git commit -m "chore: add pre-commit and pre-push git hooks with install script"
```

---

## Task 13: Thread Theme Through Pager

**Files:**
- Modify: `src/pager.rs` (accept theme, use themed colors for placeholders)
- Modify: `src/main.rs` (pass theme to `run_pager`)

- [ ] **Step 1: Update `run_pager` signature**

Change `run_pager` in `src/pager.rs`:

```rust
pub fn run_pager(content: Vec<RenderedBlock>, theme: &crate::theme::Theme) -> Result<()> {
```

Pass theme into `PagerState::new`:

```rust
let mut state = PagerState::new(content, term_height, theme);
```

- [ ] **Step 2: Store theme reference in `PagerState`**

Add a `theme` field. Since the theme is `&'static`, this is straightforward:

```rust
struct PagerState {
    content: Vec<RenderedBlock>,
    flat_lines: Vec<FlatLine>,
    scroll: usize,
    expanded: HashSet<usize>,
    terminal_height: u16,
    opener: Option<&'static str>,
    theme: &'static crate::theme::Theme,
}
```

Update `PagerState::new`:

```rust
fn new(content: Vec<RenderedBlock>, terminal_height: u16, theme: &'static crate::theme::Theme) -> Self {
    let mut state = PagerState {
        content,
        flat_lines: Vec::new(),
        scroll: 0,
        expanded: HashSet::new(),
        terminal_height,
        opener: detect_opener(),
        theme,
    };
    state.rebuild_flat_lines();
    state
}
```

- [ ] **Step 3: Use themed colors for diagram collapsed placeholder**

Change `flat_line_to_ratatui` from a static method to an instance method so it can access `self.theme`:

```rust
fn flat_line_to_ratatui(&self, flat: &FlatLine) -> Line<'static> {
```

For `DiagramCollapsed`, replace the hardcoded `RColor::Cyan` with the theme color:

```rust
FlatLine::DiagramCollapsed { .. } => {
    // ... same text formatting ...
    Line::from(Span::styled(
        text,
        Style::default()
            .fg(color_to_ratatui(&self.theme.diagram_collapsed))
            .add_modifier(Modifier::DIM),
    ))
}
```

For `ImagePlaceholder`, similarly use `self.theme.diagram_collapsed`:

```rust
FlatLine::ImagePlaceholder { alt, .. } => {
    let text = if alt.is_empty() {
        "  [Image — Tab to open]".to_string()
    } else {
        format!("  [Image: {} — Tab to open]", alt)
    };
    Line::from(Span::styled(
        text,
        Style::default()
            .fg(color_to_ratatui(&self.theme.diagram_collapsed))
            .add_modifier(Modifier::DIM),
    ))
}
```

- [ ] **Step 4: Update the `terminal.draw` closure**

Since `flat_line_to_ratatui` is now an instance method, update the draw call:

```rust
.map(|fl| state.flat_line_to_ratatui(fl))
```

- [ ] **Step 5: Update `main.rs` call site**

Change the `run_pager` call:

```rust
pager::run_pager(rendered, ui_theme)?;
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add src/pager.rs src/main.rs
git commit -m "feat: thread theme through pager for themed placeholder colors"
```

---

## Task 14: Clippy + Format Pass

**Files:**
- Potentially any modified file

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings. If any, fix them.

- [ ] **Step 2: Run fmt**

Run: `cargo fmt`

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All pass

- [ ] **Step 4: Commit if any changes**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting"
```

(Skip if no changes needed.)

---

## Task 15: Update Snapshots After All Changes

**Files:**
- Modify: `tests/snapshots/*.snap` (auto-updated by insta)

- [ ] **Step 1: Run snapshot tests**

Run: `cargo test --test snapshots`

If any fail (expected — theming changed the output):

- [ ] **Step 2: Review and accept updated snapshots**

Run: `cargo insta review`

Review each diff — the changes should reflect the new theme colors (ANSI escape code differences from old hardcoded colors to new themed RGB values).

Accept all.

- [ ] **Step 3: Run tests again**

Run: `cargo test`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add tests/snapshots/
git commit -m "test: update snapshots for themed rendering output"
```
