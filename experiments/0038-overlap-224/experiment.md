# Experiment 0038: Overlap 224

## Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

Overlap 256 reached Precision@10 but violated recall; overlap 192 preserved recall but did not improve precision enough. Overlap 224 may be the safer midpoint: enough extra same-file chunks for precision, less generic-query drift than 256.

## Change Scope

- One chunking change: default overlap 64 -> 224.
- Keep window size 512.
- Keep ranking weights unchanged.
- No benchmark targets relaxed.
- No new dependencies.
