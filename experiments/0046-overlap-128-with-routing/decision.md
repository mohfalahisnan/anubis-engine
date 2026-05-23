# Experiment 0046-overlap-128-with-routing Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: `critical-failures-increase` matched (0 -> 1)
- keepIfAny: ignored because revert has precedence
- Trial code: reverted
- Baseline update: no

## Before

- AQI: 77.8
- Recall@10: 1
- Precision@10: 0.59
- p95 latency: 331 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 79.2
- Gated AQI: 79.2
- Recall@10: 1
- Precision@10: 0.48
- p95 latency: 231 ms
- Critical failures: 1
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.16

## Production Checks

- aqi: fail (79.2 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.48 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (231 / 500)
- p99LatencyMs: pass (231 / 900)
- criticalFailures: fail (1 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
