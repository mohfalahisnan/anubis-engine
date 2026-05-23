# Experiment 0019: Candidate Pool Overfetch

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

With the per-document cap opened to 10, precision may still be limited by the small candidate pool. Overfetching BM25/vector/entity candidates before reranking can expose more relevant same-document chunks and direct-evidence candidates, letting existing scoring displace weak bottom-half tails.

## Change Scope

One retrieval candidate-pool change:

- Increase the hybrid candidate pool from `limit * 4` minimum 20 to `limit * 8` minimum 40.
- Keep scoring weights, graph construction, graph expansion, graph boost gates, centrality weight, and doc cap unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve if relevant chunks were previously absent from the scoring pool.
- Recall@10 should remain at or above 0.93 because the candidate pool only expands.
- p95 latency should remain under 500 ms; the quick corpus is small, but the latency rule protects this.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: unlikely because this only expands candidates, protected by the recall-regression rule.
- Latency regression: possible from larger candidate sets, protected by the latency rule.
- Edge bloat: graph construction unchanged.
