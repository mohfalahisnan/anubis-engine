# Experiment 0012: Doc Cap After Fanout

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.33
- Target: 0.45
- Gap: 0.12

## Hypothesis

The previous top-10 document cap attempt ran before graph fanout pruning. With graph fanout now reduced to 3, allowing same-document chunks to occupy top-10 slots should improve precision without reintroducing as much graph noise.

## Change Scope

One retrieval ranking change:

- Raise `MAX_RESULTS_PER_DOC` from 3 to 10 on top of the kept fanout-3 baseline.
- Keep graph fanout, graph gating, score weights, benchmark targets, query cases, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible for multi-document queries, protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
