# Theme Expansion + Preview Subcommand Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 9 new UI themes (cold/dark, solarized, light-background) and a `preview-themes` subcommand that renders sample markdown through the real pipeline for each theme.

**Architecture:** New themes are static `Theme` structs in `theme.rs` following the existing pattern. A new `preview.rs` module contains the subcommand logic — it iterates all themes, feeds a hardcoded sample markdown string through `parser` + `render`, and prints output to stdout. `main.rs` dispatches the new `PreviewThemes` command variant.

**Tech Stack:** Rust, clap (CLI), crossterm (terminal width), existing render pipeline

---

### Task 1: Add `Theme::all()` method and prepare theme.rs for expansion

**Files:**
- Modify: `src/theme.rs:22-61`

- [ ] **Step 1: Write the failing test for `Theme::all()`**

Add to the `#[cfg(test)] mod tests` block in `src/theme.rs`:

```rust
#[test]
fn test_all_returns_every_theme() {
    let all = Theme::all();
    let names = Theme::available_names();
    assert_eq!(all.len(), names.len(), "all() and available_names() must match in length");
    for theme in all {
        assert!(
            names.contains(&theme.name),
            "Theme '{}' in all() but not in available_names()",
            theme.name
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib test_all_returns_every_theme`
Expected: FAIL — `all` method does not exist

- [ ] **Step 3: Add `Theme::all()` method**

Add to the `impl Theme` block in `src/theme.rs`, after `all_colors()`:

```rust
pub fn all() -> &'static [&'static Theme] {
    &[&CLAY, &HEARTH]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib test_all_returns_every_theme`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/theme.rs
git commit -m "feat(theme): add Theme::all() method"
```

---

### Task 2: Add cold dark-background themes (frost, nord, glacier, steel)

**Files:**
- Modify: `src/theme.rs`

- [ ] **Step 1: Write failing tests for the new themes**

Add to the `#[cfg(test)] mod tests` block in `src/theme.rs`:

```rust
#[test]
fn test_frost_lookup() {
    assert!(Theme::by_name("frost").is_some());
    assert_eq!(Theme::by_name("frost").unwrap().heading.len(), 6);
}

#[test]
fn test_nord_lookup() {
    assert!(Theme::by_name("nord").is_some());
    assert_eq!(Theme::by_name("nord").unwrap().heading.len(), 6);
}

#[test]
fn test_glacier_lookup() {
    assert!(Theme::by_name("glacier").is_some());
    assert_eq!(Theme::by_name("glacier").unwrap().heading.len(), 6);
}

#[test]
fn test_steel_lookup() {
    assert!(Theme::by_name("steel").is_some());
    assert_eq!(Theme::by_name("steel").unwrap().heading.len(), 6);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib test_frost_lookup test_nord_lookup test_glacier_lookup test_steel_lookup`
Expected: FAIL — `by_name` returns `None` for these names

- [ ] **Step 3: Add the four static theme definitions**

Add after the `HEARTH` static in `src/theme.rs`:

