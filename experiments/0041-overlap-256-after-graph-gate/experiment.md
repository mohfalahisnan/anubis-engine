# Experiment 0041: Overlap 256 After Graph Gate

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.37
- Target: 0.45
- Gap: 0.08

## Hypothesis

Earlier overlap-256 produced enough relevant chunks to reach Precision@10 >= 0.45, but it was reverted because recall fell too far from the stronger 0.98 baseline. The current kept baseline is 0.96 Recall@10 and graph boost is stricter, so a 256-character overlap may now clear precision while staying within the recall-regression guard and production recall target.

## Change Scope

One chunking change:
- Set default sliding overlap from 64 to 256 characters.
- Keep window size, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: possible from more chunks, protected by rule.
- Edge bloat: possible, only keepable if precision or other keep rules win without revert rules.
