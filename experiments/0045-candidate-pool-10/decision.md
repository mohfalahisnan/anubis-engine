# Experiment 0045-candidate-pool-10 Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `trusted-graph-observable` matched
- AQI improved from 76.9 to 77.8, but p95 latency remains too high.
- Baseline update: yes

## Before

- AQI: 76.9
- Recall@10: 1
- Precision@10: 0.59
- p95 latency: 320 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 77.8
- Gated AQI: 77.8
- Recall@10: 1
- Precision@10: 0.59
- p95 latency: 331 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (77.8 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.59 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.98 / 0.75)
- p95LatencyMs: pass (331 / 500)
- p99LatencyMs: pass (331 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
