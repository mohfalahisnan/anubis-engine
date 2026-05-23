# Experiment 0009: Graph Fanout Prune

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.32
- Target: 0.45
- Gap: 0.13

## Hypothesis

Graph expansion fanout of 8 pulls too many weak/noisy neighbors into the bottom half of top-10 results and adds query work. Reducing fanout to 3 should prune graph noise and may improve Precision@10 and latency without hurting recall.

## Change Scope

One graph retrieval change:

- Reduce `EXPANSION_FANOUT` from 8 to 3.
- Keep graph edge thresholds, scoring weights, benchmark targets, query cases, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if true positives only arrive via lower-ranked graph neighbors; protected by rule.
- Latency regression: unlikely; less graph traversal work.
- Edge bloat: graph construction unchanged.
