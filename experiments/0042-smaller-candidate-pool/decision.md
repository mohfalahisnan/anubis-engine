# Experiment 0042-smaller-candidate-pool Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `trusted-graph-observable` matched
- AQI improved from 72.6 to 74.0, but still below target 85.
- Baseline update: yes

## Before

- AQI: 72.6
- Recall@10: 0.93
- Precision@10: 0.49
- p95 latency: 378 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 74
- Gated AQI: 74
- Recall@10: 0.93
- Precision@10: 0.49
- p95 latency: 356 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (74 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: pass (0.49 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.93 / 0.75)
- p95LatencyMs: pass (356 / 500)
- p99LatencyMs: pass (356 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
