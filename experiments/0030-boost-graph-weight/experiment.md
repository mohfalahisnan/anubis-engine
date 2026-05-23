# Experiment 0030: Boost Graph Weight

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

In Experiment 0028, reducing the graph weight `W_GRAPH` to `0.10` and increasing `W_BM25` to `0.40` regressed Precision@10 (down to 0.37) and AQI (down to 83.3). This suggests that graph-based scores are a key positive signal for precision. Conversely, dense vector matches introduce significant noise (e.g. log/audit files in query results) when `W_VEC` is high.

Shifting 0.05 weight from `W_VEC` to `W_GRAPH` (making `W_VEC = 0.35` and `W_GRAPH = 0.20`):
1. Reduces vector-only noise (e.g. `inventory_audit.json` appearing on `payment reconciliation ledger` query).
2. Increases the boost for graph-expanded true positives (e.g. shipping, billing, and inventory modules in `active module listing` query which are linked to the master README).

This should push true positive modules higher in the ranking while suppressing false positive vector matches, thereby improving average retrieval precision.

## Change Scope

One retrieval scoring-weight change:
- Set `W_VEC` to `0.35` in `src-tauri/src/query/hybrid.rs`.
- Set `W_GRAPH` to `0.20` in `src-tauri/src/query/hybrid.rs`.
- Keep graph expansion, fanout (2), doc cap, and all other weights/logic unchanged.
- Update unit test assertions in `hybrid.rs` to match the new scoring weights.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: low risk since W_VEC is still 0.35 and W_GRAPH is increased.
- Latency: no extra IO/computation.