```rust
static FROST: Theme = Theme {
    name: "frost",
    heading: [
        Color::Rgb(100, 180, 255), // H1: Bright Blue
        Color::Rgb(90, 175, 175),  // H2: Teal
        Color::Rgb(135, 175, 215), // H3: Periwinkle
        Color::Rgb(95, 135, 175),  // H4: Steel Blue
        Color::Rgb(135, 135, 175), // H5: Lavender
        Color::Rgb(108, 112, 134), // H6: Muted Slate
    ],
    body: Color::Rgb(160, 176, 192),
    bold: Color::Rgb(160, 176, 192),
    italic: Color::Rgb(160, 176, 192),
    link: Color::Rgb(90, 175, 175),
    inline_code: Color::Rgb(135, 175, 215),
    horizontal_rule: Color::Rgb(74, 85, 104),
    diagram_border: Color::Rgb(135, 175, 215),
    diagram_collapsed: Color::Rgb(90, 175, 175),
    diagram_node_fill: Color::Rgb(135, 175, 215),
    diagram_node_border: Color::Rgb(100, 180, 255),
    diagram_node_text: Color::Rgb(160, 176, 192),
    diagram_edge_stroke: Color::Rgb(90, 175, 175),
    diagram_edge_label: Color::Rgb(135, 135, 175),
};

static NORD: Theme = Theme {
    name: "nord",
    heading: [
        Color::Rgb(136, 192, 208), // H1: Nord Frost
        Color::Rgb(129, 161, 193), // H2: Nord Frost Dark
        Color::Rgb(163, 190, 140), // H3: Nord Green
        Color::Rgb(235, 203, 139), // H4: Nord Yellow
        Color::Rgb(180, 142, 173), // H5: Nord Purple
        Color::Rgb(94, 129, 172),  // H6: Nord Blue
    ],
    body: Color::Rgb(216, 222, 233),
    bold: Color::Rgb(216, 222, 233),
    italic: Color::Rgb(216, 222, 233),
    link: Color::Rgb(136, 192, 208),
    inline_code: Color::Rgb(129, 161, 193),
    horizontal_rule: Color::Rgb(76, 86, 106),
    diagram_border: Color::Rgb(129, 161, 193),
    diagram_collapsed: Color::Rgb(136, 192, 208),
    diagram_node_fill: Color::Rgb(129, 161, 193),
    diagram_node_border: Color::Rgb(136, 192, 208),
    diagram_node_text: Color::Rgb(216, 222, 233),
    diagram_edge_stroke: Color::Rgb(163, 190, 140),
    diagram_edge_label: Color::Rgb(180, 142, 173),
};

static GLACIER: Theme = Theme {
    name: "glacier",
    heading: [
        Color::Rgb(80, 200, 220),  // H1: Ice Cyan
        Color::Rgb(100, 160, 210), // H2: Arctic Blue
        Color::Rgb(70, 180, 170),  // H3: Teal
        Color::Rgb(150, 130, 200), // H4: Amethyst
        Color::Rgb(130, 150, 180), // H5: Pale Steel
        Color::Rgb(100, 115, 140), // H6: Slate
    ],
    body: Color::Rgb(185, 200, 215),
    bold: Color::Rgb(185, 200, 215),
    italic: Color::Rgb(185, 200, 215),
    link: Color::Rgb(70, 180, 170),
    inline_code: Color::Rgb(110, 150, 200),
    horizontal_rule: Color::Rgb(55, 65, 80),
    diagram_border: Color::Rgb(110, 150, 200),
    diagram_collapsed: Color::Rgb(70, 180, 170),
    diagram_node_fill: Color::Rgb(110, 150, 200),
    diagram_node_border: Color::Rgb(80, 200, 220),
    diagram_node_text: Color::Rgb(185, 200, 215),
    diagram_edge_stroke: Color::Rgb(70, 180, 170),
    diagram_edge_label: Color::Rgb(130, 150, 180),
};

static STEEL: Theme = Theme {
    name: "steel",
    heading: [
        Color::Rgb(140, 170, 210), // H1: Soft Blue
        Color::Rgb(110, 145, 180), // H2: Slate Blue
        Color::Rgb(150, 190, 140), // H3: Sage
        Color::Rgb(180, 160, 120), // H4: Khaki
        Color::Rgb(140, 150, 165), // H5: Pewter
        Color::Rgb(120, 128, 140), // H6: Gunmetal
    ],
    body: Color::Rgb(175, 180, 190),
    bold: Color::Rgb(175, 180, 190),
    italic: Color::Rgb(175, 180, 190),
    link: Color::Rgb(110, 145, 180),
    inline_code: Color::Rgb(150, 160, 180),
    horizontal_rule: Color::Rgb(60, 65, 75),
    diagram_border: Color::Rgb(150, 160, 180),
    diagram_collapsed: Color::Rgb(110, 145, 180),
    diagram_node_fill: Color::Rgb(150, 160, 180),
    diagram_node_border: Color::Rgb(140, 170, 210),
    diagram_node_text: Color::Rgb(175, 180, 190),
    diagram_edge_stroke: Color::Rgb(150, 190, 140),
    diagram_edge_label: Color::Rgb(140, 150, 165),
};
```

