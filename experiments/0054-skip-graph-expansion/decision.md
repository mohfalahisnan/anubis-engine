# Experiment 0054-skip-graph-expansion Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `aqi-jump` matched (+13.6 AQI)
- Stop condition: all productionV1 checks pass; gated AQI equals AQI; edge evidence and visible edge gates pass.
- Baseline update: yes

## Before

- AQI: 82.4
- Recall@10: 1
- Precision@10: 0.46
- p95 latency: 293 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 2.84

## After

- AQI: 96
- Gated AQI: 96
- Recall@10: 1
- Precision@10: 0.46
- p95 latency: 67 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 2.84

## Production Checks

- aqi: pass (96 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.46 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.99 / 0.75)
- p95LatencyMs: pass (67 / 500)
- p99LatencyMs: pass (67 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
