# ER Diagram Coloring and Custom Styles

**Status:** Draft — design approved, awaiting user review before implementation plan.
**Date:** 2026-04-26
**Builds on:** `2026-04-25-er-diagrams-design.md` (initial ER support, shipped in v0.1.8)

## Goal

Make ER diagrams pick up theme colors by default (matching the flowchart and sequence diagram experience) and accept the same per-entity / class-based style overrides that flowchart already supports.

After this work, an unstyled `erDiagram` block in any UI theme should render with colored borders, text, and connecting lines. Users can additionally write `style`, `classDef`, and `class` directives inside `erDiagram` blocks to override colors per entity.

## Non-goals

- New theme keys. Reuse `diagram_node_border`, `diagram_node_text`, `diagram_edge_stroke`, `diagram_edge_label`.
- Edge-index styling (`linkStyle 0`). ER relationships are not ergonomically indexable.
- Per-attribute coloring (e.g. PK column highlight). Mermaid doesn't define this; YAGNI.
- Visual fixes for the v0.1.8 known issues (crow's foot painting over entity borders; label/glyph collision in tight LR layouts). Those are separate follow-ups; coloring is orthogonal.

## Decisions

| Question | Decision |
|---|---|
| Custom styling syntax | Mirror flowchart: `style`, `classDef`, `class`. No `linkStyle`. |
| Default theme keys | Reuse `diagram_node_border`, `diagram_node_text`, `diagram_edge_stroke`, `diagram_edge_label`. No new keys. |
| Where styles take effect | `node_style.stroke` colors box borders, separators, and crow's foot glyphs. `node_style.color` colors header, attribute text, PK/FK markers, and comments. `node_style.fill` is treated as a fallback for `stroke` (matches `stroke_style` in flowchart). |
| Class vs explicit `style` precedence | Explicit `style` overrides `class`. Class overrides theme defaults. |
| Style on undefined entity | Auto-create the entity (matches existing relationship-implied entity behavior). |
| Unknown class reference | Silently ignored (matches flowchart's `apply_class` behavior). |

## Architecture

The coloring problem has two pieces:

1. **Painter respects `node_style` / `edge_style`.** Today the ER painter ignores these because of the post-processing pass introduced in Task 11 of the v0.1.8 plan: it re-paints the styled rows with default-styled spans, dropping any color information. Replace that with a properly styled ER painter that emits `StyledLine`s directly, consulting the resolved colors on each `PositionedNode` / `PositionedEdge`.

2. **Parser absorbs custom-style directives.** Add three new line handlers to `parse_er`: `style`, `classDef`, `class`. Reuse the existing `parse_node_style_props` from `src/mermaid/color.rs` so the syntax is byte-identical to the flowchart parser. Defer style application until end-of-parse (so `class A foo` can come before `classDef foo ...`), mirroring flowchart's pattern.

Plain (non-styled) rendering is unaffected — `paint_entity` and `paint_cardinality` continue to overlay onto the existing canvas. Only the styled path changes.

## Data model changes

`src/mermaid/er/mod.rs::Entity` gains:

```rust
pub struct Entity {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub rendered_lines: Vec<EntityLine>,
    pub width: usize,
    pub height: usize,
    pub node_style: Option<crate::mermaid::NodeStyle>,
}
```

No new ER types are needed. Edges have no equivalent change in v1 — relationship-level styling isn't supported.

## Parsing

### Three new directives in `parse_er`

Inside the body loop in `src/mermaid/er/parse.rs`, add (in order, before the existing relationship/entity-block dispatch):

```rust
if let Some(rest) = trimmed.strip_prefix("style ") {
    if let Some((id, props)) = rest.split_once(char::is_whitespace) {
        node_styles.push((id.trim().to_string(), parse_node_style_props(props.trim())));
    }
    continue;
}
if let Some(rest) = trimmed.strip_prefix("classDef ") {
    if let Some((cls, props)) = rest.split_once(char::is_whitespace) {
        class_defs.insert(cls.trim().to_string(), parse_node_style_props(props.trim()));
    }
    continue;
}
if let Some(rest) = trimmed.strip_prefix("class ") {
    if let Some((ids, cls)) = rest.split_once(char::is_whitespace) {
        let entity_ids: Vec<String> = ids.split(',').map(|s| s.trim().to_string()).collect();
        class_assignments.push((entity_ids, cls.trim().to_string()));
    }
    continue;
}
```

Carry three new accumulators alongside the existing `entities`, `entity_order`, `relationships`:

```rust
let mut class_defs: HashMap<String, NodeStyle> = HashMap::new();
let mut class_assignments: Vec<(Vec<String>, String)> = Vec::new();
let mut node_styles: Vec<(String, NodeStyle)> = Vec::new();
```

### End-of-parse application

After the body loop finishes and before constructing `entities_vec`:

```rust
// Apply class assignments first.
for (entity_ids, cls) in &class_assignments {
    if let Some(style) = class_defs.get(cls) {
        for id in entity_ids {
            ensure_entity(id, &mut entity_order, &mut entities);
            let e = entities.get_mut(id).unwrap();
            e.node_style = Some(style.clone());
        }
    }
    // Unknown class: silently ignored.
}
// Then apply explicit `style` lines, which override class.
for (id, style) in &node_styles {
    ensure_entity(id, &mut entity_order, &mut entities);
    let e = entities.get_mut(id).unwrap();
    e.node_style = Some(style.clone());
}
```

`style` and `class` lines may reference entities that don't yet exist — `ensure_entity` already auto-creates them.

## Layout adapter

`src/mermaid/er/layout.rs::to_flowchart` propagates `entity.node_style` onto the `Node`:

```rust
.map(|e| Node {
    id: e.name.clone(),
    label: e.name.clone(),
    shape: NodeShape::EntityBox,
    node_style: e.node_style.clone(),
    entity: Some(e.clone()),
})
```

No other layout changes.

## Theme defaults in `render_mermaid`

The Task-13 logic that fills `node.node_style` with theme defaults when None is **kept** but moved to apply *after* parser-supplied styles. The order at render time:

1. Parser writes `entity.node_style` for entities mentioned by `style` or `class`.
2. Layout adapter copies `entity.node_style` onto `Node.node_style`.
3. `render_mermaid` ER branch fills any remaining `Node.node_style == None` with theme defaults (existing logic).
4. Same for edges via theme `diagram_edge_*`.

After step 3, every `PositionedNode` has a non-None `node_style`. The painter can read it unconditionally.

**Partial-style note:** if a `classDef` or `style` sets only some fields (e.g. only `fill`), the resulting `Some(NodeStyle { fill: Some, stroke: None, color: None })` is treated as user-specified and the theme defaults do NOT fill in the missing fields. The painter's `stroke_style` already falls back to `fill` when `stroke` is None (matches flowchart's `stroke_style`); for `color` (text), missing means uncolored text. This matches flowchart semantics — users override what they specify, the rest stays unstyled rather than re-falling-back to theme.

## Painting

Two new functions in `src/mermaid/er/ascii.rs`:

```rust
pub fn paint_entity_styled(
    rows: &mut [StyledLine],
    node: &PositionedNode,
);

pub fn paint_cardinality_styled(
    rows: &mut [StyledLine],
    edge: &PositionedEdge,
);
```

Both pull colors directly from `node.node_style` / `edge.edge_style` (now guaranteed non-None by the render pipeline).

`paint_entity_styled` walks the entity's rendered region and replaces affected `StyledSpan`s. For each cell:

- `+`, `-`, `|` characters → `fg = stroke || fill` (matches flowchart's `stroke_style`).
- All other characters (header text, attribute rows, PK/FK markers, comments) → `fg = color` (matches flowchart's `label_style`).

`paint_cardinality_styled` colors the two endpoint cells per side using `edge.edge_style.stroke` (the same color that `apply_edge_style` paints the line body with).

The `StyledLine` row-rebuilding helper used in v0.1.8 (which currently produces a single default-styled span per row) is replaced by a per-cell rebuilder: for each rebuilt row, walk cells from left to right, group runs of identical `SpanStyle`, emit one `StyledSpan` per run.

### Replacement in `src/mermaid/ascii.rs::render_styled`

Find the existing block that:
1. Builds a parallel plain canvas via `paint_entity` / `paint_cardinality`.
2. Computes `rows_to_replace` (entity row range plus edge endpoint rows ±1).
3. Overwrites those `StyledLine`s with single default-styled spans.

Replace step 3 with:
- For each `PositionedNode` with `entity.is_some()`, call `paint_entity_styled(&mut rows, node)`.
- For each `PositionedEdge` with `er_meta.is_some()`, call `paint_cardinality_styled(&mut rows, edge)`.

Keep the parallel-plain-canvas approach so the cell content is correct; styled painting layers color on top.

`render` (plain) is unchanged.

## Error handling

`parse_node_style_props` already accepts ill-formed property strings gracefully (any unrecognized property is dropped; bad colors fall through). No new error paths.

Class assignments referencing unknown classes are silently ignored, matching flowchart.

## Testing

### Unit (parse, in `src/mermaid/er/parse.rs::tests`)

- `test_parse_style_directive` — `style E fill:#f00,stroke:#0f0,color:#00f` → entity has matching `NodeStyle` with the three colors set.
- `test_parse_class_def_and_assignment` — `classDef foo fill:#f9f` + `class A foo` → A has `fill: Some(#f9f)`.
- `test_parse_class_assignment_multiple_entities` — `class A,B,C foo` → A, B, C all share the style.
- `test_parse_style_overrides_class` — `class E foo` then `style E stroke:#000` → final `node_style.stroke` is `#000`, fill from class survives.
- `test_parse_style_on_implicit_entity` — entity referenced only by `style` line and a relationship → entity exists with the style applied.
- `test_parse_unknown_class_silently_ignored` — `class A nonexistent` → no error, A has `node_style: None`.

### Unit (layout)

- `test_to_flowchart_propagates_node_style` — entity with `node_style = Some(...)` produces a `Node` with the same `node_style`.

### Unit (paint, styled)

These exercise the per-cell rebuilder by sampling the resulting StyledSpans:

- `test_paint_entity_styled_borders_use_stroke_color` — entity with `stroke = Red` → border cells (`+`, `-`, `|`) have spans with `fg: Some(Red)`.
- `test_paint_entity_styled_text_uses_color` — entity with `color = Blue` → header/attribute cells have spans with `fg: Some(Blue)`.
- `test_paint_entity_styled_default_theme` — entity with theme defaults produces non-None `fg` on borders and text (any color, just confirming colors are emitted).
- `test_paint_cardinality_styled_uses_edge_stroke` — edge with `edge_style.stroke = Green` → both endpoint glyph cells have `fg: Some(Green)`.

### Snapshot

- New fixture `docs/examples/er-styled.md`:
  ```mermaid
  erDiagram
      Notification ||--o{ Pref : has
      classDef audit fill:#666
      classDef config fill:#fc0
      Notification {
        string id PK
      }
      Pref {
        string id PK
        string notificationId FK
      }
      class Notification config
      class Pref audit
      style Notification stroke:#f00
  ```
- Snapshot at w120: `snapshot_er_styled_w120`.

### Integration

- Existing `integration_renders_er_diagram` continues to pass. The new default-coloring path produces ANSI escapes; the integration test only asserts plain-text substrings, which is unaffected.

### Documentation

`docs/USAGE.md` ER section gains a "Styling" subsection mirroring the flowchart one. Add a brief example showing `style` + `classDef` + `class` for ER. CHANGELOG entry under `0.1.9` describing the addition.

## Out-of-scope follow-ups

Tracked separately, not addressed here:

- Crow's foot glyphs painting over entity box borders.
- Relationship label and cardinality glyph collision in tight LR layouts.
- ER `linkStyle` syntax (would need a non-index addressing scheme).
- Per-attribute styling (no Mermaid spec).
