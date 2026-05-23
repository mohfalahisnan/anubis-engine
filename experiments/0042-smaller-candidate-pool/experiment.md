# Experiment 0042: Smaller Candidate Pool

## Target Priority

- Metric: aqi
- Current: 72.6
- Target: 85
- Gap: 12.4

## Hypothesis

After overlap-256, Precision@10 passes but AQI is capped by query latency. The retrieval pool currently asks vector and BM25 for 40 candidates for a top-10 query. Reducing the candidate pool to 20 should cut query latency while the larger overlap still supplies enough relevant chunks to preserve Precision@10 and Recall@10.

## Change Scope

One retrieval performance change:
- Change hybrid search pool size from `limit * 4` to `limit * 2`, keeping the minimum pool at 20.
- Keep chunking, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if fewer candidates hide required hits, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: graph construction unchanged.
