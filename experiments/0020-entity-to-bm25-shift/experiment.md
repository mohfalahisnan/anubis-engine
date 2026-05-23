# Experiment 0020: Entity to BM25 Shift

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

Entity matches are still lifting weak cross-document tails, especially when generic entity tokens appear across operational files. Moving half of the entity weight into BM25 should favor direct lexical evidence while preserving exact/entity signal as a smaller boost.

## Change Scope

One retrieval scoring-weight change:

- Shift `W_ENTITY` from 0.10 to 0.05.
- Shift `W_BM25` from 0.35 to 0.40 so the base weights still sum to 1.0.
- Keep vector, graph, centrality, graph boost gates, candidate generation, graph construction, graph expansion, and doc cap unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by reducing entity-driven tail inflation and promoting stronger lexical matches.
- Recall@10 should remain at or above 0.93 because entity matches still contribute and BM25 gains weight.
- p95 latency should not increase because scoring weights do not add work.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible if a true positive depends heavily on entity boost, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
