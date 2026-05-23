# Experiment 0019-candidate-pool-overfetch Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: recall-regression matched (`recallAt10` 0.98 -> 0.93, drop 0.05)
- keepIfAny: not considered after revert precedence
- Final decision: revert
- Baseline update: no
- Trial code: reverted

## Tried Hypothesis

Overfetching the candidate pool should expose more relevant chunks before rerank. It regressed recall and precision, so it is not promotable.

## Before

- AQI: 85.5
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 216 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 82
- Gated AQI: 82
- Recall@10: 0.93
- Precision@10: 0.37
- p95 latency: 222 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (82 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.93 / 0.75)
- p95LatencyMs: pass (222 / 500)
- p99LatencyMs: pass (222 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
