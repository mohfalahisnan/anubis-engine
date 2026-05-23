# Experiment 0004: Newest Engine Binary

## Target Priority

- Metric: retrieval.criticalFailures
- Current: 1
- Target: 0
- Gap: 1

## Hypothesis

The benchmark harness is running a stale `target/release/anubis-engine.exe` before the newer debug binary. That prevents the kept image-sidecar parser change from affecting experiments, so the `img_invoice_02.png` OCR sidecar still appears as an indexed document with zero chunks.

## Change Scope

One harness behavior change:

- When no explicit binary is provided, choose the newest existing engine binary from the standard release/debug candidate list.
- Keep explicit `--bin` and `ANUBIS_ENGINE_BIN` precedence unchanged.
- Do not change benchmark goals, query cases, relevance labels, or scoring thresholds.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures increase: unlikely; harness runs fresher code, not broader data access.
- Recall regression: protected by keep/revert rules.
- Latency regression: protected by keep/revert rules.
- Edge bloat: protected by keep/revert rules.
