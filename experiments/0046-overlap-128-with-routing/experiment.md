# Experiment 0046: Overlap 128 With Routing

## Target Priority

- Metric: aqi
- Current: 77.8
- Target: 85
- Gap: 7.2

## Hypothesis

Overlap-256 supplies high precision but creates too many chunks and slow p95 latency. Reference-routed candidates now protect the listing query, so reducing overlap to 128 should cut chunk count and latency while preserving production Precision@10 and Recall@10.

## Change Scope

One chunking change:
- Set default sliding overlap from 256 to 128 characters.
- Keep window size, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: should decrease, not increase.
