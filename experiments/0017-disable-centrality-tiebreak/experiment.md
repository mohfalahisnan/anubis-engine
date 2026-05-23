# Experiment 0017: Disable Centrality Tiebreak

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

The centrality tie-break is query-independent, so even when gated it can lift graph-connected but weakly relevant chunks into the lower half of the top-10. Removing the additive centrality bonus should reduce hub/tail noise while preserving BM25, vector, entity, and query-dependent graph scores.

## Change Scope

One retrieval scoring change:

- Set the centrality tie-break weight to zero.
- Keep centrality calculation, candidate generation, graph construction, graph expansion, graph boost gates, score weights, and doc cap otherwise unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by removing query-independent lift from weak tails.
- Recall@10 should remain at or above 0.93 because candidates keep their BM25/vector/entity/graph scores and centrality is only an additive tie-break.
- p95 latency should not increase because no new work is introduced.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible if true positives rely only on centrality ordering, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
