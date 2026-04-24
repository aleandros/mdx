# Compound Graph Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make subgraph members occupy contiguous rank bands so bounding boxes are compact in both axes.

**Architecture:** Insert Phase 0 (`assign_cluster_ranks`) before the existing Kahn pass; replace the single-pass Phase 1 with a per-cluster internal rank pass; extend the Phase 2 barycenter sort key with a cluster-rank prefix. All changes are in `src/mermaid/layout.rs` only.

**Tech Stack:** Rust, `std::collections::{HashMap, HashSet, VecDeque}`

---

### Task 1: Failing test — subgraph members have contiguous ranks

**Files:**
- Modify: `src/mermaid/layout.rs` (tests section, ~line 882)

- [ ] **Step 1: Add the failing test**

Add this test inside the `#[cfg(test)] mod tests` block at the bottom of `src/mermaid/layout.rs`:

```rust
#[test]
fn test_subgraph_members_contiguous_ranks() {
    // Two subgraphs: Pkg(Send, Dispatch, Deliver) and MQ(DQ, CQ, DLQ)
    // Inter-cluster edge: Send -> DQ
    use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
    let nodes = vec![
        Node { id: "Send".into(), label: "Send".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "Dispatch".into(), label: "Dispatch".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "Deliver".into(), label: "Deliver".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "DQ".into(), label: "DQ".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "CQ".into(), label: "CQ".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "DLQ".into(), label: "DLQ".into(), shape: NodeShape::Rect, node_style: None },
    ];
    let edges = vec![
        Edge { from: "Send".into(), to: "Dispatch".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "Dispatch".into(), to: "Deliver".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "DQ".into(), to: "CQ".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "CQ".into(), to: "DLQ".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "Send".into(), to: "DQ".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
    ];
    let subgraphs = vec![
        Subgraph { id: "Pkg".into(), label: "packages/notifications".into(), node_ids: vec!["Send".into(), "Dispatch".into(), "Deliver".into()] },
        Subgraph { id: "MQ".into(), label: "RabbitMQ".into(), node_ids: vec!["DQ".into(), "CQ".into(), "DLQ".into()] },
    ];
    let chart = FlowChart { direction: Direction::LeftRight, nodes, edges, subgraphs };
    let result = layout(&chart);

    // Collect ranks per node id
    // We don't have ranks in PositionedNode directly, so verify via x positions:
    // All Pkg nodes must have x < all MQ nodes (or vice versa), i.e. the two clusters
    // must not interleave along the primary axis.
    let pkg_ids = ["Send", "Dispatch", "Deliver"];
    let mq_ids = ["DQ", "CQ", "DLQ"];
    let find_x = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().x;
    let pkg_xs: Vec<usize> = pkg_ids.iter().map(|id| find_x(id)).collect();
    let mq_xs: Vec<usize> = mq_ids.iter().map(|id| find_x(id)).collect();
    let pkg_max = pkg_xs.iter().max().unwrap();
    let mq_min = mq_xs.iter().min().unwrap();
    // Either Pkg is entirely left of MQ or MQ is entirely left of Pkg
    let pkg_min = pkg_xs.iter().min().unwrap();
    let mq_max = mq_xs.iter().max().unwrap();
    assert!(
        pkg_max < mq_min || mq_max < pkg_min,
        "Pkg and MQ x-ranges must not overlap. Pkg xs: {:?}, MQ xs: {:?}",
        pkg_xs, mq_xs
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd /home/edgar/mdx && cargo test test_subgraph_members_contiguous_ranks 2>&1 | tail -20
```

Expected: FAIL — Pkg and MQ x-ranges overlap because the current layout interleaves nodes.

---

### Task 2: Failing test — secondary axis grouping (LR)

**Files:**
- Modify: `src/mermaid/layout.rs` (tests section)

- [ ] **Step 1: Add the failing test**

