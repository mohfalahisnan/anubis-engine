# Experiment 0029: Lexical Coverage Gating

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

Vector-only matches (e.g. log files like `activity_log.csv` or JSON structures like `inventory_audit.json`) with zero word or entity overlap are leaking into the top-10 search results. In the current dataset, every true positive has at least some direct lexical overlap (high BM25 or entity match) because the query keywords or exact anchors exist in the target runbooks/sidecars.

By introducing a `signal_multiplier` based on direct lexical and entity coverage (`max_signal = score_bm25.max(score_entity)`), we can apply a non-linear penalty to chunks lacking keyword/entity overlap:
- Chunks with lexical/entity overlap (`max_signal > 0.0`): `signal_multiplier = 0.5 + 0.5 * max_signal`.
- Chunks with ZERO lexical and entity overlap (`max_signal == 0.0`): `signal_multiplier = 0.2` (80% penalty).

This will crush the scores of pure vector-only false positives while protecting the true positives.

## Change Scope

- Update `final_score` in `src-tauri/src/query/hybrid.rs` to compute and apply the `signal_multiplier` to the `base` score.
- Keep other parameters, weights, and logic unchanged.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected.
- Recall regression: very low risk, since true positives are confirmed to have high BM25 / entity signals.
- Latency regression: no extra database or network queries.
