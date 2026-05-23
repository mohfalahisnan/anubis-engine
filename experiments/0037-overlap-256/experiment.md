# Experiment 0037: Overlap 256

## Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

Medium module pages expose too few relevant windows for single-file queries. Overlap 192 improved Precision@10 but not enough; overlap 256 should create more relevant same-file chunks while keeping window size 512 and p95 latency under 500 ms.

## Change Scope

- One chunking change: default overlap 64 -> 256.
- Keep window size 512.
- Keep ranking weights unchanged.
- No benchmark targets relaxed.
- No new dependencies.