```rust
#[test]
fn test_subgraph_secondary_axis_grouping() {
    // Two subgraphs each with 2 nodes, both at the same internal rank.
    // After layout, the two Pkg nodes should be y-adjacent (no MQ node between them).
    use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
    let nodes = vec![
        Node { id: "A1".into(), label: "A1".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "A2".into(), label: "A2".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "B1".into(), label: "B1".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "B2".into(), label: "B2".into(), shape: NodeShape::Rect, node_style: None },
    ];
    // No intra-cluster edges so all nodes land at rank 0 within their cluster.
    // Inter-cluster edge A1->B1 puts MQ after Pkg.
    let edges = vec![
        Edge { from: "A1".into(), to: "B1".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
    ];
    let subgraphs = vec![
        Subgraph { id: "SGA".into(), label: "Group A".into(), node_ids: vec!["A1".into(), "A2".into()] },
        Subgraph { id: "SGB".into(), label: "Group B".into(), node_ids: vec!["B1".into(), "B2".into()] },
    ];
    let chart = FlowChart { direction: Direction::LeftRight, nodes, edges, subgraphs };
    let result = layout(&chart);

    // At the same x (same rank), A1 and A2 must be y-adjacent: no B node y between them.
    let find = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().clone();
    let a1 = find("A1");
    let a2 = find("A2");
    let b1 = find("B1");
    let b2 = find("B2");

    // All four at the same rank (same x) because A1->B1 is inter-cluster and
    // intra-cluster ranks are 0 for everyone.
    // A1 and A2 are in the same cluster: their y values must be adjacent
    // (no B node's y falls strictly between them).
    let a_ys = {let mut v = vec![a1.y, a2.y]; v.sort(); v};
    let b_ys = vec![b1.y, b2.y];
    for &by in &b_ys {
        assert!(
            by < a_ys[0] || by > a_ys[1],
            "B node y={} falls between A1.y={} and A2.y={} — clusters are interleaved",
            by, a_ys[0], a_ys[1]
        );
    }
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cd /home/edgar/mdx && cargo test test_subgraph_secondary_axis_grouping 2>&1 | tail -20
```

Expected: FAIL.

---

### Task 3: Failing tests — box compactness, free nodes, no regression

**Files:**
- Modify: `src/mermaid/layout.rs` (tests section)

- [ ] **Step 1: Add three more failing tests**

```rust
#[test]
fn test_subgraph_box_compact() {
    use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
    let nodes = vec![
        Node { id: "S".into(), label: "Send".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "D".into(), label: "Dispatch".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "Q".into(), label: "Queue".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "C".into(), label: "Consume".into(), shape: NodeShape::Rect, node_style: None },
    ];
    let edges = vec![
        Edge { from: "S".into(), to: "D".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "S".into(), to: "Q".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "Q".into(), to: "C".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
    ];
    let subgraphs = vec![
        Subgraph { id: "SGA".into(), label: "App".into(), node_ids: vec!["S".into(), "D".into()] },
        Subgraph { id: "SGB".into(), label: "Broker".into(), node_ids: vec!["Q".into(), "C".into()] },
    ];
    let chart = FlowChart { direction: Direction::LeftRight, nodes, edges, subgraphs };
    let result = layout(&chart);
    for sg_box in &result.subgraph_boxes {
        assert!(
            sg_box.width <= result.width,
            "SubgraphBox '{}' width {} > diagram width {}",
            sg_box.label, sg_box.width, result.width
        );
        assert!(
            sg_box.height <= result.height,
            "SubgraphBox '{}' height {} > diagram height {}",
            sg_box.label, sg_box.height, result.height
        );
    }
}

#[test]
fn test_free_nodes_with_subgraphs() {
    use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
    let nodes = vec![
        Node { id: "Free".into(), label: "Free".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "A".into(), label: "A".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "B".into(), label: "B".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "C".into(), label: "C".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "D".into(), label: "D".into(), shape: NodeShape::Rect, node_style: None },
    ];
    let edges = vec![
        Edge { from: "A".into(), to: "B".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "C".into(), to: "D".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "A".into(), to: "C".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
    ];
    let subgraphs = vec![
        Subgraph { id: "SGA".into(), label: "G1".into(), node_ids: vec!["A".into(), "B".into()] },
        Subgraph { id: "SGB".into(), label: "G2".into(), node_ids: vec!["C".into(), "D".into()] },
    ];
    let chart = FlowChart { direction: Direction::LeftRight, nodes, edges, subgraphs };
    let result = layout(&chart);
    // Subgraph SGA members A and B must not interleave with SGB members C and D
    let find_x = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().x;
    let a_x = find_x("A"); let b_x = find_x("B");
    let c_x = find_x("C"); let d_x = find_x("D");
    let sga_max = a_x.max(b_x);
    let sgb_min = c_x.min(d_x);
    let sga_min = a_x.min(b_x);
    let sgb_max = c_x.max(d_x);
    assert!(
        sga_max < sgb_min || sgb_max < sga_min,
        "SGA and SGB x-ranges must not overlap. SGA: [{},{}], SGB: [{},{}]",
        sga_min, sga_max, sgb_min, sgb_max
    );
    // Layout must not panic and must include the free node
    assert_eq!(result.nodes.len(), 5);
}

#[test]
fn test_single_subgraph_no_regression() {
    use crate::mermaid::{Direction, Edge, EdgeStyle, FlowChart, Node, NodeShape, Subgraph};
    let nodes = vec![
        Node { id: "A".into(), label: "A".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "B".into(), label: "B".into(), shape: NodeShape::Rect, node_style: None },
        Node { id: "C".into(), label: "C".into(), shape: NodeShape::Rect, node_style: None },
    ];
    let edges = vec![
        Edge { from: "A".into(), to: "B".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
        Edge { from: "B".into(), to: "C".into(), label: None, style: EdgeStyle::Arrow, edge_style: None },
    ];
    let subgraphs = vec![
        Subgraph { id: "SG".into(), label: "All".into(), node_ids: vec!["A".into(), "B".into(), "C".into()] },
    ];
    let chart = FlowChart { direction: Direction::LeftRight, nodes, edges, subgraphs };
    let result = layout(&chart);
    let find_x = |id: &str| result.nodes.iter().find(|n| n.id == id).unwrap().x;
    // A->B->C chain: x strictly increases
    assert!(find_x("A") < find_x("B"), "A.x should be less than B.x");
    assert!(find_x("B") < find_x("C"), "B.x should be less than C.x");
}
```

