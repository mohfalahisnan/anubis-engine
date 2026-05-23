# Experiment 0054: Skip Graph Expansion

## Target Priority

- Metric: aqi
- Current: 82.4
- Target: 85
- Gap: 2.6

## Hypothesis

After reference routing, quick-suite recall no longer needs runtime graph expansion for top-10 recovery. Skipping graph expansion should remove per-query graph neighbor work and reduce p95 below the AQI threshold while BM25/vector/entity/reference routing preserve recall and precision.

## Change Scope

One retrieval performance change:
- Disable runtime graph expansion in hybrid search.
- Keep chunking, candidate generation, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if graph expansion was required for a query, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: graph construction unchanged.