- [ ] **Step 4: Update `by_name()`, `available_names()`, and `all()`**

Update the three methods in `impl Theme`:

```rust
pub fn by_name(name: &str) -> Option<&'static Theme> {
    match name {
        "clay" => Some(&CLAY),
        "hearth" => Some(&HEARTH),
        "frost" => Some(&FROST),
        "nord" => Some(&NORD),
        "glacier" => Some(&GLACIER),
        "steel" => Some(&STEEL),
        _ => None,
    }
}

pub fn available_names() -> &'static [&'static str] {
    &["clay", "hearth", "frost", "nord", "glacier", "steel"]
}

pub fn all() -> &'static [&'static Theme] {
    &[&CLAY, &HEARTH, &FROST, &NORD, &GLACIER, &STEEL]
}
```

- [ ] **Step 5: Run all theme tests**

Run: `cargo test --lib theme`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add src/theme.rs
git commit -m "feat(theme): add frost, nord, glacier, steel themes"
```

---

### Task 3: Add solarized themes (solarized-dark, solarized-light)

**Files:**
- Modify: `src/theme.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_solarized_dark_lookup() {
    assert!(Theme::by_name("solarized-dark").is_some());
    assert_eq!(Theme::by_name("solarized-dark").unwrap().heading.len(), 6);
}

#[test]
fn test_solarized_light_lookup() {
    assert!(Theme::by_name("solarized-light").is_some());
    assert_eq!(Theme::by_name("solarized-light").unwrap().heading.len(), 6);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib test_solarized`
Expected: FAIL

- [ ] **Step 3: Add the two static theme definitions**

Add after `STEEL` in `src/theme.rs`:

```rust
static SOLARIZED_DARK: Theme = Theme {
    name: "solarized-dark",
    heading: [
        Color::Rgb(38, 139, 210),  // H1: Blue
        Color::Rgb(42, 161, 152),  // H2: Cyan
        Color::Rgb(133, 153, 0),   // H3: Green
        Color::Rgb(181, 137, 0),   // H4: Yellow
        Color::Rgb(108, 113, 196), // H5: Violet
        Color::Rgb(101, 123, 131), // H6: Base00
    ],
    body: Color::Rgb(131, 148, 150),
    bold: Color::Rgb(131, 148, 150),
    italic: Color::Rgb(131, 148, 150),
    link: Color::Rgb(42, 161, 152),
    inline_code: Color::Rgb(181, 137, 0),
    horizontal_rule: Color::Rgb(88, 110, 117),
    diagram_border: Color::Rgb(181, 137, 0),
    diagram_collapsed: Color::Rgb(42, 161, 152),
    diagram_node_fill: Color::Rgb(181, 137, 0),
    diagram_node_border: Color::Rgb(38, 139, 210),
    diagram_node_text: Color::Rgb(131, 148, 150),
    diagram_edge_stroke: Color::Rgb(42, 161, 152),
    diagram_edge_label: Color::Rgb(108, 113, 196),
};

static SOLARIZED_LIGHT: Theme = Theme {
    name: "solarized-light",
    heading: [
        Color::Rgb(38, 139, 210),  // H1: Blue
        Color::Rgb(42, 161, 152),  // H2: Cyan
        Color::Rgb(133, 153, 0),   // H3: Green
        Color::Rgb(181, 137, 0),   // H4: Yellow
        Color::Rgb(108, 113, 196), // H5: Violet
        Color::Rgb(147, 161, 161), // H6: Base1
    ],
    body: Color::Rgb(101, 123, 131),
    bold: Color::Rgb(101, 123, 131),
    italic: Color::Rgb(101, 123, 131),
    link: Color::Rgb(42, 161, 152),
    inline_code: Color::Rgb(181, 137, 0),
    horizontal_rule: Color::Rgb(147, 161, 161),
    diagram_border: Color::Rgb(181, 137, 0),
    diagram_collapsed: Color::Rgb(42, 161, 152),
    diagram_node_fill: Color::Rgb(181, 137, 0),
    diagram_node_border: Color::Rgb(38, 139, 210),
    diagram_node_text: Color::Rgb(101, 123, 131),
    diagram_edge_stroke: Color::Rgb(42, 161, 152),
    diagram_edge_label: Color::Rgb(108, 113, 196),
};
```

- [ ] **Step 4: Update `by_name()`, `available_names()`, and `all()`**

Add to the match in `by_name()`:
```rust
"solarized-dark" => Some(&SOLARIZED_DARK),
"solarized-light" => Some(&SOLARIZED_LIGHT),
```

Update `available_names()`:
```rust
pub fn available_names() -> &'static [&'static str] {
    &["clay", "hearth", "frost", "nord", "glacier", "steel", "solarized-dark", "solarized-light"]
}
```

Update `all()`:
```rust
pub fn all() -> &'static [&'static Theme] {
    &[&CLAY, &HEARTH, &FROST, &NORD, &GLACIER, &STEEL, &SOLARIZED_DARK, &SOLARIZED_LIGHT]
}
```

- [ ] **Step 5: Run all theme tests**

Run: `cargo test --lib theme`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add src/theme.rs
git commit -m "feat(theme): add solarized-dark and solarized-light themes"
```