- [ ] **Step 2: Run all new tests to confirm they fail**

```bash
cd /home/edgar/mdx && cargo test test_subgraph 2>&1 | tail -30
```

Expected: all 5 tests FAIL.

- [ ] **Step 3: Commit the failing tests**

```bash
cd /home/edgar/mdx && git add src/mermaid/layout.rs && git commit -m "test(layout): add failing tests for compound graph layout"
```

---

### Task 4: Implement `assign_cluster_ranks`

**Files:**
- Modify: `src/mermaid/layout.rs` (before the `layout` function, around line 102)

- [ ] **Step 1: Add the helper function**

Insert this function directly above `pub fn layout(chart: &FlowChart) -> LayoutResult {`:

```rust
/// Returns a rank for each cluster ID (subgraph ID, or "__free__<node_id>" for free nodes).
/// Clusters are ranked by their inter-cluster topology using Kahn + longest-path.
fn assign_cluster_ranks(chart: &FlowChart) -> std::collections::HashMap<String, usize> {
    // Map each node id -> cluster id
    let mut node_to_cluster: std::collections::HashMap<&str, String> =
        std::collections::HashMap::new();
    for sg in &chart.subgraphs {
        for nid in &sg.node_ids {
            node_to_cluster.insert(nid.as_str(), sg.id.clone());
        }
    }
    for node in &chart.nodes {
        node_to_cluster
            .entry(node.id.as_str())
            .or_insert_with(|| format!("__free__{}", node.id));
    }

    // Collect unique cluster ids in stable order
    let mut cluster_ids: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        let mut v = Vec::new();
        // subgraphs first (stable order), then free nodes
        for sg in &chart.subgraphs {
            if seen.insert(sg.id.clone()) {
                v.push(sg.id.clone());
            }
        }
        for node in &chart.nodes {
            let cid = format!("__free__{}", node.id);
            if !node_to_cluster.values().any(|c| c == &sg_id_for(&node_to_cluster, &node.id))
                || node_to_cluster.get(node.id.as_str()).map(|c| c.starts_with("__free__")).unwrap_or(false)
            {
                if seen.insert(cid.clone()) {
                    v.push(cid);
                }
            }
        }
        v
    };

    let cluster_index: std::collections::HashMap<&str, usize> = cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let nc = cluster_ids.len();

    // Build inter-cluster edges (deduplicated)
    let mut inter_edges: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();
    for edge in &chart.edges {
        if let (Some(fc), Some(tc)) = (
            node_to_cluster.get(edge.from.as_str()),
            node_to_cluster.get(edge.to.as_str()),
        ) {
            if fc != tc {
                if let (Some(&fi), Some(&ti)) =
                    (cluster_index.get(fc.as_str()), cluster_index.get(tc.as_str()))
                {
                    inter_edges.insert((fi, ti));
                }
            }
        }
    }

    // Kahn + longest-path on the cluster graph
    let mut in_degree = vec![0usize; nc];
    let mut successors: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(f, t) in &inter_edges {
        successors[f].push(t);
        in_degree[t] += 1;
    }
    let mut cluster_ranks = vec![0usize; nc];
    let mut remaining_in = in_degree.clone();
    let mut processed = vec![false; nc];
    let mut queue = std::collections::VecDeque::new();
    for (i, &deg) in remaining_in.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }
    loop {
        while let Some(idx) = queue.pop_front() {
            processed[idx] = true;
            for &succ in &successors[idx] {
                if !processed[succ] && cluster_ranks[succ] < cluster_ranks[idx] + 1 {
                    cluster_ranks[succ] = cluster_ranks[idx] + 1;
                }
                remaining_in[succ] = remaining_in[succ].saturating_sub(1);
                if remaining_in[succ] == 0 && !processed[succ] {
                    queue.push_back(succ);
                }
            }
        }
        if let Some(i) = (0..nc).find(|&i| !processed[i]) {
            remaining_in[i] = 0;
            queue.push_back(i);
        } else {
            break;
        }
    }

    cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), cluster_ranks[i]))
        .collect()
}

fn sg_id_for<'a>(node_to_cluster: &'a std::collections::HashMap<&str, String>, node_id: &str) -> &'a str {
    node_to_cluster.get(node_id).map(|s| s.as_str()).unwrap_or("")
}
```

