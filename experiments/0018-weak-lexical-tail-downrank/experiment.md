# Experiment 0018: Weak Lexical Tail Downrank

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

The previous zero-evidence tail downrank missed candidates with tiny BM25 crumbs, such as activity-log tails with BM25 around 0.07 and no entity or graph support. Treating those as weakly grounded tails should push them below candidates with stronger direct or graph evidence.

## Change Scope

One retrieval scoring change:

- Keep candidate generation, graph construction, graph expansion, graph boost gates, centrality weight, score weights, and doc cap unchanged.
- Apply a final-score multiplier only when `bm25 < 0.10`, `entity == 0`, and `graph == 0`.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by demoting weak lexical tails that currently occupy bottom-half positions.
- Recall@10 should remain at or above 0.93 because weak-tail candidates remain eligible and graph/entity/direct matches are unaffected.
- p95 latency should not increase because no extra IO, model work, graph traversal, or candidate expansion is introduced.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible if a true positive has only weak BM25 and vector signal, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
