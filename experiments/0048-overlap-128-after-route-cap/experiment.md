# Experiment 0048: Overlap 128 After Route Cap

## Target Priority

- Metric: aqi
- Current: 79.4
- Target: 85
- Gap: 5.6

## Hypothesis

Reference routing now preserves top-5 recall without flooding one mentioned file. Retrying 128-character overlap should cut p95 latency while keeping critical failures at zero and preserving production Precision@10.

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