- [ ] **Step 2: Compile check**

```bash
cd /home/edgar/mdx && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: may have compile errors — fix any type/lifetime issues before moving on.

---

### Task 5: Rewrite Phase 1 in `layout()` to use cluster-aware ranks

**Files:**
- Modify: `src/mermaid/layout.rs` — the `layout` function, Phase 1 section (lines ~124–176)

- [ ] **Step 1: Replace Phase 1**

Replace the entire block from `// Phase 1: Rank assignment (Kahn's algorithm + longest path)` through the closing `}` of the outer `loop` (current lines 124–176) with:

```rust
    // Phase 1: Cluster-aware rank assignment
    // Step 1a: get cluster rank for each cluster id
    let cluster_rank_map = assign_cluster_ranks(chart);

    // Build node -> cluster id lookup
    let mut node_to_cluster: std::collections::HashMap<&str, String> =
        std::collections::HashMap::new();
    for sg in &chart.subgraphs {
        for nid in &sg.node_ids {
            node_to_cluster.insert(nid.as_str(), sg.id.clone());
        }
    }
    for node in &chart.nodes {
        node_to_cluster
            .entry(node.id.as_str())
            .or_insert_with(|| format!("__free__{}", node.id));
    }

    // Step 1b: per-cluster internal rank via Kahn + longest-path (intra-cluster edges only)
    let mut internal_ranks = vec![0usize; n];

    // Group node indices by cluster
    let mut cluster_members: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, node) in chart.nodes.iter().enumerate() {
        let cid = node_to_cluster[node.id.as_str()].clone();
        cluster_members.entry(cid).or_default().push(i);
    }

    let mut max_internal_depth = 0usize;

    for (cid, members) in &cluster_members {
        if members.len() == 1 {
            internal_ranks[members[0]] = 0;
            continue;
        }
        // Build local index within cluster
        let local_index: std::collections::HashMap<usize, usize> = members
            .iter()
            .enumerate()
            .map(|(li, &gi)| (gi, li))
            .collect();
        let m = members.len();
        let mut in_deg = vec![0usize; m];
        let mut succs: Vec<Vec<usize>> = vec![vec![]; m];
        let mut preds: Vec<Vec<usize>> = vec![vec![]; m];
        for edge in &chart.edges {
            if let (Some(&fi), Some(&ti)) = (
                node_index.get(edge.from.as_str()),
                node_index.get(edge.to.as_str()),
            ) {
                // Only intra-cluster edges
                if node_to_cluster.get(edge.from.as_str()) == Some(cid)
                    && node_to_cluster.get(edge.to.as_str()) == Some(cid)
                {
                    if let (Some(&lf), Some(&lt)) = (local_index.get(&fi), local_index.get(&ti)) {
                        succs[lf].push(lt);
                        preds[lt].push(lf);
                        in_deg[lt] += 1;
                    }
                }
            }
        }
        let mut local_ranks = vec![0usize; m];
        let mut rem = in_deg.clone();
        let mut proc = vec![false; m];
        let mut q = std::collections::VecDeque::new();
        for (i, &d) in rem.iter().enumerate() {
            if d == 0 { q.push_back(i); }
        }
        loop {
            while let Some(idx) = q.pop_front() {
                proc[idx] = true;
                for &s in &succs[idx] {
                    if !proc[s] && local_ranks[s] < local_ranks[idx] + 1 {
                        local_ranks[s] = local_ranks[idx] + 1;
                    }
                    rem[s] = rem[s].saturating_sub(1);
                    if rem[s] == 0 && !proc[s] { q.push_back(s); }
                }
            }
            if let Some(i) = (0..m).find(|&i| !proc[i]) {
                rem[i] = 0; q.push_back(i);
            } else { break; }
        }
        let depth = *local_ranks.iter().max().unwrap_or(&0);
        if depth > max_internal_depth { max_internal_depth = depth; }
        for (li, &gi) in members.iter().enumerate() {
            internal_ranks[gi] = local_ranks[li];
        }
    }

    // Step 1c: combine cluster rank + internal rank into global ranks
    let band_size = max_internal_depth + 1;
    let mut ranks = vec![0usize; n];
    for (i, node) in chart.nodes.iter().enumerate() {
        let cid = &node_to_cluster[node.id.as_str()];
        let cr = cluster_rank_map.get(cid).copied().unwrap_or(0);
        ranks[i] = cr * band_size + internal_ranks[i];
    }

    // build predecessors/successors for Phase 2 barycenter (needed below)
    let mut successors_phase2: Vec<Vec<usize>> = vec![vec![]; n];
    let mut predecessors: Vec<Vec<usize>> = vec![vec![]; n];
    for edge in &chart.edges {
        if let (Some(&from_idx), Some(&to_idx)) = (
            node_index.get(edge.from.as_str()),
            node_index.get(edge.to.as_str()),
        ) {
            successors_phase2[from_idx].push(to_idx);
            predecessors[to_idx].push(from_idx);
        }
    }
```

