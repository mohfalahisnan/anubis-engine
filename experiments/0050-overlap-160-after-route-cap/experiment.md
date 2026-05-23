# Experiment 0050: Overlap 160 After Route Cap

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.40
- Target: 0.45
- Gap: 0.05

## Hypothesis

Overlap 128 restores AQI but leaves too few repeated relevant chunks for Precision@10. A moderate 160-character overlap may recover enough precision while keeping p95 latency near budget and preserving zero critical failures.

## Change Scope

One chunking change:
- Set default sliding overlap from 128 to 160 characters.
- Keep window size, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: protected by rule.
- Edge bloat: possible, only keepable if precision/AQI improves without revert.
