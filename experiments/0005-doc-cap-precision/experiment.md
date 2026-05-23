# Experiment 0005: Doc Cap Precision

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.33
- Target: 0.45
- Gap: 0.12

## Hypothesis

The per-document diversification cap is too low. With `MAX_RESULTS_PER_DOC = 3`, strong chunks from the relevant module are displaced by weaker cross-document graph/vector hits, limiting many one-document queries to Precision@10 near 0.30.

## Change Scope

One retrieval ranking change:

- Raise the default per-document cap from 3 to 5.
- Keep diversification behavior and overflow fallback otherwise unchanged.
- Do not change benchmark targets, relevance labels, query cases, or scoring thresholds.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected; invoice query currently fixed.
- Recall regression: unlikely by construction, but protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
