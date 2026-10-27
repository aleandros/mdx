# Ghostty graphics verification

The proposed Ghostty minimum is **1.0**, the first stable release. Source
inspection supports this minimum; **live mdx rendering has not yet been
verified**. Do not treat unit tests or the successful graphics query as a
substitute for the visual checks below.

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

These are source findings, not results from running Ghostty's own test suite.

## Live checklist

Run from this checkout in Ghostty, outside tmux, without `NO_COLOR` or
`MDX_GRAPHICS=off`:

```sh
cargo build
./target/debug/mdx --graphics auto --debug-graphics
./target/debug/mdx --graphics auto --mermaid-graphics on docs/examples/graphics.md
```

Record Ghostty version, OS, cell size, and the result of each check. Repeat on
1.0.0 and 1.3.1 before accepting 1.0 as the verified minimum. If an older version
fails, find the first working release and adjust the allowlist, tests and docs.

| Check | Ghostty 1.0.0 | Ghostty 1.3.1 |
| --- | --- | --- |
| Debug report says `graphics: on`, `query: OK`, `tmux: no` | Pending | Pending |
| Local PNG shows pixels, not placeholder glyphs or alt text | Pending | Pending |
| Mermaid flowchart, sequence and ER diagrams show images | Pending | Pending |
| `j`/`k` and page scrolling move images with surrounding text | Pending | Pending |
| At the top, clipped images show the correct lower source rows | Pending | Pending |
| At the bottom, clipped images show the correct upper source rows | Pending | Pending |
| Resizing narrower/wider and shorter/taller redraws without stale pixels | Pending | Pending |
| Scrolling away and back restores the same images | Pending | Pending |
| `i` switches Mermaid between image and ASCII, then back | Pending | Pending |

The top and bottom checks apply to both the local PNG and each Mermaid type.
The pager status line and adjacent text must remain intact. Exercise clipping
again after resizing, including a window shorter than an image.

The automated detection tests cover both XTVERSION formats, exact minimum
versions, older-version diagnostics, unknown terminals, and the kitty-only
tmux gate. Ghostty inside tmux, WezTerm and Konsole are separate follow-ups.

## Current limitation

On the task machine, the installed CLI reports Ghostty 1.3.1, but the computer-use
tool denies access to `com.mitchellh.ghostty` for safety reasons. No live visual
result is claimed. The PR must remain unmerged until the matrix is completed.
