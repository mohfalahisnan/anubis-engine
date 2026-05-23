# Experiment 0006-gated-graph-boost Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: aqi-jump matched (AQI 74.5 -> 76.7)
- Final decision: keep
- Note: Precision@10 decreased (0.33 -> 0.32), but production rule precedence keeps AQI jumps unless a revert rule matches.

## Before

- AQI: 74.5
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 360 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 76.7
- Gated AQI: 76.7
- Recall@10: 0.98
- Precision@10: 0.32
- p95 latency: 363 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (76.7 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.32 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.98 / 0.75)
- p95LatencyMs: pass (363 / 500)
- p99LatencyMs: pass (363 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
