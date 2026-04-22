# Mermaid Diagram Color Support

**Date:** 2026-04-22
**Status:** Design

## Overview

Add color support for mermaid diagrams by parsing Mermaid style directives (`style`, `classDef`/`class`, `linkStyle`) and rendering colored ASCII output through the existing styled rendering pipeline. User-specified hex/named colors are resolved to the nearest color in the active theme's palette to maintain visual coherence.

Diagrams remain monochrome by default. Colors only appear when the user writes explicit style directives.

## Style Directive Parsing

### Flowcharts

Three directive types, matching Mermaid syntax:

**`style`** — inline per-node styling:
```
style A fill:#f9f,stroke:#333,color:#000
```
Properties:
- `fill` — node background (colors border interior chars)
- `stroke` — node border chars (e.g. `┌─┐│└─┘`)
- `color` — node label text

**`classDef`** — reusable style class:
```
classDef highlight fill:#f9f,stroke:#333
```

**`class`** — apply class to nodes:
```
class A,B highlight
```

**`linkStyle`** — per-edge by declaration order (0-based index):
```
linkStyle 0 stroke:#ff3
```
Properties:
- `stroke` — line and arrow chars

### Sequence Diagrams

Same directives apply:
- `style` targets participants by name
- `linkStyle` targets messages by declaration order (0-based)
- `classDef`/`class` work on participants

### Color Formats

Supported input formats:
- `#RGB` — shorthand hex (e.g. `#f9f` → `(255, 153, 255)`)
- `#RRGGBB` — full hex (e.g. `#ff99ff`)
- CSS named colors — `red`, `blue`, `green`, `cyan`, `magenta`, `yellow`, `white`, `black`, `orange`, `purple`, `pink`, `gray`/`grey`

Invalid color values are silently ignored — the affected property renders unstyled.

### Parsed Data Structures

```rust
pub struct NodeStyle {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub color: Option<Color>,
}

pub struct MermaidEdgeStyle {
    pub stroke: Option<Color>,
    pub label_color: Option<Color>,  // inherits stroke if unset
}
```

These attach to existing structs:
- `Node` gains `pub style: Option<NodeStyle>`
- `Edge` gains `pub style: Option<MermaidEdgeStyle>`
- Sequence `Participant` gains `pub style: Option<NodeStyle>`
- Sequence `Event::Message` gains style info via `MermaidEdgeStyle`

`classDef` definitions are stored during parsing and resolved to inline styles when `class` directives are encountered, so downstream code only sees per-node/per-edge styles.

## Color Resolution: Nearest Theme Match

User-specified colors are not rendered verbatim. Instead, each color is resolved to the nearest color in the active theme's full palette using euclidean distance in RGB space.

### Algorithm

1. Parse input color to `(r, g, b)` tuple
2. Collect all colors from the active theme: headings (6), body, bold, italic, link, inline_code, horizontal_rule, diagram_border, diagram_collapsed, plus the 5 new diagram slots (~18 total)
3. For each theme color, compute: `sqrt((r1-r2)^2 + (g1-g2)^2 + (b1-b2)^2)`
4. Return the theme color with the smallest distance

### When Resolution Happens

At **render time**, not parse time. Parsed nodes/edges store the original color value. The renderer resolves against the active theme when applying styles to the canvas. This means switching themes (e.g. `--ui-theme hearth`) changes diagram colors without re-parsing.

## Theme Palette Extension

Five new slots added to the `Theme` struct:

```rust
pub diagram_node_fill: Color,
pub diagram_node_border: Color,
pub diagram_node_text: Color,
pub diagram_edge_stroke: Color,
pub diagram_edge_label: Color,
```

### Clay Theme Values

```rust
diagram_node_fill: Color::Rgb(160, 120, 60),    // warm amber (matches inline_code)
diagram_node_border: Color::Rgb(180, 90, 60),    // clay red (matches H2)
diagram_node_text: Color::Rgb(190, 180, 160),    // body text
diagram_edge_stroke: Color::Rgb(120, 160, 80),   // olive (matches H3)
diagram_edge_label: Color::Rgb(130, 140, 110),   // driftwood (matches H5)
```

### Hearth Theme Values

