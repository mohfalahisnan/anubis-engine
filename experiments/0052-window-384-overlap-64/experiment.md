# Experiment 0052: Window 384 Overlap 64

## Target Priority

- Metric: aqi
- Current: 75.9
- Target: 85
- Gap: 9.1

## Hypothesis

Window 384 gives enough precision, but overlap 128 creates too many chunks and high p95 latency. Returning overlap to 64 should reduce chunks and p95 while the smaller window and reference routing preserve production precision.

## Change Scope

One chunking change:
- Set default sliding overlap from 128 to 64 characters.
- Keep window size, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: should decrease, not increase.
