# Experiment 0008: Focused Chunk Window

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.32
- Target: 0.45
- Gap: 0.13

## Hypothesis

The default 512-character chunk window is too broad for the quick corpus. Smaller chunks should create more focused passages, reduce mixed-topic matches, and increase Precision@10 while preserving recall through overlap.

## Change Scope

One chunking change:

- Reduce `DEFAULT_WINDOW_SIZE` from 512 to 256.
- Keep overlap, min chunk size, scoring, graph construction, benchmark labels, and benchmark targets unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible but protected by rule; overlap unchanged.
- Latency regression: possible from more chunks, protected by p95 rule.
- Edge bloat: possible from more chunks/edges; precision must improve to justify it.