```rust
diagram_node_fill: Color::Rgb(200, 160, 80),     // warm gold (matches inline_code)
diagram_node_border: Color::Rgb(200, 100, 50),   // rust (matches H2)
diagram_node_text: Color::Rgb(210, 200, 180),    // body text
diagram_edge_stroke: Color::Rgb(100, 170, 90),   // forest (matches H3)
diagram_edge_label: Color::Rgb(150, 140, 120),   // sandstone (matches H5)
```

These slots serve two purposes:
1. Part of the palette pool for nearest-match resolution
2. Default colors for styled elements if a specific property is omitted (e.g. `style A fill:#f00` with no `stroke` → border stays unstyled)

## Styled Canvas

### Cell Type

The `Canvas` struct (used in both `mermaid/ascii.rs` and `mermaid/sequence/ascii.rs`) changes from `Vec<Vec<char>>` to a styled grid:

```rust
struct Cell {
    ch: char,
    style: SpanStyle,
}
```

Default cell: `Cell { ch: ' ', style: SpanStyle::default() }` — no color, no formatting.

### Canvas API

Existing methods preserved with default style (unstyled elements unchanged):
- `set(x, y, ch)` — sets char with default style
- `draw_text(x, y, text)` — draws text with default style

New styled variants:
- `set_styled(x, y, ch, style)` — sets char with explicit style
- `draw_text_styled(x, y, text, style)` — draws text with explicit style

### Output

`to_lines()` returns `Vec<StyledLine>` instead of `Vec<String>`. Implementation walks each row, grouping consecutive cells with identical `SpanStyle` into `StyledSpan`s.

### Drawing Code Changes

Node drawing functions (`draw_rect`, `draw_rounded`, `draw_diamond`, `draw_circle`) receive a resolved `Option<NodeStyle>`:
- Border chars (`┌─┐│└─┘`, `╭╮╰╯`, etc.) use `stroke` color
- Label text uses `color`
- `fill` is applied as the foreground color on the border chars as a fallback when `stroke` is not set. When both `fill` and `stroke` are present, `stroke` takes precedence on borders. True background fill is a non-goal (see Non-Goals)

Edge drawing functions receive a resolved `Option<MermaidEdgeStyle>`:
- Line chars (`─│┌┐└┘`, arrows `►◄▲▼`) use `stroke` color
- Label text uses `label_color` (falls back to `stroke` if unset)

Same changes apply to `sequence/ascii.rs`:
- Participant box drawing uses `NodeStyle`
- Message arrow drawing uses `MermaidEdgeStyle`
- Lifelines, fragment borders, and notes remain unstyled (no directive targets them)

## Render Pipeline Integration

### RenderedBlock Change

```rust
pub enum RenderedBlock {
    Lines(Vec<StyledLine>),
    Diagram { lines: Vec<StyledLine>, node_count: usize, edge_count: usize },
    Image { alt: String, url: String },
}
```

`Vec<String>` → `Vec<StyledLine>` for diagram lines.

### render_mermaid Signature

```rust
pub fn render_mermaid(content: &str, theme: &Theme) -> anyhow::Result<(Vec<StyledLine>, usize, usize)>
```

Theme parameter added for color resolution at render time.

### Pager (pager.rs)

`FlatLine::DiagramAscii(String)` → `FlatLine::DiagramAscii(StyledLine)`. Rendering path already handles `StyledLine` → ratatui spans via `color_to_ratatui`. No new rendering logic needed.

### Pipe Output (main.rs)

`styled_line_to_ansi()` already handles `StyledLine`. Diagram lines go through the same codepath. `NO_COLOR` env var disables coloring automatically.

### Watch Mode (watch.rs)

Mermaid cache stores `Vec<StyledLine>` instead of `Vec<String>`. Theme passed through to re-render when theme changes.

## Non-Goals

- **Auto-coloring** — no style directive = monochrome, same as today
- **Terminal background colors** — `SpanStyle` does not currently support background color; we color foreground (border/text chars) only
- **`stroke-width`** — no way to represent line thickness in ASCII
- **`stroke-dasharray`** — edge style (dotted/thick) already handled by `EdgeStyle` enum
- **Subgraph styling** — subgraphs not yet supported
- **New themes or theme customization CLI** — out of scope
- **`opacity`** — no terminal equivalent

## Degradation

- `NO_COLOR` env var: style directives parsed, colors not emitted. Monochrome ASCII output.
- Non-TTY pipe with `--no-pager`: same — `styled_line_to_ansi` respects `no_color` flag.
- Unknown/invalid color values: silently ignored, affected property renders unstyled.
- Unsupported style properties: silently ignored.
