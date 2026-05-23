# Experiment 0033: Evidence-backed graph expansion

## Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.50
- Gap: 0.11

## Hypothesis

False positives in ranks 4-10 are often graph-assisted or weak vector-only chunks. Graph expansion should only add neighbors connected by literal evidence-backed shared anchor/entity edges. Semantic similarity edges stay available for graph views, but no longer expand retrieval candidates.

## Change Scope

- Add precision false-positive diagnostics to benchmark JSON when debug is enabled.
- Restrict retrieval graph expansion to `shared_anchor` edges with `anchor:*` reason and `shared_entity` edges with literal reason.
- No benchmark targets relaxed.
- No new dependencies.
