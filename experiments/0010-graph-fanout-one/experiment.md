# Experiment 0010: Graph Fanout One

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.33
- Target: 0.45
- Gap: 0.12

## Hypothesis

Graph fanout 3 still contributes bottom-half noise. Keeping only the strongest graph neighbor per seed should further prune noisy graph expansion and improve Precision@10 or latency without losing recall.

## Change Scope

One graph retrieval change:

- Reduce `EXPANSION_FANOUT` from 3 to 1.
- Keep graph thresholds, scoring weights, benchmark targets, query cases, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if true positives need second/third graph neighbor; protected by rule.
- Latency regression: unlikely; less graph traversal work.
- Edge bloat: graph construction unchanged.
