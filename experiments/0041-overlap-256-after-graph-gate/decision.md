# Experiment 0041-overlap-256-after-graph-gate Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched (`recall-regression` is not triggered because 0.96 - 0.93 = 0.03, not greater than 0.03)
- keepIfAny: `precision-jump` matched (+0.12 Precision@10)
- Precision@10 now passes productionV1; AQI remains open.
- Baseline update: yes

## Before

- AQI: 81.9
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 223 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.1

## After

- AQI: 72.6
- Gated AQI: 72.6
- Recall@10: 0.93
- Precision@10: 0.49
- p95 latency: 378 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (72.6 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: pass (0.49 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.93 / 0.75)
- p95LatencyMs: pass (378 / 500)
- p99LatencyMs: pass (378 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
