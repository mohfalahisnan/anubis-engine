# Experiment 0021-graph-seed-prune Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 0.39 -> 0.38; AQI decreased)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Tried Hypothesis

Reducing graph expansion seeds from 10 to 5 should prune weak graph-expanded tails. It regressed Precision@10 and AQI, so it is not promoted.

## Before

- AQI: 85.5
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 216 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 84
- Gated AQI: 84
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 214 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (84 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.38 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (214 / 500)
- p99LatencyMs: pass (214 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
