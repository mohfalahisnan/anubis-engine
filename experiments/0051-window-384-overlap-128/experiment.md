# Experiment 0051: Window 384 With Overlap 128

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.40
- Target: 0.45
- Gap: 0.05

## Hypothesis

At overlap 128, AQI passes but several single-document module queries only surface four relevant chunks in top-10. Reducing the content window from 512 to 384 can create more focused chunks from the relevant document without the p95 and invoice crowd-out caused by higher overlap.

## Change Scope

One chunking change:
- Set default sliding window size from 512 to 384 characters.
- Keep overlap, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: protected by rule.
- Edge bloat: possible, only keepable if precision/AQI improves without revert.
