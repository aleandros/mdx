# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Flowchart cycles among free nodes (`A --> B --> C --> A`, retry loops,
  self-loops) collapsed every node of the cycle onto one rank: a flat row
  with no arrowheads and dangling stubs. Cycles are now broken by declaration
  order, so the chain renders top to bottom and the closing edge is drawn as
  a back edge around the side of the diagram, ending in an arrowhead. Cycles
  between two or more named subgraphs still share a rank so the subgraphs stay
  side by side. (#1)
- Edges that skip ranks were drawn straight through the nodes in between.
  Long edges are now split at every skipped rank with a reserved routing
  column (TD/BT) or row (LR/RL), bends live in lanes inside the inter-rank
  gap, and same-rank edges and self-loops get their own lane. Edge labels are
  placed by the layout in free gap space instead of on top of node borders.
  (#2)
- Node, subgraph, edge, entity and participant sizes used byte length, so
  CJK, accented and emoji labels produced oversized boxes with off-centre
  text. All diagram geometry now uses terminal display width, and
  double-width characters occupy two canvas cells. The ER comment wrapper
  also no longer panics on a long non-ASCII word. (#3)
- Arrowheads in `BT` and `RL` flowcharts were drawn one cell short of the
  target node.
- Edge labels written inside the operator (`A -- text --> B`,
  `A -. text .-> B`, `A == text ==> B`) were dropped, and the `--`/`==` forms
  silently ended parsing of the line, losing the following nodes.
- A subgraph around rank-0 nodes in `TD` had its top border clipped at the
  first row.

### Changed
- Nodes inside a subgraph are ranked by longest path over the subgraph's
  edges, like free nodes, instead of by declaration order; unconnected
  members sit side by side rather than in a chain.
- Long edges between different subgraphs (or a subgraph and a free node)
  route through a dedicated band outside every subgraph box.

## [0.1.11] - 2026-10-01

### Added
- Pager margins on wide terminals: the content column is capped at 100
  columns and centered, with equal gutters on both sides. Configure with
  `--max-width <N>` or `max_width` in the config file; `0` disables the cap.
  Diagrams that fit the column share its margin; wider diagrams are centered
  on the terminal when they fit it and flush-left otherwise.
- Headless pager tests render `PagerState` into a ratatui `TestBackend`
  buffer, plus a fixture sweep that checks every example document fits the
  content column at 40/80/120/200 columns.

### Fixed
- The large-diagram collapse heuristic measured line length in bytes, so any
  box-drawn diagram wider than about 53 columns (160 bytes of 3-byte
  box-drawing characters) was collapsed on an 80-column terminal even though
  it fit on screen. It now measures display columns.
- Word wrap treated the leading indent as a break point: an indented line
  with no other spaces (e.g. a dense code line) produced a near-empty first
  row and a continuation row wider than the terminal.

## [0.1.10] - 2026-04-29

### Added
- Pager word-wraps long paragraphs and list items at terminal width so
  reading no longer requires horizontal scrolling. Span styles and the
  original leading indent are preserved on continuation lines; ASCII
  diagrams remain rigid (h-scroll still works for wide diagrams).
- Collapsed mermaid indicators now name the actual diagram type (ER
  Diagram / Sequence Diagram / Flowchart) with kind-specific count nouns
  (e.g. `[ER Diagram: 4 entities, 3 relationships — Enter to expand]`).

### Changed
- Diagrams (collapsed or expanded) now have a blank line above and below
  in the pager so they stand out from neighboring text.
- Non-highlighted code blocks render in `theme.body` (and the `[lang]`
  label in `theme.horizontal_rule`) instead of dim DarkGray, fixing
  unreadable contrast on dark-background themes.

## [0.1.9] - 2026-04-27

### Added
- ER diagrams now use theme colors by default (matching flowchart and
  sequence diagrams).
- Per-entity styling via `style`, `classDef`, and `class` directives,
  identical to flowchart syntax.

## [0.1.8] - 2026-04-25

### Added
- Mermaid `erDiagram` support: entities with typed attributes, PK/FK markers, wrapped comments, ASCII crow's foot cardinality, identifying and non-identifying relationships, optional `direction` extension

## [0.1.7] - 2026-04-25

### Added
- Mermaid compound graph layout: subgraph members now occupy contiguous rank bands so bounding boxes are compact and non-overlapping
- Cyclic subgraphs (bidirectional inter-cluster edges) are detected via SCC and stacked in separate vertical bands, giving a clear top-stack arrangement for tightly-coupled clusters
- Declaration-order-based internal rank assignment within each cluster, correctly handling retry back-edges without disrupting the intended visual flow

### Fixed
- Subgraph nodes declared after edges in the mermaid source are now correctly assigned to subgraph membership (parser fix)
- Arrows crossing subgraph box walls now have breathing room (`H_PAD` 2 → 3), replacing `│►│` with `│─►│`
- Upward-going edges in LR diagrams use a short horizontal step before the vertical, avoiding the edge running along the box bottom border

## [0.1.6] - 2026-04-23

### Fixed
- Pager: ghost characters on the right edge when scrolling through code blocks containing tabs. Ratatui measured `\t` as one cell while the terminal advanced to the next tab stop, leaving uncleared cells across frames. Tabs are now expanded to spaces before reaching ratatui.
- Syntax highlighting: `//` line comments (and other end-of-line scopes) bled into every subsequent line. `highlight_line` needs trailing newlines to close line-scoped patterns; switched to `syntect::util::LinesWithEndings`.

## [0.1.5] - 2026-04-23

### Added
- Config file support: `--config` flag, layered loading (CLI > env > file > defaults), and `mdx init` subcommand to scaffold a default TOML config
- `mdx embed` subcommand: non-interactive rendering with width/height truncation and unicode-aware line clipping for embedding output in other tools
- `mdx preview-themes` subcommand to render a preview of all bundled syntax themes
- Nine new UI themes: frost, nord, glacier, steel, solarized-dark, solarized-light, paper, snow, latte
- `Theme::all()` enumeration API
- Mermaid color support: hex and CSS named color parsing, `style`/`classDef`/`class`/`linkStyle` directive parsing, styled sequence diagrams, per-cell `SpanStyle` canvas, theme palette extension with nearest-color resolution
- Vim-style search and scroll keybindings in the pager
- Publishing to crates.io on release; crate renamed to `mermd` to claim the name
- README logo and status badges

### Fixed
- `preview-themes` now reuses `pipe_output` and respects `NO_COLOR` in theme headers
- `generate_default` uses single `#` for description comments so output parses as valid TOML
- Mermaid rendering: removed dead annotations, deduplicated helpers, and now recurses into sequence fragments

## [0.1.4] - 2026-04-21

### Added
- Self-update command: `mdx update` checks GitHub for the latest release and replaces the binary in-place
- Watch mode (`--watch` / `-W`): live-preview that re-renders on file save with block-level diffing and mermaid diagram caching
- File watcher with polling fallback and content hashing for reliable change detection
- Horizontal scrolling for wide diagrams (left/right arrow keys, Home/End)
- Active block indicator showing which collapsible diagram is selected
- Automatic collapse for diagrams that exceed terminal width
- Watch mode status bar with file path, change count, and last-updated timestamp
- Integration tests for watch mode CLI validation

### Fixed
- Terminal cleanup on all pager exit paths (no more raw-mode leaks)
- Rust 1.95.0 toolchain pinned to prevent clippy drift between local and CI
- CI workflow passes explicit toolchain version
- Mermaid cache keyed by block position for correct mid-edit fallback
- Page scroll uses live terminal height after resize
- Debug assertion and cache fallback for block-level diff rendering

## [0.1.3] - 2026-04-18

### Added
- UI theming with two built-in themes: clay (default) and hearth (`--ui-theme` flag)
- Mermaid rendering modes: `--no-mermaid-rendering` and `--split-mermaid-rendering` flags
- Image support with Tab-to-open in pager mode via xdg-open/open
- Bundled syntax grammars compiled at build time via build.rs packdump
- Additional grammars: TSX, HCL, SCSS, Vue, Svelte
- Default syntax theme set to base16-ocean.dark with RGB colors
- Snapshot integration tests with insta for all example files
- Pre-commit hook (cargo fmt + clippy) and pre-push hook (cargo test)
- Auto-tagging CI workflow on Cargo.toml version change
- MIT license

## [0.1.2] - 2026-04-15

### Added
- Syntax highlighting via syntect with `--theme` flag
- `Color::Rgb` variant for 24-bit true color support
- `--theme=list` to show available syntax themes
- Integration tests for syntax highlighting

### Fixed
- Validate theme names and return helpful errors for invalid themes

## [0.1.1] - 2026-04-15

### Added
- Mermaid sequence diagram rendering (participants, messages, activations, notes, fragments)
- Autonumber support for sequence diagrams
- 14 sequence diagram test fixtures

### Fixed
- Sequence diagram rendering for arrows, notes, activations, and fragments

## [0.1.0] - 2026-04-15

### Added
- Terminal markdown rendering with pulldown-cmark
- Interactive pager with ratatui (j/k/arrows scroll, mouse support, q to exit)
- Mermaid flowchart rendering as ASCII art (graph TD/LR/BT/RL)
- All node shapes: rect, rounded, diamond, circle
- All edge styles: arrow, plain, dotted, thick with labels
- `--width` flag for custom terminal width
- `--pager` / `--no-pager` flags for output mode control
- `NO_COLOR` environment variable support
- Large diagram collapse/expand with Tab key
- Graceful terminal restore on panic
- CI pipeline (check, test, clippy, fmt)
- Cross-platform release builds (x86_64/aarch64 Linux and macOS)
