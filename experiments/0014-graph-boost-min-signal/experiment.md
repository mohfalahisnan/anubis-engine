# Experiment 0014: Graph Boost Minimum Signal

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

After 0006, graph boost requires any BM25 or entity signal, but score breakdowns still show weak bottom-half hits where tiny BM25/entity crumbs unlock a full graph contribution. Requiring a minimum direct signal before graph boost contributes should demote those graph-amplified tails while preserving directly matched chunks and stronger graph-supported answers.

## Change Scope

One retrieval ranking change:

- Keep graph expansion, fanout, edge thresholds, candidate pool size, doc cap, and score weights unchanged.
- Change only the graph contribution gate from `bm25 > 0 || entity > 0` to `bm25 >= 0.10 || entity >= 0.20`.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by pushing weak graph-expanded candidates below candidates with stronger direct query evidence.
- Recall@10 should remain at or above 0.93 because graph-expanded true positives with meaningful direct evidence still keep the graph boost, and all candidates keep their base BM25/vector/entity scores.
- p95 latency should not increase because no additional IO, model work, graph traversal, or candidate expansion is introduced.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible if a true positive relies only on very weak direct evidence plus graph boost, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
