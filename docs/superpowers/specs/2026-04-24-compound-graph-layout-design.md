# Compound Graph Layout — Design Spec

**Date:** 2026-04-24
**Status:** Approved

---

## Problem

Mermaid `subgraph` blocks declare logical cluster groups. The current layout assigns ranks
to individual nodes ignoring cluster membership, so nodes from different subgraphs interleave
across ranks. Bounding boxes then span non-contiguous regions and produce wide, visually
misleading rectangles.

---

## Goal

All members of each subgraph occupy a **contiguous block of ranks** with no foreign nodes
interspersed. Bounding boxes are compact in both the primary (rank) and secondary
(within-rank) axes.

---

## Scope

- **In scope:** Phase 0 (cluster rank assignment), Phase 1 modification, Phase 2 sort key, 5 new tests.
- **Out of scope:** Inter-cluster edge routing around subgraph box borders (deferred).
- **No changes to:** `parse.rs`, `mod.rs`, `ascii.rs`.

---

## Architecture

Single file change: `src/mermaid/layout.rs`.

New phase inserted before the existing rank assignment:

```
Phase 0 (NEW):  assign_cluster_ranks()  →  HashMap<cluster_id, usize>
Phase 1 (MOD):  per-cluster Kahn pass   →  ranks[] (globally contiguous per cluster)
Phase 2 (MOD):  (cluster_rank, bc) sort →  rank_groups
Phase 3+:       unchanged
```

Two additions:
- `fn assign_cluster_ranks(chart: &FlowChart) -> HashMap<String, usize>` — pure function, no side effects.
- Modified rank loop in `layout()` replaces the current single-pass Kahn with a cluster-aware two-pass version.

---

## Algorithm

### Phase 0 — `assign_cluster_ranks(chart)`

1. Build `node_to_cluster: HashMap<&str, String>`:
   - Each subgraph member maps to its subgraph ID.
   - Free nodes (not in any subgraph) map to `"__free__<node_id>"` (synthetic singleton cluster).

2. Build deduplicated inter-cluster edges: for each `Edge(u→v)` where `cluster(u) != cluster(v)`,
   add `(cluster(u), cluster(v))` to a `HashSet`.

3. Assign a numeric index to each unique cluster ID.

4. Run Kahn + longest-path on the cluster graph (same algorithm as current Phase 1).
   Return `HashMap<cluster_id, usize>`.

### Phase 1 — Per-cluster internal rank assignment

For each cluster, run Kahn + longest-path on its member nodes (only intra-cluster edges).
Each node's final rank:

```
rank[node] = cluster_rank_offset[cluster] + internal_rank[node]
```

where:

```
cluster_rank_offset[c] = cluster_rank[c] * (max_internal_depth + 1)
```

`max_internal_depth` = maximum internal rank depth across all clusters (uniform band width).

Free nodes (singleton clusters) get `internal_rank = 0`.

### Phase 2 — Within-rank ordering

Sort key extended from `(barycenter: f64)` to `(cluster_rank: usize, barycenter: f64)`:

```rust
cr_a.cmp(&cr_b)
    .then(a.1.partial_cmp(&b.1).unwrap())
    .then(a.0.cmp(&b.0))
```

Groups same-cluster nodes together on the secondary axis within each rank.

### Phases 3–5 — Coordinate assignment, bounding boxes, rendering

Unchanged. With contiguous ranks, `SubgraphBox` computation naturally produces compact boxes.

---

## Testing

All new tests added to `src/mermaid/layout.rs` `#[cfg(test)]`. Existing tests are unchanged
(they all use `subgraphs: vec![]`, so Phase 0 is a no-op for them).

| Test | Assertion |
|------|-----------|
| `test_subgraph_members_contiguous_ranks` | All members of each subgraph have ranks in a contiguous block; no rank overlap between clusters. |
| `test_subgraph_secondary_axis_grouping` | LR direction: same-subgraph nodes at the same rank are adjacent in y (no foreign node between them). |
| `test_subgraph_box_compact` | `SubgraphBox` width and height ≤ total diagram width/height. |
| `test_free_nodes_with_subgraphs` | Free nodes mixed with subgraph nodes; subgraph members still contiguous. |
| `test_single_subgraph_no_regression` | Single subgraph with linear chain; existing rank ordering preserved. |

---

## Open Questions Resolved

| Question | Decision |
|----------|----------|
| Rank band sizing | `max_internal_depth + 1` uniform width across all clusters. Empty slots are not collapsed. |
| Cross-cluster edges at intermediate ranks | Deferred. `route_edge` unchanged for now. |
| Nested subgraphs | Treated as flat (innermost subgraph wins for node membership). Nesting is a follow-up. |
| Direction interaction | Phase 0/1 operate on rank integers, direction-independent. `sg_top_margin` unchanged. |
