# Experiment 0012-doc-cap-after-fanout Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: precision-jump matched (Precision@10 0.33 -> 0.38)
- Final decision: keep
- Baseline update: yes

## Before

- AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 205 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 81
- Gated AQI: 81
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 239 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (81 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.38 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (239 / 500)
- p99LatencyMs: pass (239 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
