# Ghostty graphics verification

The verified Ghostty minimum is **1.3.1**, outside tmux. This is the lowest
version tested with mdx, not a claim about when Ghostty first implemented the
protocol. Earlier releases, including 1.3.0, stay off until separately verified.

## User verification

The user tested the worktree build on macOS in Ghostty 1.3.1, outside tmux,
with 16×34-pixel cells, and supplied this output:

```text
graphics: on
reason: terminal supports Kitty graphics with Unicode placeholders
setting: auto (from --graphics)
query: OK
terminal: ghostty 1.3.1
cell size: 16x34 px
tmux: no
```

The user received the fixture command and the scrolling, resizing and
top/bottom clipping checklist below, reported that the rendering looked
excellent, and subsequently accepted the visual result: “i think it looks
fine.” This is user-performed visual acceptance; the agent did not independently
observe the terminal. The patch-aware minimum keeps that acceptance scoped to
the version actually tested.

## Upstream evidence

- Ghostty [v1.0.0's Unicode graphics implementation](https://github.com/ghostty-org/ghostty/blob/v1.0.0/src/terminal/kitty/graphics_unicode.zig)
  recognizes U+10EEEE, decodes the low 24 image-ID bits from foreground color,
  and reads row, column and optional high image-ID bits from combining
  diacritics. Its placement iterator groups visible cell runs, and
  `renderPlacement` computes the corresponding source rectangle at the current
  cell size. Its tests cover runs starting in the middle, high image-ID bits,
  scaled placements and blank padding cells.
- The same protocol machinery is present in
  [v1.3.1](https://github.com/ghostty-org/ghostty/blob/v1.3.1/src/terminal/kitty/graphics_unicode.zig).
- Ghostty [v1.0.0's `reportXtversion`](https://github.com/ghostty-org/ghostty/blob/v1.0.0/src/termio/stream_handler.zig#L948)
  sends `DCS >|ghostty <version> ST`, unlike kitty's `name(version)` form.
- mdx's `src/kitty.rs` transmits PNG data, creates a `U=1` virtual placement,
  and writes U+10EEEE with explicit row/column diacritics and the image ID in
  the foreground color. Both local images and rasterized Mermaid use this path.

These source findings explain protocol compatibility; they do not establish
live mdx verification on 1.0.0 or replace the user test on 1.3.1. Ghostty's own
test suite was not run as part of this change.

## Repeatable visual checklist

Run from this checkout in Ghostty, outside tmux, without `NO_COLOR` or
`MDX_GRAPHICS=off`:

```sh
cargo build
./target/debug/mdx --graphics auto --debug-graphics
./target/debug/mdx --graphics auto --mermaid-graphics on docs/examples/graphics.md
```

Record Ghostty version, OS, cell size, and the result of each check when
verifying another version. Lower the allowlist minimum only after that version
passes these checks with mdx.

- Debug report says `graphics: on`, `query: OK`, `tmux: no`.
- Local PNG shows pixels, not placeholder glyphs or alt text.
- Mermaid flowchart, sequence and ER diagrams show images.
- `j`/`k` and page scrolling move images with surrounding text.
- At the top, clipped images show the correct lower source rows.
- At the bottom, clipped images show the correct upper source rows.
- Resizing narrower/wider and shorter/taller redraws without stale pixels.
- Scrolling away and back restores the same images.
- `i` switches Mermaid between image and ASCII, then back.

The top and bottom checks apply to both the local PNG and each Mermaid type.
The pager status line and adjacent text must remain intact. Exercise clipping
again after resizing, including a window shorter than an image.

The automated detection tests cover both XTVERSION formats, exact minimum
versions, older-version diagnostics, unknown terminals, and the kitty-only
tmux gate. Ghostty inside tmux, WezTerm and Konsole are separate follow-ups.
