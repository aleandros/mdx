# Sequence Diagram Support — Design Spec

## Overview

Add Mermaid sequence diagram rendering to mdx. Separate pipeline from flowcharts (own parser, layout, renderer) sharing only the Canvas primitive. Full Mermaid sequence syntax support built incrementally.

## Scope

### Supported Syntax

**Participants:**
- `participant A as Alice` — explicit declaration with alias
- `actor A as Alice` — parsed, rendered as box (same as participant)
- Implicit creation from first usage in a message

**Messages (all 6 arrow styles):**

| Syntax | Style |
|--------|-------|
| `->>` | Solid with arrowhead |
| `-->>` | Dashed with arrowhead |
| `->` | Solid, open arrow |
| `-->` | Dashed, open arrow |
| `-x` | Solid with cross |
| `--x` | Dashed with cross |

**Self-messages:** `A->>A: text` — rendered as loopback arrow.

**Notes (all 4 positions):**
- `Note right of A: text`
- `Note left of A: text`
- `Note over A: text`
- `Note over A,B: text` (spanning)

**Activation:**
- `activate A` / `deactivate A`

**Fragments:**
- `loop Label` ... `end`
- `alt Label` ... `else Label` ... `end`
- `opt Label` ... `end`
- `par Label` ... `end`
- Nesting supported (fragments inside fragments)

**Autonumbering:**
- `autonumber` — prefixes messages with sequential numbers

**Comments:** `%%` lines ignored.

## Architecture

### Module Structure

```
src/mermaid/
├── mod.rs              # dispatch: flowchart vs sequence
├── parse.rs            # flowchart parser (unchanged)
├── layout.rs           # flowchart layout (unchanged)
├── ascii.rs            # flowchart render, Canvas becomes pub(crate)
└── sequence/
    ├── mod.rs          # data structures
    ├── parse.rs        # sequence parser
    ├── layout.rs       # column-based layout
    └── ascii.rs        # sequence renderer (uses shared Canvas)
```

### Integration Point

`render_mermaid()` in `mermaid/mod.rs` detects diagram type from first non-blank, non-comment line:
- `sequenceDiagram` → sequence pipeline
- `graph`/`flowchart` → flowchart pipeline (existing)

No changes to `parser.rs`, `render.rs`, `pager.rs`, or `main.rs`. The existing `Block::MermaidBlock` dispatch handles everything transparently.

### Canvas Sharing

`Canvas` struct in `mermaid::ascii` made `pub(crate)` so `sequence::ascii` can reuse it. This is the only change to existing flowchart code.

## Data Structures

```rust
pub struct SequenceDiagram {
    pub participants: Vec<Participant>,
    pub events: Vec<Event>,
    pub autonumber: bool,
}

pub struct Participant {
    pub id: String,
    pub label: String,
}

pub enum Event {
    Message {
        from: String,
        to: String,
        label: String,
        arrow: ArrowStyle,
    },
    Note {
        position: NotePosition,
        participants: Vec<String>,
        text: String,
    },
    Activate { participant: String },
    Deactivate { participant: String },
    Fragment {
        kind: FragmentKind,
        label: String,
        sections: Vec<FragmentSection>,
    },
}

pub struct FragmentSection {
    pub label: Option<String>,  // None for first section, Some("Failure") for else
    pub events: Vec<Event>,     // recursive
}

pub enum ArrowStyle {
    SolidArrow,    // ->>
    DashedArrow,   // -->>
    SolidOpen,     // ->
    DashedOpen,    // -->
    SolidCross,    // -x
    DashedCross,   // --x
}

pub enum NotePosition {
    RightOf,
    LeftOf,
    Over,
}

pub enum FragmentKind {
    Loop,
    Alt,
    Opt,
    Par,
}
```

`Event` is recursive via `FragmentSection.events`, naturally modeling nested fragments.

## Parsing

**Entry point:** `sequence/parse.rs`

