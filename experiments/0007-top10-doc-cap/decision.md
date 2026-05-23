# Experiment 0007-top10-doc-cap Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (`precisionAt10` +0.03, below +0.05; AQI decreased)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Before

- AQI: 76.7
- Recall@10: 0.98
- Precision@10: 0.32
- p95 latency: 363 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 74.8
- Gated AQI: 74.8
- Recall@10: 0.96
- Precision@10: 0.35
- p95 latency: 368 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (74.8 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.35 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.97 / 0.7)
- ndcgAt10: pass (0.95 / 0.75)
- p95LatencyMs: pass (368 / 500)
- p99LatencyMs: pass (368 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
