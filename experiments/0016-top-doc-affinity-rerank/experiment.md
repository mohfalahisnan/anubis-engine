# Experiment 0016: Top Doc Affinity Rerank

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

Most benchmark queries rank a relevant document first, then lose bottom-half precision to weak cross-document tails. A small rerank bonus for chunks from the top-ranked document should let strong same-document candidates fill more top-10 slots without blocking recall for multi-document queries.

## Change Scope

One retrieval reranking change:

- Keep candidate generation, score weights, graph construction, graph expansion, graph boost gates, and doc cap unchanged.
- Before per-doc diversification, add a small rerank bonus only to candidates from the document that owns the current strongest result.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by replacing weak bottom-half cross-document tails with additional chunks from the strongest answer document.
- Recall@10 should remain at or above 0.93 because the bonus is small and the doc cap still allows other documents into top-10.
- p95 latency should remain under 500 ms because the rerank reuses the existing doc-id lookup already performed by diversification.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible for multi-document queries, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
