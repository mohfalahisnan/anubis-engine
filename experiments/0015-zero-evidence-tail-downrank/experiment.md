# Experiment 0015: Zero Evidence Tail Downrank

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

After the graph boost gate, the lowest-precision queries still show bottom-half results that are vector-only: no BM25, no entity match, and no graph evidence. These candidates are semantically nearby but weakly grounded in the query. Downranking only zero-evidence vector tails should let candidates with any direct or graph evidence outrank them without affecting strong first-page matches.

## Change Scope

One retrieval ranking change:

- Keep candidate generation, graph construction, graph expansion, graph boost gates, score weights, and doc cap unchanged.
- Apply a score multiplier only when a candidate has `bm25 == 0`, `entity == 0`, and `graph == 0`.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should improve by moving ungrounded vector-only tails below candidates with at least one query-linked signal.
- Recall@10 should remain at or above 0.93 because vector-only candidates remain eligible; only their final score is reduced when they have no other evidence.
- p95 latency should not increase because no extra IO, model work, graph traversal, or candidate expansion is introduced.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible if a required true positive has only vector signal, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
