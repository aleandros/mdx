# mdx Feature Batch Design Spec

**Date**: 2026-04-17
**Scope**: Six features — image support, git hooks, versioning/changelog, snapshot tests, mermaid rendering modes, theming

---

## 1. Image Support

### Behavior

When the parser encounters `![alt](path)` image syntax:

- **Pager mode**: Show a placeholder line `[Image: alt text — Tab to open]` styled with the theme's `diagram_collapsed` color. Tab key opens the path/URL via the system opener.
- **Pipe mode**: Show `[Image: alt text](path)` as plain text. No interactivity.
- Both local file paths and remote URLs are supported — `xdg-open`/`open` handles both natively.

### Parser Changes

Add a new variant to the `Block` enum:

```rust
Block::Image { alt: String, url: String }
```

pulldown-cmark already emits `Event::Start(Tag::Image { .. })` and `Event::End(Tag::Image { .. })` with inline text events for alt text. Capture these in the parser state machine instead of dropping them.

### Pager Changes

Add a new `FlatLine` variant:

```rust
FlatLine::ImagePlaceholder { alt: String, url: String, block_index: usize }
```

Tab handler logic (already used for diagram expand/collapse):
1. Check if the line at current scroll position is an `ImagePlaceholder`.
2. If so, spawn the system opener as a background process (detached, no stdout/stderr capture).
3. Do not block or leave pager mode.

### Render Changes

Add a new `RenderedBlock::Image { alt, url }` variant. The render step converts `Block::Image` into this variant. When flattening to `FlatLine`s, it becomes an `ImagePlaceholder` (pager) or a styled text line (pipe mode).

### Platform Detection

At startup (or lazily), detect which opener is available:
- Check for `open` command (macOS)
- Fall back to `xdg-open` (Linux)
- If neither found, show placeholder without "Tab to open" hint — just `[Image: alt text]`

Store the detected opener path in a shared location accessible to the pager (e.g., field on `PagerState`).

---

## 2. Pre-commit / Pre-push Git Hooks

### Pre-commit Hook (fast gate)

Runs on every `git commit`:
- `cargo fmt -- --check` — reject unformatted code
- `cargo clippy -- -D warnings` — reject any clippy warning

### Pre-push Hook (full suite)

Runs on every `git push`:
- `cargo test` — all unit and integration tests

### File Layout

```
hooks/
  pre-commit    # Shell script
  pre-push      # Shell script
  install.sh    # Symlinks hooks into .git/hooks/
```

### Hook Script Behavior

- Print each command before running it
- On failure: show the command's output, print a clear error message, exit non-zero
- Colored output if terminal supports it

### install.sh

- Symlinks `hooks/pre-commit` → `.git/hooks/pre-commit`
- Symlinks `hooks/pre-push` → `.git/hooks/pre-push`
- Makes hooks executable
- Idempotent — safe to run multiple times

---

## 3. Versioning & CHANGELOG

### CHANGELOG.md