---

### Task 4: Add light-background themes (paper, snow, latte)

**Files:**
- Modify: `src/theme.rs`

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_paper_lookup() {
    assert!(Theme::by_name("paper").is_some());
    assert_eq!(Theme::by_name("paper").unwrap().heading.len(), 6);
}

#[test]
fn test_snow_lookup() {
    assert!(Theme::by_name("snow").is_some());
    assert_eq!(Theme::by_name("snow").unwrap().heading.len(), 6);
}

#[test]
fn test_latte_lookup() {
    assert!(Theme::by_name("latte").is_some());
    assert_eq!(Theme::by_name("latte").unwrap().heading.len(), 6);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib test_paper_lookup test_snow_lookup test_latte_lookup`
Expected: FAIL

- [ ] **Step 3: Add the three static theme definitions**

Add after `SOLARIZED_LIGHT` in `src/theme.rs`:

```rust
static PAPER: Theme = Theme {
    name: "paper",
    heading: [
        Color::Rgb(130, 80, 40),   // H1: Dark Brown
        Color::Rgb(150, 60, 30),   // H2: Burnt Sienna
        Color::Rgb(40, 105, 55),   // H3: Forest
        Color::Rgb(80, 65, 140),   // H4: Plum
        Color::Rgb(90, 105, 120),  // H5: Slate
        Color::Rgb(130, 130, 130), // H6: Gray
    ],
    body: Color::Rgb(55, 50, 45),
    bold: Color::Rgb(55, 50, 45),
    italic: Color::Rgb(55, 50, 45),
    link: Color::Rgb(40, 105, 55),
    inline_code: Color::Rgb(120, 85, 40),
    horizontal_rule: Color::Rgb(192, 184, 168),
    diagram_border: Color::Rgb(120, 85, 40),
    diagram_collapsed: Color::Rgb(40, 105, 55),
    diagram_node_fill: Color::Rgb(120, 85, 40),
    diagram_node_border: Color::Rgb(150, 60, 30),
    diagram_node_text: Color::Rgb(55, 50, 45),
    diagram_edge_stroke: Color::Rgb(40, 105, 55),
    diagram_edge_label: Color::Rgb(90, 105, 120),
};

static SNOW: Theme = Theme {
    name: "snow",
    heading: [
        Color::Rgb(25, 105, 160),  // H1: Deep Blue
        Color::Rgb(10, 120, 120),  // H2: Teal
        Color::Rgb(70, 120, 40),   // H3: Olive
        Color::Rgb(90, 75, 190),   // H4: Indigo
        Color::Rgb(100, 115, 130), // H5: Cool Gray
        Color::Rgb(130, 130, 145), // H6: Silver
    ],
    body: Color::Rgb(40, 55, 70),
    bold: Color::Rgb(40, 55, 70),
    italic: Color::Rgb(40, 55, 70),
    link: Color::Rgb(10, 120, 120),
    inline_code: Color::Rgb(80, 65, 140),
    horizontal_rule: Color::Rgb(176, 192, 208),
    diagram_border: Color::Rgb(80, 65, 140),
    diagram_collapsed: Color::Rgb(10, 120, 120),
    diagram_node_fill: Color::Rgb(80, 65, 140),
    diagram_node_border: Color::Rgb(25, 105, 160),
    diagram_node_text: Color::Rgb(40, 55, 70),
    diagram_edge_stroke: Color::Rgb(10, 120, 120),
    diagram_edge_label: Color::Rgb(100, 115, 130),
};

static LATTE: Theme = Theme {
    name: "latte",
    heading: [
        Color::Rgb(30, 102, 245),  // H1: Blue
        Color::Rgb(23, 146, 153),  // H2: Teal
        Color::Rgb(64, 160, 43),   // H3: Green
        Color::Rgb(223, 142, 29),  // H4: Yellow
        Color::Rgb(136, 57, 239),  // H5: Mauve
        Color::Rgb(108, 111, 133), // H6: Overlay
    ],
    body: Color::Rgb(76, 79, 105),
    bold: Color::Rgb(76, 79, 105),
    italic: Color::Rgb(76, 79, 105),
    link: Color::Rgb(23, 146, 153),
    inline_code: Color::Rgb(254, 100, 11),
    horizontal_rule: Color::Rgb(188, 192, 204),
    diagram_border: Color::Rgb(254, 100, 11),
    diagram_collapsed: Color::Rgb(23, 146, 153),
    diagram_node_fill: Color::Rgb(254, 100, 11),
    diagram_node_border: Color::Rgb(30, 102, 245),
    diagram_node_text: Color::Rgb(76, 79, 105),
    diagram_edge_stroke: Color::Rgb(23, 146, 153),
    diagram_edge_label: Color::Rgb(136, 57, 239),
};
```

- [ ] **Step 4: Update `by_name()`, `available_names()`, and `all()`**

Final versions:

```rust
pub fn by_name(name: &str) -> Option<&'static Theme> {
    match name {
        "clay" => Some(&CLAY),
        "hearth" => Some(&HEARTH),
        "frost" => Some(&FROST),
        "nord" => Some(&NORD),
        "glacier" => Some(&GLACIER),
        "steel" => Some(&STEEL),
        "solarized-dark" => Some(&SOLARIZED_DARK),
        "solarized-light" => Some(&SOLARIZED_LIGHT),
        "paper" => Some(&PAPER),
        "snow" => Some(&SNOW),
        "latte" => Some(&LATTE),
        _ => None,
    }
}

