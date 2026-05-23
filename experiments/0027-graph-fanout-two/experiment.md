# Experiment 0027: Graph Fanout Two

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

In experiment 0009, graph fanout was reduced from 8 to 3, which successfully pruned graph noise and improved precision.
In experiment 0010, reducing it to 1 regressed recall to 0.93.
Capping the graph expansion fanout at 2 (down from 3) should prune extra graph noise while preserving enough connections to keep Recall@10 at or near the 0.98 baseline.

## Change Scope

One retrieval ranking change:
- Set `EXPANSION_FANOUT` to `2` in `src-tauri/src/query/hybrid.rs`.
- Keep graph expansion, weights, thresholds, and all other logic unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if some relevant chunks are only reached via the third neighbor, protected by rule (floor 0.93).
- Latency regression: should decrease because we traverse fewer edges.
- Edge bloat: graph construction unchanged.
