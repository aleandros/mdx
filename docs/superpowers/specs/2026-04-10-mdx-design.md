# mdx — Terminal Markdown Renderer with Mermaid ASCII Diagrams

## Overview

`mdx` is a Rust CLI tool that renders markdown files in the terminal with ANSI styling, similar to `glow`. Its distinguishing feature is the ability to render mermaid flowchart diagrams as ASCII art directly in the terminal. Small diagrams are inlined; large ones are collapsed behind a Tab-to-expand interaction in pager mode.

## Architecture

### Pipeline

```
Input (file/stdin)
  -> pulldown-cmark parser (event stream)
  -> Block collector (groups events into renderable blocks)
  -> Renderer (converts blocks to styled terminal content)
      |-- Text blocks -> ANSI-styled strings
      |-- Mermaid blocks -> parse flowchart -> graph layout -> ASCII art
  -> Display layer
      |-- TTY -> ratatui pager with scrolling + Tab-to-expand
      |-- Pipe -> plain ANSI output to stdout
```

### Modules

- **`parser`** — Thin wrapper around pulldown-cmark. Groups markdown events into `Block` enums (Header, Paragraph, CodeBlock, MermaidBlock, List, HorizontalRule, etc.).
- **`render`** — Converts `Block`s into styled line buffers suitable for display.
- **`mermaid`** — Parses flowchart syntax, runs graph layout, produces ASCII art. Three sub-components: parser, layout, ASCII renderer.
- **`pager`** — ratatui-based interactive viewer with scrolling and diagram expansion.
- **`main`** — CLI arg handling, TTY detection, orchestration.

## Mermaid Flowchart Engine

### Parser

Handles the flowchart subset of mermaid syntax:

- Direction declarations: `graph TD`, `graph LR`, `graph TB`, `graph RL`
- Node definitions with shapes:
  - `A[rect]` — rectangle
  - `A(rounded)` — rounded rectangle
  - `A{diamond}` — diamond/decision
  - `A((circle))` — circle
- Edge types: `-->`, `---`, `-.->`, `==>`, with optional labels `-->|text|`
- Subgraphs are out of scope for the prototype

Produces a graph data structure: nodes (id, label, shape) and edges (from, to, label, style).

### Layout

Primary strategy: use the `layout-rs` crate to assign (x, y) coordinates and edge routes. If `layout-rs` proves unsuitable (API mismatch, insufficient control over output), implement a custom layered layout (Sugiyama-style) with rank assignment, ordering within ranks, and coordinate assignment. For a flowchart-only prototype, a basic custom implementation is tractable.

### ASCII Renderer

Takes the positioned graph and draws into a 2D character buffer (`Vec<Vec<char>>`):

- Boxes with box-drawing characters (`┌─┐│└─┘` for rects, rounded variants for others)
- Diamond shapes approximated with `/\ \/`
- Edges drawn with `─`, `│`, `┼` and arrow heads `>`, `v`, `^`, `<`
- Edge labels placed along the line

## Pager & Interaction

### TTY Detection

At startup, check `stdout.is_terminal()`. TTY -> launch ratatui pager. Pipe -> write ANSI output to stdout and exit.

### Pager Mode

- Document rendered into a list of styled line segments
- Scrolling: arrow keys, Page Up/Down, Home/End, `j`/`k`, `q` to quit
- Mouse scroll support via crossterm

### Diagram Expansion

- Diagrams exceeding 50% of terminal height are collapsed by default
- Collapsed display: `[Flowchart: N nodes, M edges -- Tab to expand]`
- Tab on that line expands inline (pushes content below down)
- Tab again collapses
- Diagrams under 50% terminal height are always inlined

### Pipe Mode

- All blocks rendered to ANSI strings written to stdout sequentially
- Diagrams always inlined (no interactivity)
- Respects `NO_COLOR` environment variable

## Markdown Rendering (Prototype Scope)

Basic but functional styling:

- **Headers** — colored and bold, brightness decreasing with depth
- **Bold/Italic** — terminal bold/italic ANSI codes
- **Code spans** — dimmed or different color
- **Fenced code blocks** — dim color, no syntax highlighting
- **Lists** — indented with `*` (unordered) or numbers (ordered)
- **Links** — rendered as `text (url)` in a link color
- **Horizontal rules** — full-width `─` line
- **Paragraphs** — word-wrapped to terminal width, blank line separation

Out of scope for prototype: tables, images, footnotes, HTML blocks, syntax highlighting.

## CLI Interface

### Usage

```
mdx [OPTIONS] [FILE]
mdx README.md          # render file
cat README.md | mdx    # render stdin
mdx                    # no args, no stdin -> print help
```

### Options

- `-p, --pager` — force pager mode even when piped
- `--no-pager` — force plain output even on TTY
- `-w, --width <N>` — override wrap width (default: terminal width)

Arg parsing with `clap`.

## Error Handling

- File not found / unreadable -> clear error message to stderr, exit 1
- Malformed mermaid block -> render as plain code block with warning comment (don't crash)
- Terminal too narrow for diagram -> render what fits, truncate with `...`

No logging or verbose mode for the prototype.

## Key Dependencies

- `pulldown-cmark` — markdown parsing
- `ratatui` + `crossterm` — TUI pager
- `clap` — CLI argument parsing
- `layout-rs` (or similar) — graph layout for mermaid diagrams
