# Experiment 0047-reference-route-one-per-doc Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `trusted-graph-observable` matched
- Recall@5 recovered to 1.0, but AQI remains below target due p95 latency.
- Baseline update: yes

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

- AQI: 79.4
- Gated AQI: 79.4
- Recall@10: 1
- Precision@10: 0.47
- p95 latency: 343 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (79.4 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.47 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.99 / 0.75)
- p95LatencyMs: pass (343 / 500)
- p99LatencyMs: pass (343 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
