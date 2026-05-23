# Experiment 0025: Prune Weak Edges

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

Increasing `STRONG_EDGE_THRESHOLD` from `0.62` to `0.70` will prune weak graph edges during traversal, preventing weakly connected neighbor chunks from receiving graph boosts and rising to the top-10. This should improve Precision@10.

## Change Scope

One retrieval ranking change:
- Raise `STRONG_EDGE_THRESHOLD` from `0.62` to `0.70` in `src-tauri/src/query/hybrid.rs`.
- Keep graph expansion, fanout, and all other weights/logic unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if some relevant neighbor chunks are no longer reached, protected by rule.
- Latency regression: should decrease or remain flat since we traverse fewer edges.
- Edge bloat: graph construction unchanged.