> **Note:** The old Phase 1 also built `successors` and `predecessors` used in Phase 2. The replacement above builds `predecessors` (same variable name). The old `successors` variable is no longer needed after Phase 1. Rename `successors_phase2` → `successors` in the replacement, or update the Phase 2 reference — see Step 2.

- [ ] **Step 2: Remove the old `successors` variable reference in Phase 2**

Phase 2 (around the current barycenter loop) references `predecessors` and `ranks` — both still exist with the same names. Verify there are no compile errors:

```bash
cd /home/edgar/mdx && cargo build 2>&1 | grep -E "^error" | head -30
```

Fix any `successors` / `predecessors` name conflicts that surface.

---

### Task 6: Extend Phase 2 sort key with cluster rank

**Files:**
- Modify: `src/mermaid/layout.rs` — Phase 2 barycenter sort (around line 218 in the original, now shifted)

- [ ] **Step 1: Update the sort**

Find the line:
```rust
        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));
```

Replace with:

```rust
        barycenters.sort_by(|a, b| {
            let cid_a = node_to_cluster.get(chart.nodes[a.0].id.as_str()).map(|s| s.as_str()).unwrap_or("");
            let cid_b = node_to_cluster.get(chart.nodes[b.0].id.as_str()).map(|s| s.as_str()).unwrap_or("");
            let cr_a = cluster_rank_map.get(cid_a).copied().unwrap_or(0);
            let cr_b = cluster_rank_map.get(cid_b).copied().unwrap_or(0);
            cr_a.cmp(&cr_b)
                .then(a.1.partial_cmp(&b.1).unwrap())
                .then(a.0.cmp(&b.0))
        });
```

- [ ] **Step 2: Compile**

