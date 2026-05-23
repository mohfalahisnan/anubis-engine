# Experiment 0033-evidence-backed-graph-expansion Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched (permission leakage 0, critical failures 0, recall drop 0.02, p95 245 ms)
- keepIfAny: none matched (Precision@10 decreased 0.39 -> 0.37; AQI decreased 87.9 -> 80.6)
- edge-bloat: not matched (edge count stayed 29340)
- Final decision: needs_more_data
- Baseline update: no
- Trial graph-expansion code: not promoted

## Diagnostic Finding

Precision diagnostics now show false-positive profiles per query. Dominant remaining noise is mostly `lexical_entity` and `vector_only`; strict evidence-backed graph expansion hurt top3/AQI and did not improve precision.

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 80.6
- Gated AQI: 80.6
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 245 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (80.6 / 95)
- recallAt10: pass (0.96 / 0.95)
- precisionAt10: fail (0.37 / 0.5)
- top3Accuracy: fail (0.93 / 0.95)
- mrrAt10: pass (0.94 / 0.9)
- ndcgAt10: pass (0.94 / 0.9)
- p95LatencyMs: pass (245 / 500)
- p99LatencyMs: pass (245 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
