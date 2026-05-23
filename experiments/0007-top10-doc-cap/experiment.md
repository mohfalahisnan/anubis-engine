# Experiment 0007: Top10 Doc Cap

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.32
- Target: 0.45
- Gap: 0.13

## Hypothesis

After graph-only boost gating, the remaining precision loss is caused by the hard per-document cap forcing weak cross-document hits above stronger same-document chunks. For top-10 search, the cap should allow up to the requested top-10 unless ranking itself chooses other documents.

## Change Scope

One retrieval ranking change:

- Raise `MAX_RESULTS_PER_DOC` from 3 to 10.
- Keep score ordering, graph gating, and overflow fallback unchanged.
- Do not change benchmark targets, relevance labels, query cases, or scoring thresholds.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if diverse relevant docs are displaced, protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