pub fn available_names() -> &'static [&'static str] {
    &[
        "clay", "hearth", "frost", "nord", "glacier", "steel",
        "solarized-dark", "solarized-light", "paper", "snow", "latte",
    ]
}

pub fn all() -> &'static [&'static Theme] {
    &[
        &CLAY, &HEARTH, &FROST, &NORD, &GLACIER, &STEEL,
        &SOLARIZED_DARK, &SOLARIZED_LIGHT, &PAPER, &SNOW, &LATTE,
    ]
}
```

- [ ] **Step 5: Run all theme tests**

Run: `cargo test --lib theme`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add src/theme.rs
git commit -m "feat(theme): add paper, snow, latte light-background themes"
```

---

### Task 5: Create `preview.rs` module

**Files:**
- Create: `src/preview.rs`

- [ ] **Step 1: Create `src/preview.rs` with the `run()` function**

```rust
use anyhow::Result;

use crate::highlight::Highlighter;
use crate::render;
use crate::theme::Theme;

const SAMPLE_MARKDOWN: &str = "\
# Heading Level 1

Body text with **bold words** and *italic phrases* in a paragraph.

## Heading Level 2

A link: [example](https://example.com) and inline code: `fn main()`.

### Heading Level 3

#### Heading Level 4

##### Heading Level 5

###### Heading Level 6

---
";

pub fn run() -> Result<()> {
    let width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
    let no_color = std::env::var("NO_COLOR").is_ok();
    let highlighter = Highlighter::new(None).map_err(|e| anyhow::anyhow!(e))?;
    let blocks = crate::parser::parse_markdown(SAMPLE_MARKDOWN);

    for theme in Theme::all() {
        // Theme name header
        println!(
            "\n\x1b[1m  ── {} ──\x1b[0m\n",
            theme.name
        );

        let rendered = render::render_blocks(
            &blocks,
            width,
            &highlighter,
            theme,
            render::MermaidMode::Render,
        );

        for block in &rendered {
            match block {
                render::RenderedBlock::Lines(lines) => {
                    for line in lines {
                        println!("{}", render::styled_line_to_ansi(line, no_color));
                    }
                }
                render::RenderedBlock::Diagram { lines, .. } => {
                    for line in lines {
                        println!("{}", render::styled_line_to_ansi(line, no_color));
                    }
                    println!();
                }
                render::RenderedBlock::Image { alt, url } => {
                    if alt.is_empty() {
                        println!("[Image]({})", url);
                    } else {
                        println!("[Image: {}]({})", alt, url);
                    }
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Write failing integration test**

Add to `tests/integration.rs`:

```rust
#[test]
fn test_preview_themes_runs_and_prints_all_themes() {
    let output = Command::new(env!("CARGO_BIN_EXE_mdx"))
        .arg("preview-themes")
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run mdx preview-themes");
    assert!(output.status.success(), "preview-themes should succeed, stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain every theme name
    for name in &["clay", "hearth", "frost", "nord", "glacier", "steel",
                   "solarized-dark", "solarized-light", "paper", "snow", "latte"] {
        assert!(stdout.contains(name), "output should contain theme '{}', got: {}", name, stdout);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test integration test_preview_themes`
Expected: FAIL — subcommand not recognized

- [ ] **Step 3: Add `mod preview;` and wire up the subcommand in `main.rs`**

Add `mod preview;` after the existing module declarations at the top of `src/main.rs`:

```rust
mod preview;
```

Add the new variant to the `Commands` enum:

```rust
#[derive(clap::Subcommand)]
enum Commands {
    /// Update mdx to the latest version
    Update,
    /// Render markdown into a bounded ANSI stream for embedding in other TUIs
    Embed(EmbedArgs),
    /// Preview all available UI themes with sample markdown
    PreviewThemes,
}
```

Add the dispatch in the `match cli.command` block in `main()`:

```rust
match cli.command {
    Some(Commands::Update) => return self_update::run(),
    Some(Commands::Embed(eargs)) => return run_embed(eargs),
    Some(Commands::PreviewThemes) => return preview::run(),
    None => {}
}
```

- [ ] **Step 4: Run integration test**

Run: `cargo test --test integration test_preview_themes`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all PASS

- [ ] **Step 6: Manual smoke test**

Run: `cargo run -- preview-themes`
Expected: all 11 themes render sequentially with their name headers and sample markdown

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/preview.rs tests/integration.rs
git commit -m "feat: add preview-themes subcommand"
```

---

### Task 6: Clean up preview script

**Files:**
- Delete: `preview_themes.sh`

- [ ] **Step 1: Remove the bash preview script**

```bash
rm preview_themes.sh
```

- [ ] **Step 2: Commit**

```bash
git commit -am "chore: remove bash preview script, replaced by preview-themes subcommand"
```