Format: [Keep a Changelog](https://keepachangelog.com/) v1.1.0.

Structure:
```markdown
# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.0] - 2026-04-17

### Added
- Terminal markdown rendering with pulldown-cmark
- Interactive pager with ratatui (scroll, mouse support)
- Mermaid flowchart rendering (graph TD/LR/BT/RL)
- Mermaid sequence diagram rendering
- Syntax highlighting via syntect with --theme flag
- Bundled syntax grammars (TOML, Bash, Dockerfile, Kotlin, Swift, Zig, Terraform, TypeScript, TSX, Svelte, Vue, SCSS, HCL)
- --width flag for custom terminal width
- --pager / --no-pager flags
- NO_COLOR environment variable support
- Large diagram collapse/expand with Tab key
```

Backfill based on git history. Actual content may vary after reviewing all commits.

### Version Source of Truth

`Cargo.toml` `version` field. clap's `#[command(version)]` already reads it for `mdx --version`.

### Auto-tagging CI Workflow

New file: `.github/workflows/tag.yml`

```yaml
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
      - name: Create tag
        if: steps.check.outputs.exists == 'false'
        run: |
          git tag "v${{ steps.version.outputs.version }}"
          git push origin "v${{ steps.version.outputs.version }}"
```

This triggers the existing `release.yml` workflow which fires on `v*` tag pushes.

### Release Workflow

Bump version in Cargo.toml + update CHANGELOG → merge to main → CI creates tag → existing release.yml builds artifacts.

---

## 4. Snapshot Integration Tests

### Crate

`insta` — standard Rust snapshot testing library.

Add to `Cargo.toml`:
```toml
[dev-dependencies]
insta = "1"
```

### Test Structure

For each example file in `docs/examples/`, render at widths 80 and 120 in pipe mode. Compare against golden snapshots.

Test naming convention: `snapshot_{example_name}_w{width}`

Example:
```rust
#[test]
fn snapshot_basic_w80() {
    let output = run_mdx("docs/examples/basic.md", 80);
    insta::assert_snapshot!(output);
}
```

Where `run_mdx` executes the binary with `--no-pager --width {width}` and captures stdout.

### Snapshot Storage

`tests/snapshots/` — insta's default location. Each snapshot file named by test function.

### ANSI Handling

Snapshots capture raw ANSI output. This catches color/theme regressions, not just structural changes.

### Existing Tests

Keep existing `tests/integration.rs` tests. They test specific behaviors (error handling, flag interactions, structural assertions). Snapshots are complementary — they catch rendering regressions that targeted assertions miss.

### Workflow

1. Change rendering code
2. `cargo test` fails with snapshot mismatch
3. `cargo insta review` to inspect diffs interactively
4. Accept new snapshots or fix the code

---

## 5. Mermaid Rendering Modes

### CLI Flags

Three mutually exclusive flags via clap `ArgGroup`:

```rust
/// Render mermaid diagrams as ASCII art (default)
#[arg(long)]
mermaid_rendering: bool,

/// Show raw mermaid source without rendering
#[arg(long)]
no_mermaid_rendering: bool,

/// Show mermaid source followed by rendered diagram
#[arg(long)]
split_mermaid_rendering: bool,
```

### MermaidMode Enum

```rust
enum MermaidMode {
    Render,  // default — current behavior
    Raw,     // show source as code block
    Split,   // source code block + rendered diagram, stacked
}
```

Parse from CLI flags. Default: `Render`.

### Rendering Behavior

Pass `MermaidMode` to `render_blocks`.

When processing a `Block::MermaidBlock { content }`:

- **`Render`**: Current behavior. Render as ASCII diagram.
- **`Raw`**: Convert to `Block::CodeBlock { language: "mermaid", content }` and render as syntax-highlighted code.
- **`Split`**: Emit two `RenderedBlock`s — first a code block (same as Raw), then the rendered diagram (same as Render).

### Pager Behavior

In split mode, source and diagram are separate blocks. The diagram block still gets collapse/expand via Tab if it exceeds the height threshold. The source code block is always visible and not collapsible.

### Pipe Mode

Same behavior in all modes — split outputs both blocks sequentially to stdout.

---

## 6. Theming

### Built-in Themes

**"clay"** (default) — muted earth tones:
| Element | Color | RGB |
|---------|-------|-----|
| H1 | Dark Honey | (210, 140, 40) |
| H2 | Clay Red | (180, 90, 60) |
| H3 | Olive | (120, 160, 80) |
| H4 | Sienna | (160, 110, 70) |
| H5 | Driftwood | (130, 140, 110) |
| H6 | Slate Moss | (110, 115, 100) |
| Body | Parchment | (190, 180, 160) |
| Bold | Parchment (bold) | (190, 180, 160) |
| Italic | Parchment (italic) | (190, 180, 160) |
| Link | Olive | (120, 150, 100) |
| Inline code | Dim earth | (160, 120, 60) |
| Horizontal rule | Dark earth | (90, 80, 60) |
| Diagram border | Earth | (160, 120, 60) |
| Diagram collapsed | Dim olive | (120, 150, 100) |

**"hearth"** — higher contrast warm tones:
| Element | Color | RGB |
|---------|-------|-----|
| H1 | Sunflower | (240, 180, 60) |
| H2 | Rust | (200, 100, 50) |
| H3 | Forest | (100, 170, 90) |
| H4 | Caramel | (190, 140, 90) |
| H5 | Sandstone | (150, 140, 120) |
| H6 | Flint | (130, 125, 110) |
| Body | Ivory | (210, 200, 180) |
| Bold | Ivory (bold) | (210, 200, 180) |
| Italic | Ivory (italic) | (210, 200, 180) |
| Link | Forest | (100, 170, 90) |
| Inline code | Warm amber | (200, 160, 80) |
| Horizontal rule | Dark earth | (110, 100, 80) |
| Diagram border | Earth gold | (170, 130, 70) |
| Diagram collapsed | Forest | (100, 170, 90) |

### Theme Struct

```rust
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
```

Static instances for each built-in theme. A `Theme::by_name(name: &str) -> Option<&'static Theme>` lookup.

### CLI Flag

New flag `--ui-theme` (separate from existing `--theme` for syntax highlighting):

```rust
/// UI color theme [default: clay]
#[arg(long, default_value = "clay")]
ui_theme: String,
```

`--ui-theme list` prints available themes and exits (same pattern as `--theme list`).

### Integration

- `render_blocks` receives `&Theme` parameter. All hardcoded colors replaced with theme field lookups.
- Pager receives `&Theme` for diagram placeholder styling.
- `NO_COLOR` still overrides everything — strips all ANSI regardless of theme.

### Syntax Highlighting Theme Independence

`--theme` (syntax highlighting) and `--ui-theme` (UI colors) are independent. Each ui-theme could define a suggested default syntax theme, but `--theme` always takes precedence if specified.

---

## Implementation Order

Recommended sequence based on dependencies:

1. **Theming** — introduces `Theme` struct that other features depend on (image placeholders, diagram styling)
2. **Mermaid rendering modes** — modifies render pipeline, good to do before snapshot tests lock output
3. **Image support** — parser + pager changes
4. **Snapshot integration tests** — lock down output after rendering changes are in
5. **Versioning & CHANGELOG** — independent of code features
6. **Git hooks** — independent, can be done anytime

Items 5 and 6 are independent of 1-4 and can be done in parallel.
