# Experiment 0006: Gated Graph Boost

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.33
- Target: 0.45
- Gap: 0.12

## Hypothesis

Graph expansion is boosting vector-only neighbors that have no lexical or entity evidence for the query. Those chunks are visible in score breakdowns as `bm25=0`, `entity=0`, high `graph`, and they fill the bottom half of top-10 results.

## Change Scope

One ranking change:

- Keep graph expansion in the candidate pool.
- Apply the graph score contribution only when a chunk also has BM25 or entity signal.
- Do not change graph construction, benchmark labels, benchmark targets, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if graph-only hits carried true positives, protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
