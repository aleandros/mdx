# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
