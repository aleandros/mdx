# ER Diagram Support

**Status:** Draft — design approved, awaiting user review before implementation plan.
**Date:** 2026-04-25

## Goal

Render Mermaid `erDiagram` blocks as ASCII in the terminal, joining the existing `flowchart`/`graph` and `sequenceDiagram` support. Scope is full Mermaid ER syntax (entities, attributes with types/keys/comments, identifying/non-identifying relationships, crow's foot cardinality, relationship labels).

After this work lands, `tests/integration.rs` and the `complex-flow-chart-example.md` `erDiagram` block should render without error, and the README "Diagram support is scoped" caveat should drop ER.

## Non-goals

- Mermaid `classDiagram`, `stateDiagram`, `gantt`, etc. — out of scope.
- Interactive editing or hover-reveal of long comments — comments wrap inline.
- A new pager mode for ER — the existing pager handles styled lines uniformly.
- New theme keys — reuse `diagram_node_border`, `diagram_node_text`, `diagram_edge_stroke`, `diagram_edge_label`, `diagram_border`.

## Decisions

| Question | Decision |
|---|---|
| Syntax scope | Full Mermaid ER (entities + attributes + comments + cardinality + labels + PK/FK + identifying/non-identifying) |
| Cardinality rendering | ASCII crow's foot endpoints (`\|\|`, `o\|`, `}o`, `}\|`) — no Unicode |
| Attribute comment rendering | Wrapped below attribute, indented; box width capped at `max_box_width` (default 50 cols) |
| Layout | Adaptive: try `LeftRight` first; fall back to `TopDown` if laid-out width exceeds terminal width |
| Layout reuse | Reuse `mermaid::layout::layout` via an adapter that produces `FlowChart` with a new `NodeShape::EntityBox` |
| Theme | No new keys. Borders/text/edges/labels use existing diagram theme entries |

## Architecture

New module tree:

```
src/mermaid/
  er/
    mod.rs        # ER types
    parse.rs      # erDiagram → ErDiagram
    layout.rs     # ErDiagram → FlowChart (adapter), entity dim calc
    ascii.rs      # ER painter (entity box, crow's foot endpoints)
```

`src/mermaid/mod.rs::render_mermaid` gains a third dispatch branch when the first non-comment line equals `erDiagram`. Existing flowchart and sequence branches are untouched.

Two minimal extensions to shared modules:

1. `NodeShape` gains an `EntityBox` variant.
2. `Node` gains an optional `entity: Option<er::Entity>` field. None for flowchart, Some for ER.

These are the only flowchart-side changes. Layout's `node_dimensions` adds one branch for `EntityBox` (returns precomputed dims). The ASCII renderer's per-node paint dispatch adds one branch (`if entity.is_some()` → ER painter).

### Why this shape

- New ER concepts (cardinality, attribute keys, identifying edges) live in their own types — clean boundary, no flowchart contamination.
- The heavy shared work — node positioning, edge routing, subgraph boxes — is reused. The ER box is just a tall rect from layout's perspective.
- Painting is per-diagram-type already (sequence has its own painter); ER follows the same precedent.

## Data model

```rust
// src/mermaid/er/mod.rs

pub struct ErDiagram {
    pub direction: Direction,
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

pub struct Entity {
    pub name: String,
    pub attributes: Vec<Attribute>,
    // Populated by the layout adapter once max_box_width is known.
    pub rendered_lines: Vec<EntityLine>,
    pub width: usize,
    pub height: usize,
}

pub struct Attribute {
    pub ty: String,
    pub name: String,
    pub key: KeyKind,
    pub comment: Option<String>,
}

pub enum KeyKind { None, Pk, Fk, PkFk }

pub struct Relationship {
    pub left: String,
    pub right: String,
    pub left_card: Cardinality,
    pub right_card: Cardinality,
    pub identifying: bool,   // true for `--`, false for `..`
    pub label: Option<String>,
}

pub enum Cardinality { ZeroOrOne, ExactlyOne, ZeroOrMany, OneOrMany }

// Internal: pre-painted entity box content, attached to PositionedNode via Entity.
pub struct EntityLine {
    pub kind: EntityLineKind,
    pub text: String,
}
pub enum EntityLineKind { Header, Separator, AttrRow, CommentRow }
```

## Parsing

`er::parse::parse_er(content: &str) -> anyhow::Result<ErDiagram>` — handwritten line-based parser, same style as `mermaid::parse::parse_flowchart`.

### Header

First non-empty, non-comment line must be `erDiagram`. Optional `direction LR` / `direction TD` line follows (extension to standard Mermaid; matches flowchart's behavior).

### Cardinality tokens

Two-character tokens at each end of the relationship operator. The operator itself is `--` (identifying) or `..` (non-identifying).

| Token | Cardinality | Side |
|---|---|---|
| `\|\|` | ExactlyOne | either |
| `o\|` | ZeroOrOne | left side (open circle outside) |
| `\|o` | ZeroOrOne | right side |
| `}o` | ZeroOrMany | left |
| `o{` | ZeroOrMany | right |
| `}\|` | OneOrMany | left |
| `\|{` | OneOrMany | right |

Relationship line grammar:

```
NAME LEFTCARD ('--' | '..') RIGHTCARD NAME (':' LABEL)?
```

`LABEL` may be a quoted string or an unquoted identifier (matches Mermaid).

### Entity block grammar

```
NAME '{'
  (TYPE ATTRNAME (PK | FK | 'PK,FK' | 'FK,PK')? COMMENT?)*
'}'
```

`TYPE` and `ATTRNAME` are identifiers. `COMMENT` is a quoted string. Whitespace within an attribute line is collapsed.

### Auto-creation

A relationship referencing a name that has no entity block creates an empty `Entity { name, attributes: [] }`. Matches Mermaid behavior.

### Errors

Use `anyhow::Result` with line-number context. Hard errors:

- Unknown cardinality token
- Malformed attribute line inside `{ }`
- Unclosed entity block

Empty diagram (`erDiagram` with no body) is **not** an error — yields `ErDiagram { entities: [], relationships: [] }`.

## Layout adapter

`er::layout::to_flowchart(&ErDiagram, max_box_width: usize) -> FlowChart`

For each entity:

1. Compute three column widths across all attributes:
   - `key_w` — width of widest key marker (`PK`, `FK`, `PK,FK`, blank)
   - `type_w` — width of widest type token
   - `name_w` — width of widest attribute name
2. Attribute row format: `{KEY} {TYPE} {NAME}` padded to fixed columns.
3. Comment width budget: `max_box_width - 4 (border+padding) - key_w - type_w - 2`. If a comment fits in remaining space on the same line, inline it. Otherwise, wrap onto subsequent lines indented under the attribute name column.
4. Box width = `max(name_row_width, "+ name +" header width, max attr/comment line width) + 2`.
5. Box height = `2 (top/bottom border) + 1 (header) + 1 (separator) + sum(rows per attribute)`.

The adapter writes `entity.rendered_lines` (header, separator, attr rows, comment rows) and `entity.width`/`entity.height` so painting is a straight copy.

The adapter returns a `FlowChart` whose `nodes` are `Node { shape: EntityBox, entity: Some(...), label: name, ... }` and whose `edges` carry the cardinality/identifying/label data encoded so the ER painter can draw correct endpoints. Concretely, edges carry:

- `style: EdgeStyle::Arrow` for identifying, `EdgeStyle::Dotted` for non-identifying
- `label: relationship.label.clone()`
- A new optional field on `Edge`: `er_meta: Option<er::ErEdgeMeta>` carrying `(left_card, right_card, identifying)`. Mirrors the `Node.entity` extension pattern — None for flowchart edges, Some for ER.

### Adaptive direction

In `render_mermaid` for the ER branch:

1. Resolve terminal width. Add a `terminal_width: usize` parameter to `render_mermaid`. Call sites: `pager.rs`, `embed.rs`, and any tests that call `render_mermaid` directly. Tests with no preference pass `120`.
2. Run `to_flowchart` with `max_box_width = min(50, terminal_width / 3)`.
3. Lay out with `Direction::LeftRight`.
4. If `result.width > terminal_width`, re-lay out with `Direction::TopDown` and use that.
5. If the user wrote `direction LR` / `direction TD`, skip step 4 — honor the explicit choice.

## Painting

`er::ascii::render_styled(layout: &LayoutResult, theme_meta: &ErThemeMeta) -> Vec<StyledLine>`

For each `PositionedNode` whose `entity.is_some()`:

```
+-- Notification ---------------------------+
| PK string  id                             |
|    string  name                           |
|            unique slug, e.g. card-        |
|            transaction-cardholder         |
| FK string  notificationId                 |
|    int     ttlMs                          |
|            max age before discard         |
|            @default(10 days)              |
+-------------------------------------------+
```

- Border characters: `+`, `-`, `|` (matches existing flowchart rect style for visual consistency).
- Header row contains the entity name centered with a leading/trailing dash padding.
- Separator below header is a row of `-` from inside-left to inside-right.
- Attribute rows: `KEY` column left-aligned, then `TYPE`, then `NAME`, then optional inline comment.
- Comment continuation rows align under the `NAME` column.

For each `PositionedEdge`:

- Default flowchart edge routing draws the line.
- ER painter overwrites both endpoint cells with the cardinality glyph pair for that side.
- Identifying edges use solid line characters; non-identifying use dotted (uses existing `EdgeStyle::Dotted` rendering).
- Relationship label paints mid-edge, reusing the flowchart label painter.

### Crow's foot glyph mapping (ASCII)

| Cardinality | Left-side glyph | Right-side glyph |
|---|---|---|
| ExactlyOne | `\|\|` | `\|\|` |
| ZeroOrOne | `o\|` | `\|o` |
| ZeroOrMany | `}o` | `o{` |
| OneOrMany | `}\|` | `\|{` |

Painted in the two cells nearest each entity (perpendicular to the edge direction). Existing arrowhead painting is suppressed for ER edges.

## Theming

Reuse:

- `diagram_node_border` — entity box border, separator, attribute key column accent (optional)
- `diagram_node_text` — attribute name/type, header, comments
- `diagram_edge_stroke` — relationship lines and crow's foot glyphs
- `diagram_edge_label` — relationship labels
- `diagram_border` — unused for ER (no subgraphs in standard Mermaid ER)

No new theme keys. No `style` / `classDef` support in v1 (Mermaid ER itself doesn't define inline styling for entities).

## Error handling

Match existing parser style. `parse_er` returns `anyhow::Result<ErDiagram>` with line numbers in error context. `render_mermaid`'s ER branch propagates parse errors the same way the flowchart branch does today.

## Testing

### Unit — parse (in `er/parse.rs`)

- Each of the eight cardinality tokens parses correctly on each side.
- Identifying (`--`) vs non-identifying (`..`) edges.
- Attribute with each `KeyKind` value.
- Attribute with and without comment.
- Empty entity (`Entity { }`).
- Auto-created entity from a relationship line.
- Quoted vs unquoted relationship labels.
- Errors: unknown cardinality token, malformed attribute, unclosed brace — assert error contains line number.

### Unit — layout (in `er/layout.rs`)

- Entity dimension calc with: zero attributes, one attribute no comment, attribute with short comment that fits inline, attribute with long comment that wraps.
- Column-width alignment across heterogeneous attribute lengths.
- `max_box_width` is honored (no row exceeds it).

### Snapshot (`tests/snapshots/`)

Add four new snapshots:

1. **`er_minimal`** — two entities, one `||--o{` relationship, no attributes.
2. **`er_full`** — the `erDiagram` block from `complex-flow-chart-example.md` lines 228–298, rendered at `--width 200 --height 200`.
3. **`er_identifying_vs_non`** — same two entities connected once with `--` and once with `..`.
4. **`er_lr_overflow_falls_back_to_td`** — five entities at `--width 80`, asserting TD fallback fires.

### Integration (`tests/integration.rs`)

- New fixture markdown with an `erDiagram` block. Assert the final rendered output contains expected entity names and at least one crow's foot glyph pair.
- Existing flowchart and sequence integration tests must still pass — confirms the dispatch branch and `Node.entity` field don't break existing paths.

### Documentation

- `README.md` — flip ER from "not yet supported" to supported. Add a one-line example.
- `docs/USAGE.md` — add a Mermaid ER syntax section listing supported tokens and the `direction` extension.
- `CHANGELOG.md` — add `0.1.8` entry: "feat(mermaid): erDiagram support".

## Open questions

None — all answered during brainstorming.

## Out-of-scope follow-ups

- ER `style` / `classDef` if Mermaid ever standardizes it.
- Unicode crow's foot glyphs as an opt-in theme/config flag.
- Attribute comment expand-on-demand via the pager (`o`-key style).
