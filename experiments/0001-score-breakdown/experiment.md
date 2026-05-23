# Experiment 0001: Score Breakdown and Query Status

## Goal

Make Anubis benchmark results inspectable before optimizing ranking.

## Hypothesis

The current system has strong recall but weak precision because noisy candidates are ranked too high. Before changing ranking, we need result-level score breakdown and query-level classification.

## Change

Add:
- Query status classification
- Score breakdown for top results in benchmark/debug mode
- Ranking metrics: Top-1, Top-3, MRR@10, nDCG@10
- Graph density metrics
- Indexing phase timing placeholders or real measurements where possible

## Success Criteria

- Existing benchmark still runs.
- Existing AQI can still be reported.
- Each query has status.
- Top results can show score breakdown in debug report.
- No behavior change to normal retrieval ranking.
- No meaningful query latency regression.

## Decision Rule

Keep if benchmark output becomes more inspectable and no existing structural assertion regresses.
