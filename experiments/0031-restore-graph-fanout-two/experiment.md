# Experiment 0031: Restore Graph Fanout Two

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.50
- Gap: 0.11

## Hypothesis

The active production baseline points at `0027-graph-fanout-two`, but the current query code still expands three graph neighbors per seed. Restoring the fan-out cap to two should prune one extra graph-expanded neighbor per seed, reducing bottom-half top-10 noise while preserving enough graph recall to avoid the recall-regression revert rule.

## Change Scope

One retrieval ranking change:
- Set `EXPANSION_FANOUT` to `2` in `src-tauri/src/query/hybrid.rs`.
- Update only the matching unit test expectation/name for the new fan-out cap.
- Keep scoring weights, graph edge construction, benchmark targets, relevance labels, and query cases unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: unchanged by construction.
- Recall regression: possible if a true positive is only reachable through the third neighbor; protected by revert rule.
- Latency regression: should decrease or remain bounded because fewer graph neighbors are expanded.
- Edge bloat: graph construction unchanged.
