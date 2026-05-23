# Experiment 0040: Graph Boost Evidence Gate

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.37
- Target: 0.45
- Gap: 0.08

## Hypothesis

Precision diagnostics show graph-assisted false positives can get a full graph boost even when direct query evidence is tiny. Requiring a modest BM25 or entity signal before applying the graph boost should keep graph traversal evidence-aware and reduce noisy expanded candidates without changing global weights, chunking, benchmark expectations, or relevance labels.

## Change Scope

One retrieval scoring change:
- Apply graph boost only when `max(score_bm25, score_entity)` reaches the existing relevance gate.
- Keep graph expansion, graph edge construction, scoring weights, chunking, and benchmark targets unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if graph-only recovery was masking a miss, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely, scoring-only change.
- Edge bloat: graph construction unchanged.