```bash
cd /home/edgar/mdx && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: clean build.

---

### Task 7: Fix `assign_cluster_ranks` — clean up the free-node collection

The version written in Task 4 has a redundant helper `sg_id_for` and awkward free-node collection. Replace the entire `assign_cluster_ranks` function body with this cleaned-up version:

**Files:**
- Modify: `src/mermaid/layout.rs` — `assign_cluster_ranks` function

- [ ] **Step 1: Replace with clean version**

```rust
fn assign_cluster_ranks(chart: &FlowChart) -> std::collections::HashMap<String, usize> {
    // node id -> cluster id
    let mut node_to_cluster: std::collections::HashMap<&str, String> =
        std::collections::HashMap::new();
    for sg in &chart.subgraphs {
        for nid in &sg.node_ids {
            node_to_cluster.insert(nid.as_str(), sg.id.clone());
        }
    }
    for node in &chart.nodes {
        node_to_cluster
            .entry(node.id.as_str())
            .or_insert_with(|| format!("__free__{}", node.id));
    }

    // Stable list of cluster ids: subgraphs first, then free nodes in node order
    let mut seen = std::collections::HashSet::new();
    let mut cluster_ids: Vec<String> = Vec::new();
    for sg in &chart.subgraphs {
        if seen.insert(sg.id.clone()) {
            cluster_ids.push(sg.id.clone());
        }
    }
    for node in &chart.nodes {
        let cid = node_to_cluster[node.id.as_str()].clone();
        if seen.insert(cid.clone()) {
            cluster_ids.push(cid);
        }
    }

    let cluster_index: std::collections::HashMap<String, usize> = cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let nc = cluster_ids.len();

    let mut inter_edges: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();
    for edge in &chart.edges {
        let fc = node_to_cluster.get(edge.from.as_str());
        let tc = node_to_cluster.get(edge.to.as_str());
        if let (Some(fc), Some(tc)) = (fc, tc) {
            if fc != tc {
                if let (Some(&fi), Some(&ti)) =
                    (cluster_index.get(fc), cluster_index.get(tc))
                {
                    inter_edges.insert((fi, ti));
                }
            }
        }
    }

    let mut in_degree = vec![0usize; nc];
    let mut successors: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(f, t) in &inter_edges {
        successors[f].push(t);
        in_degree[t] += 1;
    }
    let mut cluster_ranks = vec![0usize; nc];
    let mut remaining_in = in_degree.clone();
    let mut processed = vec![false; nc];
    let mut queue = std::collections::VecDeque::new();
    for (i, &deg) in remaining_in.iter().enumerate() {
        if deg == 0 { queue.push_back(i); }
    }
    loop {
        while let Some(idx) = queue.pop_front() {
            processed[idx] = true;
            for &succ in &successors[idx] {
                if !processed[succ] && cluster_ranks[succ] < cluster_ranks[idx] + 1 {
                    cluster_ranks[succ] = cluster_ranks[idx] + 1;
                }
                remaining_in[succ] = remaining_in[succ].saturating_sub(1);
                if remaining_in[succ] == 0 && !processed[succ] {
                    queue.push_back(succ);
                }
            }
        }
        if let Some(i) = (0..nc).find(|&i| !processed[i]) {
            remaining_in[i] = 0;
            queue.push_back(i);
        } else {
            break;
        }
    }

    cluster_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), cluster_ranks[i]))
        .collect()
}
```

Also remove the `sg_id_for` helper added in Task 4 — it is no longer needed.

- [ ] **Step 2: Compile**

```bash
cd /home/edgar/mdx && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: clean build.

---

### Task 8: Run all tests

- [ ] **Step 1: Run the full test suite**

```bash
cd /home/edgar/mdx && cargo test 2>&1 | tail -40
```

Expected: all 5 new subgraph tests pass; all existing tests pass.

- [ ] **Step 2: If any test fails, debug**

For a failing test, add a `println!` inside the test to dump node positions, run with:

```bash
cd /home/edgar/mdx && cargo test <test_name> -- --nocapture 2>&1
```

Common failure modes:
- `test_subgraph_members_contiguous_ranks` still fails → `band_size` offset not applied; check `ranks[i] = cr * band_size + internal_ranks[i]`.
- `test_subgraph_secondary_axis_grouping` fails → sort key in Phase 2 not reading `cluster_rank_map`; check the closure captures `cluster_rank_map` and `node_to_cluster`.
- `test_subgraph_box_compact` fails → box wider than canvas → padding constants overflow; check `saturating_sub` in box computation.

- [ ] **Step 3: Commit**

```bash
cd /home/edgar/mdx && git add src/mermaid/layout.rs && git commit -m "feat(layout): compound graph layout — contiguous subgraph rank bands"
```

---

### Task 9: Clippy + fmt

- [ ] **Step 1: Run clippy**

```bash
cd /home/edgar/mdx && cargo clippy -- -D warnings 2>&1 | head -40
```

Fix any warnings (unused variables, needless borrows, etc.).

- [ ] **Step 2: Run fmt**

```bash
cd /home/edgar/mdx && cargo fmt
```

- [ ] **Step 3: Final test run**

```bash
cd /home/edgar/mdx && cargo test 2>&1 | tail -20
```

Expected: all tests pass, zero warnings.

- [ ] **Step 4: Commit**

```bash
cd /home/edgar/mdx && git add src/mermaid/layout.rs && git commit -m "chore(layout): clippy and fmt cleanup for compound graph layout"
```
