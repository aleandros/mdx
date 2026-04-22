# Embed mode design

**Date:** 2026-04-22
**Status:** Approved (awaiting implementation plan)

## Problem

`mdx` today is a terminal-first tool: on a TTY it opens an interactive pager, off a TTY it dumps plain ANSI. Callers that want to embed rendered markdown inside their own UI — another TUI, an editor plugin, a dashboard widget — have to pick the non-pager path and then patch around the fact that:

- There is no `--height`; output runs forever and the host has to re-paginate.
- There is no single CLI surface that promises "no pager, no alt-screen, always colored, no extra chrome." Callers have to remember a specific flag combination and trust that future flags will stay safe.
- Mermaid diagrams render at their natural size and can overflow the caller's widget.

The goal is a stable, discoverable CLI surface that other programs can shell out to and render the result into a fixed box.

## Non-goals

- Reflowing mermaid layouts to fit a target width/height. For v1 diagrams render at natural size and get **hard-cropped** at the declared width/height (see Q2 in the brainstorm transcript).
- JSON or structured output. v1 emits the same styled ANSI the existing pipe path produces.
- Streaming / incremental output. The caller gets the full rendered stream at once.
- A library / C FFI surface. This is a CLI contract only.

## Surface

New subcommand:

```
mdx embed [FILE] [OPTIONS]
```

Flags:

| Flag | Type | Default | Notes |
|---|---|---|---|
| `FILE` | positional, optional | stdin | Same semantics as top-level `mdx` |
| `-w, --width <N>` | `u16` | terminal width when attached, else 80 | Drives text wrapping *and* hard-crops overflowing lines |
| `--height <N>` | `usize` | unlimited | Cap on emitted line count |
| `--theme <NAME>` | string | current default | Accepts `list` |
| `--ui-theme <NAME>` | string | current default | Accepts `list` |
| `--no-mermaid-rendering` | bool | false | Same as top-level |
| `--split-mermaid-rendering` | bool | false | Same as top-level |

Flags intentionally **not** exposed on `embed`:

- `--pager` / `--no-pager` — embed is always non-interactive
- `--watch` — embed is a one-shot render

Exit codes:
- `0` on success
- non-zero with message on stderr for parse errors, read errors, unknown themes

## Output contract

The guarantee callers can rely on. These are API-level promises; regressions here are breaking changes.

1. **No pager, no alt-screen, no raw mode.** `embed` never installs the crossterm panic hook, never calls `EnterAlternateScreen`, never touches terminal modes.
2. **Always ANSI color unless suppressed.** Color is emitted regardless of whether stdout is a TTY. `NO_COLOR=1` in the environment strips color (same rule the existing `pipe_output` uses).
3. **Line-based, UTF-8.** Every emitted line ends with `\n`. When height truncation cuts mid-stream, the last emitted line still ends with `\n` (no special final-line handling). Callers can split on `\n` safely.
4. **No ANSI control sequences other than SGR.** Only color/style escape codes appear in output — no cursor movement, no screen clears, no bracketed-paste toggles.
5. **Deterministic sizing.**
   - Each output line has display width ≤ `--width` (measured with `unicode-width`, not byte count).
   - Total lines ≤ `--height` when provided.
   - Width truncation preserves styled spans across the cut point.
6. **No chrome changes vs. pipe mode.** Images still render as `[Image: alt](url)` placeholders (same as `pipe_output` today). Diagrams still get a trailing blank line.

## Truncation semantics

Post-render, single pass:

1. Flatten `Vec<RenderedBlock>` into `Vec<StyledLine>` preserving the order `pipe_output` uses:
   - `Lines(v)` → each styled line in `v`, in order
   - `Diagram { lines, .. }` → each styled line in `lines` in order, followed by one empty styled line
   - `Image { alt, url }` → one plain styled line: `[Image: alt](url)` or `[Image](url)` when alt is empty
2. Width pass: for each line, walk spans left-to-right accumulating `unicode_width::UnicodeWidthStr`. At the first span that crosses `max_cols`, cut mid-string on a char boundary; drop remaining spans. Lines shorter than `max_cols` pass through untouched.
3. Height pass: if `--height N` is set, take the first `N` lines; the rest are dropped.
4. Encode each surviving `StyledLine` with the existing `styled_line_to_ansi(line, no_color)` and write with a trailing `\n`.

