# Theme Expansion + Preview Subcommand

## Summary

Add 9 new UI themes to mdx (cold/dark, solarized, light-background) and a `preview-themes` subcommand that renders sample markdown through the real pipeline for each theme.

## New Themes

11 total (2 existing + 9 new):

| Name | Category | Background | Key colors |
|------|----------|------------|------------|
| `clay` | warm | dark | honey, clay red, olive |
| `hearth` | warm | dark | sunflower, rust, forest |
| `frost` | cold | dark | bright blue, teal, lavender |
| `nord` | cold | dark | Nord frost/aurora palette |
| `glacier` | cold | dark | icy cyan, blue, purple accent |
| `steel` | cold | dark | desaturated blues, industrial |
| `solarized-dark` | solarized | dark | Schoonover's canonical palette |
| `solarized-light` | solarized | light | same accents, dark body text |
| `paper` | warm | light | brown, forest green, aged-paper feel |
| `snow` | cool | light | deep blue, teal, crisp white |
| `latte` | vivid | light | Catppuccin Latte-inspired pastels |

RGB values for each theme were previewed and approved via `preview_themes.sh`.

## `preview-themes` Subcommand

### CLI

```
mdx preview-themes
```

No arguments. Prints all themes sequentially to stdout. No pager.

### Implementation

**New module: `src/preview.rs`**

- `pub fn run() -> Result<()>`
- Contains a const `SAMPLE_MARKDOWN` string (~15 lines) exercising: H1, H2, H3, body with bold/italic, a link, inline code, horizontal rule
- For each theme from `Theme::all()`:
  1. Print separator + theme name header (bold, plain ANSI)
  2. `parser::parse_markdown(SAMPLE_MARKDOWN)`
  3. `render::render_blocks(...)` with that theme, terminal width, default highlighter
  4. Print lines via `render::styled_line_to_ansi()`
- Respects `NO_COLOR` env var

### Changes to existing files

**`src/theme.rs`:**
- Add 9 new static `Theme` structs with approved RGB values
- Add `Theme::all() -> &'static [&'static Theme]` returning all 11
- Update `Theme::by_name()` match arms for new names
- Update `Theme::available_names()` to include all 11

**`src/main.rs`:**
- Add `mod preview;`
- Add `PreviewThemes` variant to `Commands` enum
- Dispatch: `Some(Commands::PreviewThemes) => preview::run()`

### Not changed

- Render pipeline, parser, pager, mermaid — untouched
- Existing `clay` and `hearth` themes — unchanged
- No new dependencies
