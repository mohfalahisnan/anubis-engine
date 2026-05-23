# Experiment 0045: Candidate Pool 10

## Target Priority

- Metric: aqi
- Current: 76.9
- Target: 85
- Gap: 8.1

## Hypothesis

The reduced candidate pool still bottoms out at 20 candidates for a top-10 query. With overlap and reference-routed candidates now supplying enough relevant chunks, lowering the minimum pool to 10 should reduce p95 latency while keeping recall and precision above productionV1 targets.

## Change Scope

One retrieval performance change:
- Change hybrid search pool size from `(limit * 2).max(20)` to `limit.max(10)`.
- Keep chunking, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if fewer candidates hide required hits, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: graph construction unchanged.
