# Code Block Syntax Highlighting

## Overview

Add syntax highlighting to fenced code blocks using `syntect`, exposed through a new `src/highlight.rs` module. Default theme maps to ANSI colors for terminal consistency; `--theme` flag selects syntect built-in themes. Unknown languages and `NO_COLOR` fall back to plain monochrome.

## Architecture

```
main.rs (--theme flag)
   |
render.rs (calls highlight module for CodeBlocks)
   |
highlight.rs (wraps syntect: syntax lookup, tokenization, theme management)
   |
syntect (grammars + themes)
```

Data flow unchanged — parser extracts `Block::CodeBlock { language, content }`, renderer owns styling decisions. The new `highlight.rs` module sits between the renderer and syntect, providing a clean abstraction boundary.

## New module: `src/highlight.rs`

### Responsibilities

- Initialize and cache syntect `SyntaxSet` and `ThemeSet`
- Look up syntax definition by language tag
- Tokenize code content into styled spans
- Map syntect `Style` to the existing `StyledSpan`/`SpanStyle` types
- Provide ANSI color mapping mode (default) vs full theme color mode

### Public API

```rust
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: Option<String>,  // None = ANSI-mapped default
}

impl Highlighter {
    pub fn new(theme_name: Option<String>) -> Self;
    pub fn highlight_code(&self, code: &str, language: Option<&str>) -> Vec<Vec<StyledSpan>>;
    pub fn available_themes(&self) -> Vec<&str>;
}
```

- `highlight_code` returns one `Vec<StyledSpan>` per line
- If language is `None` or unrecognized, returns plain unstyled spans
- When `theme_name` is `None`, syntect RGB colors are mapped to the nearest ANSI color
- When a specific theme is selected, syntect RGB colors are used directly

## ANSI color mapping

Syntect themes produce RGB colors. For the default ANSI mode, map each RGB value to the nearest of the 16 standard ANSI colors (the existing `Color` enum in `render.rs`) using Euclidean distance in RGB space. This keeps output consistent with the rest of the renderer and respects terminal color schemes.

When a specific `--theme` is selected, use syntect's RGB colors directly via the `Color::Rgb(u8, u8, u8)` variant.

## Changes to existing files

### `Cargo.toml`

Add `syntect` dependency with default features (includes bundled grammars and themes).

### `src/render.rs`

- `render_blocks()` and `render_code_block_lines()` accept a `&Highlighter` parameter
- For `Block::CodeBlock`, call `highlighter.highlight_code()` instead of applying flat dim cyan
- If highlighter returns unstyled spans (unknown language), fall back to current dim styling
- Add `Rgb(u8, u8, u8)` variant to `Color` enum
- Update `styled_line_to_ansi()` to emit `\x1b[38;2;r;g;bm` for RGB colors

### `src/main.rs`

- Add `--theme` CLI flag (optional string)
- Construct `Highlighter` with theme choice
- Pass `Highlighter` into the rendering pipeline
- When `NO_COLOR` is set, skip highlighter entirely — use plain monochrome

### `src/pager.rs`

- Map `Color::Rgb(r, g, b)` to `ratatui::style::Color::Rgb(r, g, b)` in the existing color conversion function

## NO_COLOR behavior

When `NO_COLOR` is set:
- Highlighter is not used
- Code blocks render as plain text (no colors, no dim)
- Consistent with current behavior and the NO_COLOR spec

## Fallback behavior

- No language tag: plain monochrome (current dim styling)
- Unrecognized language tag: same plain monochrome
- No auto-detection or guessing

## Testing

- **Unit tests in `highlight.rs`:** known language produces styled output, unknown language returns unstyled spans, ANSI mapping produces valid `Color` variants
- **Integration test:** pipe a markdown file with a Rust code block, verify ANSI output contains color codes within the code block region
- **`--theme` flag test:** verify selecting a named theme produces different output than default
- **`NO_COLOR` test:** verify no ANSI color codes appear in code block output
