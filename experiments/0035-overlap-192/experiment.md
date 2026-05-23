# Experiment 0035: Overlap 192

## Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.50
- Gap: 0.11

## Hypothesis

Many single-relevant-file queries already rank every relevant module chunk before noise, but module documents only expose about four chunks. Increasing overlap from 64 to 192 should create one more relevant retrieval window per medium module page, raising Precision@10 without shrinking the window or changing ranking weights.

## Change Scope

- One chunking change: default overlap 64 -> 192.
- Keep window size 512.
- Keep ranking weights unchanged.
- No benchmark targets relaxed.
- No new dependencies.
