# Experiment 0053: Skip Centrality Tiebreak

## Target Priority

- Metric: aqi
- Current: 81.8
- Target: 85
- Gap: 3.2

## Hypothesis

With candidate pool capped and recall/precision passing, p95 latency is the remaining AQI blocker. Centrality is only a query-independent tie-breaker, but it runs an extra graph-edge aggregation for every query. Skipping that tie-breaker should reduce p95 latency without changing core BM25/vector/entity/graph relevance.

## Change Scope

One retrieval performance change:
- Do not populate centrality during hybrid search.
- Keep chunking, candidate generation, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if centrality was rescuing a required hit, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: graph construction unchanged.
