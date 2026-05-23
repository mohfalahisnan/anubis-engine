# Experiment 0014-graph-boost-min-signal Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: aqi-jump matched (AQI 81 -> 83.1)
- Final decision: keep
- Baseline update: yes

## Before

- AQI: 81
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 239 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 83.1
- Gated AQI: 83.1
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (83.1 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.38 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (230 / 500)
- p99LatencyMs: pass (230 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
