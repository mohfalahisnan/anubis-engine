# Experiment 0036: Trusted graph metrics

## Priority

- Metric: grounding.edgeEvidenceCoverage
- Current: 0.04
- Target: 0.60
- Gap: 0.56

## Hypothesis

The productionV1 contract distinguishes candidate graph edges from trusted/visible graph edges. Candidate semantic edges may remain evidence-light; trusted/visible edges must have literal evidence. The benchmark should report visible/trusted edges separately and compute evidence coverage over that visible set.

## Change Scope

- Benchmark/harness change only.
- `graphMetrics.visibleEdges` = evidence-backed edge types.
- `graphMetrics.edgeEvidenceCoverage` = evidence coverage over visible edges.
- Experiment result decision metrics include `edgeEvidenceCoverage` and `visibleEdgesPerNode`.
- No retrieval scoring or engine behavior change.
- No benchmark target relaxed.
