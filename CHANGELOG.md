# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- UI theming with two built-in themes: clay (default) and hearth (`--ui-theme` flag)
- Mermaid rendering modes: `--no-mermaid-rendering` and `--split-mermaid-rendering` flags
- Image support with Tab-to-open in pager mode via xdg-open/open
- Snapshot integration tests with insta for all example files
- Pre-commit hook (cargo fmt + clippy) and pre-push hook (cargo test)
- Auto-tagging CI workflow on version bump

## [0.1.0] - 2026-04-17

### Added
- Terminal markdown rendering with pulldown-cmark
- Interactive pager with ratatui (j/k/arrows scroll, mouse support, q to exit)
- Mermaid flowchart rendering as ASCII art (graph TD/LR/BT/RL)
- All node shapes: rect, rounded, diamond, circle
- All edge styles: arrow, plain, dotted, thick with labels
- Mermaid sequence diagram rendering (participants, messages, activations, notes, fragments)
- Syntax highlighting via syntect with `--theme` flag
- Bundled syntax grammars: TOML, Bash, Dockerfile, Kotlin, Swift, Zig, Terraform, TypeScript, TSX, Svelte, Vue, SCSS, HCL
- `--width` flag for custom terminal width
- `--pager` / `--no-pager` flags for output mode control
- `NO_COLOR` environment variable support
- Large diagram collapse/expand with Tab key
- Graceful terminal restore on panic
- CI pipeline (check, test, clippy, fmt)
- Cross-platform release builds (x86_64/aarch64 Linux and macOS)
