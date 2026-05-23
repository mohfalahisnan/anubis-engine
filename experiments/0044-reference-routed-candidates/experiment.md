# Experiment 0044: Reference Routed Candidates

## Target Priority

- Metric: aqi
- Current: 73.2
- Target: 85
- Gap: 11.8

## Hypothesis

The reference rerank helped only when reference chunks were already in the candidate pool. For generic listing queries, the reference document can be outside the reduced pool even though it contains exact query terms and names the primary files. Scanning indexed reference chunks for all query terms, adding matching content chunks from mentioned filenames, then applying the existing reference rerank should restore listing recall and improve AQI without changing benchmark expectations.

## Change Scope

One retrieval routing change:
- Add candidate chunks from content documents named by query-matching reference chunks.
- Keep scoring weights, chunking, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if routed candidates displace exact hits, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: small extra DB scan over reference chunks, protected by rule.
- Edge bloat: graph construction unchanged.
