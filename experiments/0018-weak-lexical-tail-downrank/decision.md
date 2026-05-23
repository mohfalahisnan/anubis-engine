# Experiment 0018-weak-lexical-tail-downrank Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 0.39 -> 0.37; AQI decreased)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Tried Hypothesis

Weak lexical tails with BM25 below the graph gate and no entity/graph support should receive a final-score multiplier. It regressed Precision@10 and AQI, so it is not promoted.

## Before

- AQI: 85.5
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 216 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 80.1
- Gated AQI: 80.1
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 254 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (80.1 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (254 / 500)
- p99LatencyMs: pass (254 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
