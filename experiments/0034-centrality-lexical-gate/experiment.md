# Experiment 0034: Centrality lexical gate

## Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.50
- Gap: 0.11

## Hypothesis

Precision diagnostics show many bottom-rank false positives are `vector_only`. Query-independent centrality should not lift chunks that have no BM25 or entity evidence for the query. Gating centrality with lexical/entity evidence should downrank vector-only noise while preserving precise lexical hits.

## Change Scope

- One retrieval scoring change: centrality tie-breaker requires BM25/entity signal.
- Keep BM25/vector/graph/entity weights unchanged.
- No benchmark targets relaxed.
- No new dependencies.