Edge cases:

- **CJK / double-width glyph straddling the width boundary:** cut before the glyph, not through it. The resulting line may have display width `max_cols - 1`.
- **Zero-width joiner / combining marks:** treated as width 0 by `unicode-width`; they stay attached to the preceding base char.
- **`--width 0` or `--height 0`:** both produce empty output (zero lines written). No error.
- **Empty input:** writes nothing. Exit 0.
- **Diagram naturally wider than width:** cropped. No scaling attempt, no ellipsis marker. This is Q2 answer (a) from the brainstorm transcript.

## Code organization

New module `src/embed.rs`:

```rust
pub struct EmbedOptions {
    pub width: u16,
    pub height: Option<usize>,
    pub no_color: bool,
}

pub fn run(
    input: &str,
    opts: EmbedOptions,
    highlighter: &Highlighter,
    ui_theme: &Theme,
    mermaid_mode: MermaidMode,
) -> anyhow::Result<()>;

fn flatten_blocks(blocks: &[RenderedBlock]) -> Vec<StyledLine>;
fn truncate_line_width(line: &StyledLine, max_cols: usize) -> StyledLine;
```

All three functions are pure (the `run` impl is the only one doing I/O) — unit tests target `flatten_blocks` and `truncate_line_width` directly.

`src/main.rs` changes:

- Add `Commands::Embed(EmbedArgs)` variant with its own `clap::Args` struct containing only the seven fields from the Surface table. No pager / watch / no_pager fields.
- Dispatch branch in `main()` — after `Update` handling, before the existing default path.
- Extract the small helper bits (mermaid mode selection, theme resolution, `--theme=list` / `--ui-theme=list` short-circuits) into private helpers so `embed` and the default path share one implementation.

`Cargo.toml`: add `unicode-width = "0.1"`.

## Testing

Following `superpowers:test-driven-development`, tests land before implementation.

Unit tests (`src/embed.rs`):

- `flatten_blocks`:
  - `Lines` → expected line count, preserves styles
  - `Diagram` → diagram lines plus exactly one trailing empty line
  - `Image` with alt → `[Image: alt](url)` line
  - `Image` no alt → `[Image](url)` line
  - Mixed sequence preserves order
- `truncate_line_width`:
  - ASCII, width exactly at boundary → unchanged
  - ASCII, width below boundary → cut at boundary, style of cut span preserved on the surviving prefix
  - CJK double-width straddling the cut → cut falls before the wide char, resulting display width `max_cols - 1`
  - Emoji with ZWJ sequence crossing boundary → split on char boundary, not inside a cluster
  - Empty line → empty line
  - `max_cols = 0` → empty line

Integration tests (`tests/`):

- `mdx embed --width 40 --height 5 fixture.md` → exactly 5 lines on stdout, each display-width ≤ 40
- `mdx embed` with no stdin, no file → exit non-zero, message on stderr (mirrors default path)
- `mdx embed --width 40 fixture-with-diagram.md` with a diagram wider than 40 → diagram lines are cropped, non-diagram text wraps normally
- `NO_COLOR=1 mdx embed fixture.md` → stdout contains no ESC bytes
- `mdx embed --theme list` → lists themes, exits 0
- `mdx embed --width 40 --pager fixture.md` → clap rejects (`--pager` not on subcommand)

## Risks / open questions

- **`unicode-width` dependency:** adds a small crate (~kB of code, no transitive deps). Acceptable.
- **Diagram cropping is lossy:** a very wide flowchart becomes unreadable when the caller's widget is narrow. Documented as a known limitation; future work could teach the layout engine to reflow (Q2 answer (b)).
- **Image placeholder text wider than `--width`:** will be cropped by the width pass. For narrow widgets the URL will be truncated. Acceptable — the alt text alone usually conveys enough.
- **Stability:** the "output contract" section is the public API. Any change to items 1-6 is a breaking change and belongs in `CHANGELOG.md`.
