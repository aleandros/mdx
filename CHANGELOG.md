# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.21] - 2026-10-26

### Added
- Switch Mermaid diagrams between image and ASCII in the pager. With graphics
  on, `i` switches the selected diagram between its image and its ASCII form,
  which stays available because its text can be selected and copied. Every
  diagram with an image can now be selected with `Tab` or by scrolling to it,
  and the blank row above the selected diagram (below it when the row above
  is scrolled away) names its form (`[image — i for ASCII]`,
  `[ASCII — i for image]`, `image loading`, `image unavailable`). `Enter`
  still only expands and collapses large diagrams.
  Both forms take the same rows, so switching does not scroll. Collapsed
  diagrams stay collapsed and show the chosen form when expanded. The choice
  is kept per diagram across scrolling, resizing and watch-mode reloads.
  Without graphics, `i` does nothing and no hint is shown. (#18)
- `--mermaid-graphics on|off` and `mermaid_graphics = "on" | "off"` in the
  config choose whether diagrams start as images (default) or ASCII when
  graphics are on; the flag wins over the config.

### Changed
- When a search match (`/`, `?`, `n`, `N`) lands inside a diagram shown as an
  image, that diagram switches to ASCII so the match is visible.

## [0.1.20] - 2026-10-26

### Added
- Graphics inside tmux. With `set -g allow-passthrough on` (tmux 3.3+),
  `auto` now turns graphics on in a tmux pane when tmux sends 24-bit color to
  the terminal outside it (the `RGB` feature, or `setrgbf`/`setrgbb` in its
  terminal description), that terminal is kitty 0.28+ (from tmux's
  `client_termtype`), the pane is the client's active pane (so the reply
  cannot reach another program), and the Kitty graphics query sent through
  passthrough is answered. Image transmissions, placements and deletions are
  wrapped in tmux passthrough, one sequence (and so one 4096-byte chunk) per
  passthrough; the placeholder cells stay ordinary text that tmux redraws, so
  inline images and diagrams scroll with the pane and come back after
  switching windows. Otherwise graphics stay off, with the reason. (#17)
- `--debug-graphics` shows the tmux version, the pane's `allow-passthrough`,
  whether tmux sends 24-bit color and whether the pane is active, and names
  the terminal outside tmux. A `TMUX` variable inherited by a terminal outside
  tmux is ignored.

## [0.1.19] - 2026-10-26

### Added
- Mermaid diagrams as images in the pager. With graphics on (#14), flowcharts,
  ER diagrams and sequence diagrams are drawn by a new SVG backend over the
  existing layouts, rasterized with `resvg`, and shown with Kitty Unicode
  placeholders (#16) in exactly the cell rectangle of their ASCII form: real
  node shapes, polyline edges with arrowheads, crow's-foot cardinality marks,
  lifelines, activations, notes and fragments, in the UI theme's colors.
  Label text is fitted to the cells the layout reserved for it, so labels
  never overflow their shapes. Labels use a bundled DejaVu Sans Mono font,
  with system fonts for missing glyphs. Diagrams rasterize on the image thread
  and are cached by source, width, theme and cell size; the ASCII diagram
  shows until the image is ready and whenever it cannot be drawn. Plain and
  piped output, `mdx embed`, `--graphics off` and `NO_COLOR` are unchanged.
  (#10)

## [0.1.18] - 2026-10-26

### Added
- Inline images in the pager. With graphics on (#14), local PNG, JPEG and GIF
  images are drawn with Kitty Unicode placeholders, so they scroll, search and
  redraw with the text. Paths are relative to the markdown file's directory, or
  absolute. Images are scaled down to the content column and to 60% of the
  pager height, keeping the aspect ratio, and refit on resize. Decoding runs on
  a background thread. The alt text stays as a caption below the image. Watch
  mode checks image files once a second and re-sends an image only when its
  mtime changes, and images are deleted from the terminal on exit. Remote
  URLs, missing files, other formats, `--graphics off`, `NO_COLOR`, piped
  output and `mdx embed` keep the text placeholder. (#16)

### Changed
- `Enter` on an image opens local paths relative to the markdown file's
  directory instead of the current directory.

## [0.1.17] - 2026-10-26

### Added
- Terminal graphics detection, decided once at startup: `--graphics auto|off`,
  `MDX_GRAPHICS=off` and `graphics = "auto" | "off"` in the config (CLI wins,
  then the environment, then the config). `auto` enables graphics only in the
  interactive pager on a terminal that answers a Kitty graphics query, supports
  Unicode placeholders (kitty 0.28+) and is not inside tmux. The cell size in
  pixels is recorded too. Piped output and `mdx embed` never send a query.
  Rendering is unchanged. (#14)
- `--debug-graphics` prints the graphics decision and the reason.
- An invalid `MDX_GRAPHICS` value is ignored with a warning.

## [0.1.16] - 2026-10-26

### Changed
- Upgrade `ratatui` to 0.30 and `crossterm` to 0.29. This unblocks
  `ratatui-image` for image placement. (#15)

### Fixed
- Halfwidth katakana with voiced or semi-voiced sound marks (`ｶﾞ`, `ﾊﾟ`) no
  longer garble the pager. Each mark now takes its own column, so the
  following text is no longer shifted or overwritten.

## [0.1.15] - 2026-10-26

### Changed
- Default UI theme is now `nord`, and the default syntax theme is now `Nord`.
  Several `clay` colors (H2, H6, links, inline code, rules) fell below WCAG
  contrast targets on dark terminals. The `nord` theme is retuned to the
  official palette: against nord0 `#2E3440`, text roles reach at least 4.5:1
  and rules and diagram strokes at least 3:1. It no longer dims inline code or
  horizontal rules, and mermaid `style`/`classDef` colors snap to these colors
  too. The previous look is still available by name:
  `--ui-theme clay --theme base16-ocean.dark`, or `ui_theme`/`theme` in
  `config.toml`. Its output is unchanged. If you set only `ui_theme` (for example
  a light theme), also set `theme = "base16-ocean.dark"` to keep the previous
  code-block colors. (#20)

### Added
- Bundled `Nord` syntax theme (`Nord.tmTheme` from nordtheme/sublime-text, MIT),
  listed by `--theme=list`. Its comment color is lightened from `#616e88` to
  `#7b88a1` to reach 3:1 contrast.
- Optional `dim_muted` key for custom UI theme files (default `true`), which
  controls whether inline code and horizontal rules are dimmed.

## [0.1.14] - 2026-10-26

### Fixed
- Preserve inline and display math as raw source with dollar delimiters in the
  pager, stdout and `mdx embed`. Inline math uses inline-code styling; display
  math occupies a separate block without syntax highlighting. (#13)
- Preserve raw HTML and footnote references instead of silently ignoring their
  text-carrying parser events.

## [0.1.13] - 2026-10-26

### Added
- Shared shrinking generators and property tests for Markdown parsing, styled
  wrapping/clipping, flowchart/sequence/ER layout, and pager flattening.
- Pager operation model covering navigation, search, activation, resize and reload;
  reproducible regression seeds and `PROPTEST_CASES` stress runs.

### Fixed
- Preserve inline/display math and table cell text instead of silently dropping it.
- Wrap prose on terminals narrower than four columns and retain empty rendered
  blocks in pager flattening.
- Keep the selected interactive block on resize when it remains interactive.
- Prevent parallel-edge and self-loop labels from overwriting one another or
  node text; reserve space around ER cardinality markers.

## [0.1.12] - 2026-10-01

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
