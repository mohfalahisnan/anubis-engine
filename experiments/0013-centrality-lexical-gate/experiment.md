# Experiment 0013: Centrality Lexical Gate

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

The remaining top-10 precision loss includes weak bottom-half candidates that have semantic/vector similarity but little direct lexical or entity evidence. `score_centrality` is graph-derived and query-independent, so allowing vector-only candidates to receive the centrality tie-break can lift graph-connected noise into the tail. Requiring BM25 or entity signal before centrality contributes should downrank those weak tails while preserving strong direct matches.

## Change Scope

One retrieval ranking change:

- Keep vector, BM25, graph, entity weights unchanged.
- Keep graph construction, graph expansion fanout, graph edge threshold, and per-document cap unchanged.
- Change only the centrality tie-break gate so centrality requires BM25 or entity evidence instead of accepting vector-only relevance.
- Do not change benchmark targets, relevance labels, or query cases.

## Expected Delta

- Precision@10 should move toward 0.45 by pushing weak graph-central vector-only candidates below stronger lexical/entity matches.
- Recall@10 should remain at or above 0.93 because vector-only candidates still keep their normal vector score; only the extra centrality tie-break is removed.
- p95 latency should not increase because the candidate pool, SQL queries, graph traversal, and embedding work are unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no indexing or auth behavior changes.
- Recall regression: possible only if true positives rely on vector-only centrality boost, protected by the recall-regression rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
