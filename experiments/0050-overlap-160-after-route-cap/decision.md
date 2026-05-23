# Experiment 0050-overlap-160-after-route-cap Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: `critical-failures-increase` matched (0 -> 1)
- keepIfAny: ignored because revert has precedence
- Trial code: reverted
- Baseline update: no

## Before

- AQI: 86.1
- Recall@10: 1
- Precision@10: 0.4
- p95 latency: 231 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.16

## After

- AQI: 80.4
- Gated AQI: 80
- Recall@10: 1
- Precision@10: 0.41
- p95 latency: 249 ms
- Critical failures: 1
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 2.94

## Production Checks

- aqi: fail (80.4 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: fail (0.41 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.95 / 0.75)
- p95LatencyMs: pass (249 / 500)
- p99LatencyMs: pass (249 / 900)
- criticalFailures: fail (1 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