```rust
pub fn parse_sequence(input: &str) -> anyhow::Result<SequenceDiagram>
```

**Strategy:** Line-by-line. First non-blank, non-comment line must be `sequenceDiagram`.

**Line dispatch:**

| Pattern | Action |
|---------|--------|
| `participant A as Label` | Add participant |
| `actor A as Label` | Add participant (same rendering) |
| `autonumber` | Set flag |
| `A->>B: text` | Message (detect arrow style) |
| `Note right of A: text` | Note |
| `activate A` | Activate event |
| `deactivate A` | Deactivate event |
| `loop Label` | Push fragment onto stack |
| `alt Label` / `opt Label` / `par Label` | Push fragment onto stack |
| `else Label` | New section in top-of-stack fragment |
| `end` | Pop fragment, add as Event to parent |

**Arrow parsing:** Match longest prefix first to avoid ambiguity:
1. `-->>` (4 chars)
2. `->>` (3 chars)
3. `-->` (3 chars)
4. `--x` (3 chars)
5. `->` (2 chars)
6. `-x` (2 chars)

**Implicit participants:** Messages referencing unknown IDs auto-create participants. Order = first appearance across all declarations and messages.

**Fragment stack:** Fragments parsed recursively via stack. Events accumulate into the current fragment's current section. `end` pops and emits the completed fragment as an Event.

## Layout

**Entry point:** `sequence/layout.rs`

```rust
pub fn layout(diagram: &SequenceDiagram) -> SequenceLayout
```

**Core model:** Fixed columns for participants, time flows downward.

### Layout Output

```rust
pub struct SequenceLayout {
    pub participants: Vec<PositionedParticipant>,
    pub lifelines: Vec<Lifeline>,
    pub messages: Vec<PositionedMessage>,
    pub notes: Vec<PositionedNote>,
    pub activations: Vec<PositionedActivation>,
    pub fragments: Vec<PositionedFragment>,
    pub width: usize,
    pub height: usize,
}

pub struct PositionedParticipant {
    pub label: String,
    pub x: usize,
    pub y: usize,         // always 0
    pub width: usize,
    pub center_x: usize,  // lifeline x-coordinate
}

pub struct PositionedMessage {
    pub from_x: usize,
    pub to_x: usize,
    pub y: usize,
    pub label: String,
    pub arrow: ArrowStyle,
    pub self_message: bool,
}

pub struct PositionedNote {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub text: String,
}

pub struct PositionedActivation {
    pub x: usize,
    pub y_start: usize,
    pub y_end: usize,
}

pub struct PositionedFragment {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub kind: FragmentKind,
    pub label: String,
    pub section_dividers: Vec<(usize, Option<String>)>,
}
```

### Column Assignment

Space participants with gap = `max(participant_label_width, longest_message_between_neighbors) + padding`. Each participant gets a `center_x` for its lifeline.

### Vertical Pass

Walk events top-to-bottom, incrementing `current_y`:

| Event type | Rows consumed |
|------------|---------------|
| Message | 2 (label + arrow) |
| Self-message | 3 (out stub + vertical + return) |
| Note | text lines + 2 (borders) |
| Activate/deactivate | 0 (marks y on activation stack) |
| Fragment start | 1 (header line) |
| Fragment `else` | 1 (divider line) |
| Fragment end | 1 (footer line) |

### Activation Tracking

Per-participant stack. `activate` pushes `current_y`. `deactivate` pops and emits `PositionedActivation { y_start, y_end }`.

### Fragment Bounds

Fragment x spans leftmost to rightmost involved participant plus margin. Height determined by recursive event layout.

## ASCII Rendering

**Entry point:** `sequence/ascii.rs`

```rust
pub fn render(layout: &SequenceLayout) -> Vec<String>
```

Uses shared `Canvas` from `mermaid::ascii`.

### Draw Order

