# Experiment 0032-smaller-content-window Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 stayed 0.39; AQI decreased)
- edge-bloat: not matched (edge count stayed 29340)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted

## Tried Hypothesis

Reducing the default content window to 384 did not change chunk or edge counts in the quick benchmark and did not improve Precision@10. AQI decreased from 87.9 to 86.7, so the change is not promoted.

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 86.7
- Gated AQI: 86.7
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 196 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (86.7 / 95)
- recallAt10: pass (0.98 / 0.95)
- precisionAt10: fail (0.39 / 0.5)
- top3Accuracy: pass (1 / 0.95)
- mrrAt10: pass (1 / 0.9)
- ndcgAt10: pass (0.98 / 0.9)
- p95LatencyMs: pass (196 / 500)
- p99LatencyMs: pass (196 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
