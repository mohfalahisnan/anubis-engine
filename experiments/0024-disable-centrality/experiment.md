# Experiment 0024: Disable Centrality

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

Centrality is a query-independent signal (how connected this chunk is in the global graph).
Even with relevance gating, it adds up to 0.05 score boost to generic "hub" chunks. This can push these hubs above more precise, query-relevant chunks in the top-10.
Disabling the centrality tiebreaker entirely (`W_CENTRALITY_TIEBREAK = 0.0`) will prevent query-independent hub boosting, improving Precision@10 on top of the scaled graph boost baseline.

## Change Scope

One retrieval ranking change:
- Set `W_CENTRALITY_TIEBREAK` to `0.0` in `src-tauri/src/query/hybrid.rs`.
- Keep graph expansion, fanout, and all other weights unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if some relevant hubs are demoted too much, protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
