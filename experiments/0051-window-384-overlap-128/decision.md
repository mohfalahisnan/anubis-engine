# Experiment 0051-window-384-overlap-128 Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `precision-jump` matched (+0.16 Precision@10)
- Precision now passes; AQI remains below target due p95 latency.
- Baseline update: yes

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

- AQI: 75.9
- Gated AQI: 75.9
- Recall@10: 1
- Precision@10: 0.56
- p95 latency: 402 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (75.9 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.56 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.99 / 0.75)
- p95LatencyMs: pass (402 / 500)
- p99LatencyMs: pass (402 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
