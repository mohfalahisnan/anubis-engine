# Experiment 0037-overlap-256 Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: recall-regression matched (Recall@10 dropped 0.98 -> 0.93; drop 0.05 > 0.03)
- keepIfAny: precision-jump matched (Precision@10 improved 0.38 -> 0.49)
- Precedence: revertIfAny wins over keepIfAny
- Final decision: revert
- Baseline update: no
- Trial overlap code: reverted

## Tried Hypothesis

Overlap 256 achieved Precision@10 target, but destroyed the active-module-listing query and violated the recall-regression guard. Do not promote broad overlap 256 without a separate fix for generic module-listing queries.

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

- AQI: 72.8
- Gated AQI: 72.8
- Recall@10: 0.93
- Precision@10: 0.49
- p95 latency: 376 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (72.8 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: pass (0.49 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.93 / 0.75)
- p95LatencyMs: pass (376 / 500)
- p99LatencyMs: pass (376 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
