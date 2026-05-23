# Experiment 0052-window-384-overlap-64 Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `aqi-jump` matched (+5.9 AQI)
- Precision still passes; AQI remains below target due p95 latency.
- Baseline update: yes

## Before

- AQI: 75.9
- Recall@10: 1
- Precision@10: 0.56
- p95 latency: 402 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 81.8
- Gated AQI: 81.8
- Recall@10: 1
- Precision@10: 0.46
- p95 latency: 303 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 2.84

## Production Checks

- aqi: fail (81.8 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.46 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.99 / 0.75)
- p95LatencyMs: pass (303 / 500)
- p99LatencyMs: pass (303 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
