# Experiment 0023: Scale Graph Boost

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

Score breakdowns show that the bottom-half of top-10 results is filled with graph-only or entity-only hits that have very weak BM25 + vector signal.
By scaling the graph relation boost by the chunk's direct text/semantic relevance (`s.score_vec.max(s.score_bm25)`), we can smoothly downrank graph-expanded candidates that lack strong direct direct evidence for the query, thereby improving retrieval precision.

## Change Scope

One retrieval ranking change:
- Scale `score_graph` by `s.score_vec.max(s.score_bm25)` when calculating `graph_score` in `final_score`.
- Keep graph expansion, fanout, document cap (10), and weights unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if true positives rely entirely on graph boost with very low direct relevance, protected by rule (max 0.03 drop, floor 0.93).
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
