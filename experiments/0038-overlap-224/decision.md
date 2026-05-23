# Experiment 0038-overlap-224 Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: recall-regression matched (Recall@10 dropped 0.98 -> 0.93; drop 0.05 > 0.03)
- keepIfAny: none matched (Precision@10 improved 0.38 -> 0.42; gain 0.04 < 0.05)
- Final decision: revert
- Baseline update: no
- Trial overlap code: reverted

## Tried Hypothesis

Overlap 224 still destroyed active-module-listing recall and did not reach the precision-jump keep threshold. Do not continue broad-overlap tuning.

## Before

- AQI: 77.2
- Recall@10: 0.98
- Precision@10: 0.38
- p95 latency: 328 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.1

## After

- AQI: 75.8
- Gated AQI: 75.8
- Recall@10: 0.93
- Precision@10: 0.42
- p95 latency: 326 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3

## Production Checks

- aqi: fail (75.8 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: fail (0.42 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.93 / 0.75)
- p95LatencyMs: pass (326 / 500)
- p99LatencyMs: pass (326 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
