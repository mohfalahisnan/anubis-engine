# Experiment 0005-doc-cap-precision Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (`precisionAt10` +0.03, below +0.05; AQI decreased)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Before

- AQI: 74.5
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 360 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 72.9
- Gated AQI: 72.9
- Recall@10: 0.96
- Precision@10: 0.36
- p95 latency: 360 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (72.9 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.36 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.96 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (360 / 500)
- p99LatencyMs: pass (360 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
