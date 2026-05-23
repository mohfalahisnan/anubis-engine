# Experiment 0039: Manifest Overlap Expansion

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.38
- Target: 0.45
- Gap: 0.07

## Hypothesis

The quick suite has generic listing queries where a reference/listing document carries useful evidence-backed links to the primary content documents. Graph expansion currently ignores `manifest_overlap`, so reference evidence can be measured but not used for retrieval. Allowing graph expansion to follow strong `manifest_overlap` edges should boost content documents backed by manifest evidence without changing global scoring weights or benchmark expectations.

## Change Scope

One retrieval change:
- Include `manifest_overlap` in the graph expansion edge-type allowlist.
- Keep graph threshold, fan-out, scoring weights, chunking, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if manifest expansion adds noise, protected by rule.
- Latency regression: possible from extra graph candidates, protected by rule.
- Edge bloat: graph construction unchanged.
