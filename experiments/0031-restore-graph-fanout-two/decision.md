# Experiment 0031-restore-graph-fanout-two Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 0.39 -> 0.38; AQI decreased)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Tried Hypothesis

Restoring graph fan-out to two did not improve precision against the active production baseline. Recall stayed inside the allowed range, but Precision@10 fell to 0.38 and AQI fell to 85, so the change is not promoted.

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 85
- Gated AQI: 85
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 198 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (85 / 95)
- recallAt10: pass (0.96 / 0.95)
- precisionAt10: fail (0.38 / 0.5)
- top3Accuracy: pass (1 / 0.95)
- mrrAt10: pass (0.97 / 0.9)
- ndcgAt10: pass (0.95 / 0.9)
- p95LatencyMs: pass (198 / 500)
- p99LatencyMs: pass (198 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
