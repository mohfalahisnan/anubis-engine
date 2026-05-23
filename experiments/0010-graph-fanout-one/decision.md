# Experiment 0010-graph-fanout-one Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: recall-regression matched (`recallAt10` 0.98 -> 0.93, drop 0.05)
- keepIfAny: not considered after revert precedence
- Final decision: revert
- Baseline update: no
- Trial code: reverted

## Before

- AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 205 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 87.1
- Gated AQI: 87.1
- Recall@10: 0.93
- Precision@10: 0.32
- p95 latency: 137 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: pass (87.1 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: fail (0.32 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.93 / 0.75)
- p95LatencyMs: pass (137 / 500)
- p99LatencyMs: pass (137 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
