# Experiment 0032: Smaller Content Window

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.50
- Gap: 0.11

## Hypothesis

Score breakdowns show many single-document relevance cases place only three or four chunks from the correct module before top-10 fills with vector-near log/audit noise. Reducing the default sliding window from 512 to 384 characters should create more focused chunks from the same relevant content documents, allowing more true-positive filename hits to occupy top-10 without changing ranking weights or benchmark expectations.

## Change Scope

One chunking change:
- Set `DEFAULT_WINDOW_SIZE` to `384` in `src-tauri/src/chunker/sliding.rs`.
- Keep overlap, minimum chunk size, scoring, graph expansion, graph construction, benchmark targets, relevance labels, and query cases unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible if chunk boundaries lose context, protected by rule.
- Latency regression: possible from more chunks, protected by rule.
- Edge bloat: possible from more chunks/edges; this change is only keepable if precision or AQI improves enough to justify it.
