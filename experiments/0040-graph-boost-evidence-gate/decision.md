# Experiment 0040-graph-boost-evidence-gate Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `trusted-graph-observable` matched
- Precision@10 unchanged at 0.37, so the precision target remains open.
- Baseline update: yes

## Before

- AQI: 81.9
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 224 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.1

## After

- AQI: 81.9
- Gated AQI: 81.9
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 223 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.1

## Production Checks

- aqi: fail (81.9 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (223 / 500)
- p99LatencyMs: pass (223 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
