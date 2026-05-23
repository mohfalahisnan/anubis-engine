# Experiment 0053-skip-centrality-tiebreak Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `trusted-graph-observable` matched
- p95 improved 303 -> 293, but AQI remains below target.
- Baseline update: yes

## Before

- AQI: 81.8
- Recall@10: 1
- Precision@10: 0.46
- p95 latency: 303 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 2.84

## After

- AQI: 82.4
- Gated AQI: 82.4
- Recall@10: 1
- Precision@10: 0.46
- p95 latency: 293 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 2.84

## Production Checks

- aqi: fail (82.4 / 85)
- recallAt10: pass (1 / 0.92)
- precisionAt10: pass (0.46 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.99 / 0.75)
- p95LatencyMs: pass (293 / 500)
- p99LatencyMs: pass (293 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
