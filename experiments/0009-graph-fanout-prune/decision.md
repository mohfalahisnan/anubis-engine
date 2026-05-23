# Experiment 0009-graph-fanout-prune Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: aqi-jump matched (AQI 76.7 -> 83.1)
- Final decision: keep
- Baseline update: yes

## Before

- AQI: 76.7
- Recall@10: 0.98
- Precision@10: 0.32
- p95 latency: 363 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 83.1
- Gated AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 205 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (83.1 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.33 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.95 / 0.75)
- p95LatencyMs: pass (205 / 500)
- p99LatencyMs: pass (205 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
