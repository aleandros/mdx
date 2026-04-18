# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
