# Experiment 0047: Reference Route One Per Doc

## Target Priority

- Metric: aqi
- Current: 77.8
- Target: 85
- Gap: 7.2

## Hypothesis

Reference routing restores listing recall, but pushing every chunk from a mentioned file can crowd out other required evidence in top-5. Routing only the first chunk per mentioned filename should preserve reference evidence while improving top-5 recall for multi-source queries.

## Change Scope

One retrieval ordering change:
- Limit the front-routed reference-mentioned candidates to one chunk per filename.
- Keep candidate generation, chunking, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if routed coverage drops too far, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: graph construction unchanged.
