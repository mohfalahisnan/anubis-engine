# Experiment 0043: Reference Mentioned Document Rerank

## Target Priority

- Metric: aqi
- Current: 74
- Target: 85
- Gap: 11

## Hypothesis

AQI is mostly held down by the generic `active module listing` query missing all relevant docs at top-5. Reference documents are intentionally downranked, but they can still carry routing evidence by naming the primary content files. Prioritizing candidate chunks whose filenames are mentioned by a matching reference chunk should restore listing-query recall without showing the reference document itself.

## Change Scope

One retrieval ordering change:
- After final scoring, move candidate chunks from content documents named inside matching reference chunks ahead of other equally scored candidates.
- Keep scoring weights, chunking, graph construction, benchmark targets, and relevance labels unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: possible if reference routing displaces exact hits, protected by rule.
- Recall regression: possible, protected by rule.
- Latency regression: small extra DB lookups, protected by rule.
- Edge bloat: graph construction unchanged.
