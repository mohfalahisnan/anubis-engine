# Experiment 0049: Reference Route Two Per Doc

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.40
- Target: 0.45
- Gap: 0.05

## Hypothesis

Routing one chunk per reference-mentioned filename restored top-5 recall but left precision below target. Routing two chunks per mentioned filename should improve precision for listing and graph queries while avoiding the unlimited-routing crowd-out that caused the earlier invoice critical failure.

## Change Scope

One retrieval ordering change:
- Raise the front-routed cap for reference-mentioned files from one chunk per filename to two.
- Keep candidate generation, chunking, scoring weights, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if routed chunks crowd out exact evidence, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: unlikely.
- Edge bloat: graph construction unchanged.
