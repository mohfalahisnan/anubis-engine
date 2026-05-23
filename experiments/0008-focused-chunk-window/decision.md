# Experiment 0008-focused-chunk-window Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: latency-regression matched (`p95LatencyMs` 1281 > 500)
- keepIfAny: none considered after revert precedence
- Final decision: revert
- Baseline update: no
- Trial code: reverted

## Before

- AQI: 76.7
- Recall@10: 0.98
- Precision@10: 0.32
- p95 latency: 363 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 63
- Gated AQI: 63
- Recall@10: 0.93
- Precision@10: 0.31
- p95 latency: 1281 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (63 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: fail (0.31 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.92 / 0.75)
- p95LatencyMs: fail (1281 / 500)
- p99LatencyMs: fail (1281 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
