# Experiment 0026: Square Graph Scaling

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

In experiment 0023, we successfully scaled the graph boost by direct query relevance (`s.score_vec.max(s.score_bm25)`), which improved AQI and Precision@10 by suppressing weak graph-expanded noise.
However, score breakdowns still show that moderately relevant candidates (direct relevance ~0.4 - 0.6) can get a significant graph boost that pushes them into the top-10.
By squaring the direct query relevance scaling factor (applying `rel * rel`), we can apply a non-linear penalty: strongly relevant candidates (e.g. relevance 0.9) keep 81% of their graph boost, while moderately relevant candidates (e.g. relevance 0.4) have their graph boost cut to 16%. This should further suppress moderately matching graph noise and improve retrieval precision.

## Change Scope

One retrieval ranking change:
- Scale `score_graph` by `rel * rel` (where `rel = s.score_vec.max(s.score_bm25)`) when calculating `graph_score` in `final_score`.
- Keep graph expansion, fanout, document cap (10), and weights unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if some relevant graph-expanded chunks have moderate direct relevance, protected by rule (floor 0.93, max 0.03 drop).
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