1. **Lifelines** — dashed vertical `│` at each participant's center_x
2. **Activations** — 3-char-wide box over lifeline
3. **Fragment boxes** — borders with kind/label headers, `else` dividers
4. **Messages** — horizontal arrows with labels above
5. **Notes** — bordered boxes
6. **Participant boxes** — drawn last, sit clean on top

### Message Arrow Rendering

**Left-to-right:**
```
  label text
──────────────>>
```

**Right-to-left:**
```
       label text
<<──────────────
```

| Arrow style | Line char | Head (right) | Head (left) |
|-------------|-----------|--------------|-------------|
| SolidArrow | `─` | `>>` | `<<` |
| DashedArrow | `- ` | `>>` | `<<` |
| SolidOpen | `─` | `>` | `<` |
| DashedOpen | `- ` | `>` | `<` |
| SolidCross | `─` | `x` | `x` |
| DashedCross | `- ` | `x` | `x` |

### Self-Message Rendering

```
──┐ label
  │
<─┘
```

3 rows: right stub with label, vertical drop, left return with arrowhead.

### Activation Rendering

```
 ┌─┐
 │ │  active body
 └─┘
```

3-char wide box centered on lifeline. Replaces dashed lifeline while active.

### Fragment Rendering

```
┌─ loop [Every minute] ──────────────────┐
│                                        │
│  (contained events rendered normally)  │
│                                        │
└────────────────────────────────────────┘
```

Alt with else sections:
```
┌─ alt [Success] ────────────────────────┐
│                                        │
│  (success path events)                 │
│                                        │
├─ [Failure] ────────────────────────────┤
│                                        │
│  (failure path events)                 │
│                                        │
└────────────────────────────────────────┘
```

### Note Rendering

```
┌──────────┐
│ Thinking │
└──────────┘
```

Positioned relative to participant center_x based on NotePosition. Spanning notes center between the two participants.

## Test Strategy

### Test Fixtures (one feature per file)

| File | Feature | Content |
|------|---------|---------|
| `test-seq-basic.md` | Minimal | 2 participants, 1 message |
| `test-seq-arrows.md` | Arrow styles | All 6 arrow types between 2 participants |
| `test-seq-multi.md` | Multiple participants | 3+ participants, various messages |
| `test-seq-self.md` | Self-message | A->>A loopback |
| `test-seq-activate.md` | Activation | activate/deactivate pairs |
| `test-seq-notes.md` | Notes | All 4 note positions |
| `test-seq-loop.md` | Loop fragment | Single loop block |
| `test-seq-alt.md` | Alt fragment | alt/else block |
| `test-seq-opt.md` | Opt fragment | Optional block |
| `test-seq-par.md` | Par fragment | Parallel block |
| `test-seq-nested.md` | Nesting | Fragment inside fragment |
| `test-seq-autonumber.md` | Autonumbering | Sequential message numbers |
| `test-seq-implicit.md` | Implicit participants | No declarations, auto-created |
| `test-seq-complex.md` | Full integration | Real-world API auth flow with all features |

### Unit Tests

- **Parser:** One test per syntax element (participant, each arrow, notes, fragments, nesting, autonumber, implicit participants, comments)
- **Layout:** Column spacing, y-positioning, activation tracking, fragment bounds, self-message height
- **Render:** Arrow characters, lifeline drawing, activation boxes, fragment borders, note boxes

### Feedback Loop

Same as flowcharts: run `mdx --no-pager` on each fixture, read output, fix, rebuild, re-run.

## Incremental Delivery

Build in passes:

1. **Pass 1:** Parse + layout + render for basic messages (test-seq-basic, test-seq-arrows, test-seq-multi, test-seq-implicit)
2. **Pass 2:** Self-messages + activation (test-seq-self, test-seq-activate)
3. **Pass 3:** Notes (test-seq-notes)
4. **Pass 4:** Fragments — loop, alt, opt, par, nesting (test-seq-loop through test-seq-nested)
5. **Pass 5:** Autonumber + complex integration (test-seq-autonumber, test-seq-complex)

Each pass is independently testable and committable.
