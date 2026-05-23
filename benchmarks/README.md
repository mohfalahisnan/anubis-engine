# Anubis Benchmarks

The benchmark harness is intentionally small and repo-local. It generates a mixed quick corpus, runs the Anubis MCP engine, measures indexing/query behavior, and reports both human-readable and machine-readable results.

## Normal Benchmark

Run:

```bash
npm run benchmark
```

Machine-readable summary:

```bash
node bin/benchmark.js --json
```

Benchmark-only score diagnostics:

```bash
node bin/benchmark.js --debug
```

The benchmark command preserves the existing AQI formula. New metrics are additive so older comparisons remain readable.

## AQI

AQI is the top-line quality index. The current formula is still based on legacy average recall and p95 query latency. Production goals in `benchmarks/goals/production.json` add gates and extra targets, but do not rewrite historical AQI.

Hard gates cap AQI when a result has production-blocking failures:

- Permission leakage caps AQI at 50.
- Critical query failures cap AQI at 80.
- Source coverage below 80% caps AQI at 75.
- Indexing crashes cap AQI at 70.

## Query Status

Each query receives a research status from its benchmark recall/precision pair:

- `strong_pass`: recall >= 0.85 and precision >= 0.45
- `pass`: recall >= 0.75 and precision >= 0.35
- `weak_pass`: recall >= 0.60 and precision >= 0.20
- `fail`: below weak-pass thresholds
- `critical_fail`: recall is 0

This is separate from legacy `PASS`/`FAIL`, which still checks required files and hit presence.

## Added Retrieval Metrics

The harness now emits `Recall@5`, `Recall@10`, `Precision@5`, `Precision@10`, `Top-1 Accuracy`, `Top-3 Accuracy`, `MRR@10`, and `nDCG@10`. Precision is the current priority because the alpha baseline already has strong recall but returns too many noisy top candidates.

## Graph and Indexing Metrics

Graph metrics include total nodes, total edges, edges per chunk, candidate edges, visible edge placeholders, weak/duplicate edge placeholders, and edge evidence coverage. The current engine does not distinguish candidate versus visible graph edges, so existing graph edges are reported as candidate edges and visibility-specific fields are `null`.

Indexing phase timing has a stable shape. The current MCP benchmark only times the full folder indexing call, so unavailable sub-phases are `null` and `totalMs` is measured.
