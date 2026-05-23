# Experiment 0048-overlap-128-after-route-cap Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `aqi-jump` matched (+6.7 AQI)
- AQI now passes; Precision@10 remains below target.
- Baseline update: yes

## Before

- AQI: 79.4
- Recall@10: 1
- Precision@10: 0.47
- p95 latency: 343 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 86.1
- Gated AQI: 86.1
- Recall@10: 1
- Precision@10: 0.4
- p95 latency: 231 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.16

## Production Checks

- aqi: pass (86.1 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: fail (0.4 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.95 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (231 / 500)
- p99LatencyMs: pass (231 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
