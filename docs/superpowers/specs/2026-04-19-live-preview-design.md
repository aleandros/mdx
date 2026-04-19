# Live Preview Mode — Design Spec

## Overview

Add a `--watch` / `-W` flag to `mdx` that watches a file for changes and
re-renders it in the pager efficiently, preserving viewport state and handling
parse errors gracefully.

## CLI Interface

**New flag:** `--watch` / `-W` (bool, default `false`)

**Constraints:**
- Requires a file argument. Error if used with stdin.
- Implies pager mode. `--no-pager` + `--watch` is a conflict error.

**Status bar:** Thin bar at the bottom of the pager showing:
- Filename and "watching" indicator
- Brief "updated" flash on successful re-render
- "mermaid error at block N" when a mermaid block fails to parse

## File Watching

**Crate:** `notify` v7 with `PollWatcher` fallback for network mounts / FUSE.

**Setup:** The watcher runs on its own background thread (notify's default). It
watches the single file path from the CLI and sends `WatchEvent::FileChanged`
through an `std::sync::mpsc::channel` to the pager loop.

**Debouncing:** Time-based, 100ms window. On receiving a file change event, the
pager loop records `last_event_time`. It only acts once 100ms have elapsed since
the last event, coalescing rapid multi-event editor saves.

**Content dedup:** After the debounce fires, the file is read and hashed (e.g.
`DefaultHasher` / FNV). If the hash matches the last processed version, the
re-render is skipped. Catches editors that touch files without changing content.

**Error resilience:**
- File temporarily unreadable during atomic save: retry once after 50ms, then
  skip the event.
- File deleted: show "file removed, waiting..." in status bar, keep last render,
  keep watching for re-creation.
- Watcher errors: log to status bar, fall back to polling if native watcher
  fails.

## Block-Level Diffing

**Identity:** Structural equality on `Block` and `InlineElement` via derived
`PartialEq` / `Eq` / `Hash` traits.

**Algorithm:** Compare old `Vec<Block>` vs new `Vec<Block>` by position:
- **Unchanged** — reuse cached `RenderedBlock`.
- **Changed** — re-render only this block.
- **Inserted / Removed** — if block count changed, re-render from first
  difference onward.

**Mermaid diagram cache:**
- `HashMap<String, RenderedMermaid>` keyed by raw mermaid source string.
- Unchanged source → reuse cached diagram (skips the expensive render).
- Changed source, render succeeds → update cache.
- Changed source, render fails → keep last good diagram, show error overlay.

## Viewport Stability

After diffing and re-rendering:
- Rebuild `flat_lines` in the pager.
- Preserve `scroll` position — clamp to new max if document shortened.
- Preserve `expanded: HashSet<usize>` for diagram collapse state — remap indices
  if blocks were inserted/removed above the expanded diagrams.
- If a block above the viewport changed height, adjust scroll to anchor on the
  first visible block, keeping the same content in view.

## Error Handling (Hybrid)

**Markdown parsing:** `pulldown_cmark` is very permissive and almost always
produces a valid partial render, even for broken input. Always show whatever it
produces — no "last good render" fallback needed at this layer.

**Mermaid blocks:** Cache the last successful render per block. If re-rendering
fails after a source change, display the cached diagram with an error overlay
(red label). When the source becomes valid again, update normally.

**Net effect:** The user sees continuous partial renders while editing. Mermaid
diagrams remain stable during mid-edit broken states.

## Pager Loop (Watch Mode)

```
pending_change = false
last_event_time = None

loop {
    // 1. Keyboard/mouse (non-blocking, 50ms timeout)
    if crossterm::event::poll(50ms) {
        event = crossterm::event::read()
        handle input (scroll, quit, toggle diagram, etc.)
    }

    // 2. Drain file change events
    while channel.try_recv() == Ok(FileChanged) {
        last_event_time = Some(Instant::now())
        pending_change = true
    }

    // 3. Debounce check
    if pending_change && last_event_time.elapsed() >= 100ms {
        pending_change = false
        content = read_file()
        if hash(content) != last_hash {
            new_blocks = parse_markdown(content)
            rendered = diff_and_render(old_blocks, new_blocks, mermaid_cache)
            update pager state (preserve scroll, expanded, anchor viewport)
            last_hash = hash(content)
            redraw = true
        }
    }

    // 4. Draw if needed
    if redraw { draw frame }
}
```

The non-watch pager is unaffected — it keeps its existing blocking
`crossterm::event::read()` loop with no behavioral or performance change.

**Quit:** `q` / `Esc` exits the pager and stops the watcher. Clean shutdown:
drop the watcher, restore terminal state.

## Module Structure

### `watch.rs` (new)
Owns all watch-specific logic:
- File watcher setup (notify config, polling fallback)
- Debounce logic and content hashing
- `diff_and_render()` — takes old blocks, new blocks, and mermaid cache; returns
  updated `Vec<RenderedBlock>`
- Mermaid diagram cache (`HashMap<String, RenderedMermaid>`)
- Watch-mode pager loop (`run_watch()`)
- Status bar rendering

### `main.rs`
- Add `--watch` / `-W` field to `Args` struct
- Validation: `--watch` requires file, conflicts with `--no-pager`
- Dispatch: watch mode calls `watch::run_watch()`, normal mode calls
  `pager::run_pager()` or `pipe_output()` as before

### `parser.rs`
- Derive `PartialEq`, `Eq`, `Hash` on `Block` and `InlineElement` enums

### `render.rs`
- Derive `PartialEq` on `RenderedBlock` and related types (for cache
  validation, not strictly required but useful)

### `pager.rs`
- Make `PagerState`, `FlatLine`, and drawing helpers `pub(crate)` so
  `watch.rs` can reuse them
- No logic changes

## Dependencies

```toml
# Cargo.toml
notify = { version = "7", features = ["macos_fsevent"] }
```

No async runtime. No other new dependencies.
