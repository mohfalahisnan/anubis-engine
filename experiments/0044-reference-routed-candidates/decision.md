# Experiment 0044-reference-routed-candidates Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `precision-jump` and `aqi-jump` matched
- Active listing recall recovered; AQI remains below target due p95 latency and top-5 misses.
- Baseline update: yes

## Before

- AQI: 73.2
- Recall@10: 0.93
- Precision@10: 0.53
- p95 latency: 330 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 76.9
- Gated AQI: 76.9
- Recall@10: 1
- Precision@10: 0.59
- p95 latency: 320 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (76.9 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.59 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.98 / 0.75)
- p95LatencyMs: pass (320 / 500)
- p99LatencyMs: pass (320 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
