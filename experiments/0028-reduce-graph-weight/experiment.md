# Experiment 0028: Reduce Graph Weight

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

Graph-expanded chunks with weak direct matches are still finding their way into the bottom-half of the top-10.
Shifting 0.05 weight from `W_GRAPH` to `W_BM25` (making `W_GRAPH = 0.10` and `W_BM25 = 0.40`) will reduce the maximum possible graph boost while increasing the importance of direct lexical matches. This should push noisy graph-expanded hits down, improving Precision@10.

## Change Scope

One retrieval scoring-weight change:
- Set `W_GRAPH` to `0.10` in `src-tauri/src/query/hybrid.rs`.
- Set `W_BM25` to `0.40` in `src-tauri/src/query/hybrid.rs`.
- Keep graph expansion, fanout (2), doc cap, and other weights unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if true positives rely heavily on graph boost and have weak BM25, protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
