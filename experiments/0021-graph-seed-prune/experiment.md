# Experiment 0021: Graph Seed Prune

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

Graph fanout is already capped, but expansion still starts from ten seeds. Lower-ranked seeds are more likely to be weak or noisy, so reducing the seed count should prune graph-expanded tail candidates while preserving graph support from the strongest direct matches.

## Change Scope

One graph retrieval change:

- Reduce graph expansion seed count from 10 to 5.
- Keep fanout, edge thresholds, scoring weights, centrality weight, graph construction, candidate pool size, and doc cap unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by reducing graph-expanded noise from weaker seeds.
- Recall@10 should remain at or above 0.93 because top seeds still expand and all direct candidates remain.
- p95 latency should not increase because less graph traversal is performed.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible if true positives depend on lower-ranked graph seeds, protected by the recall-regression rule.
- Latency regression: unlikely; less graph traversal work.
- Edge bloat: graph construction unchanged.
