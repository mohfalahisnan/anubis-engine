# Experiment 0017-disable-centrality-tiebreak Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: aqi-jump matched (AQI 83.1 -> 85.5)
- Final decision: keep
- Baseline update: yes

## Before

- AQI: 83.1
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 85.5
- Gated AQI: 85.5
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 216 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: pass (85.5 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.39 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.97 / 0.7)
- ndcgAt10: pass (0.97 / 0.75)
- p95LatencyMs: pass (216 / 500)
- p99LatencyMs: pass (216 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
