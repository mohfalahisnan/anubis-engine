# Experiment 0016-top-doc-affinity-rerank Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 0.38 -> 0.37; AQI 83.1 -> 84.3, below +2)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Tried Hypothesis

A small rerank bonus for candidates from the current top document should improve bottom-half precision. It raised AQI slightly but regressed Precision@10, so it is not promoted.

## Before

- AQI: 83.1
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 84.3
- Gated AQI: 84.3
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 210 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (84.3 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.96 / 0.7)
- ndcgAt10: pass (0.95 / 0.75)
- p95LatencyMs: pass (210 / 500)
- p99LatencyMs: pass (210 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
