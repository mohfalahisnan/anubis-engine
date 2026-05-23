# Experiment 0004-newest-engine-binary Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: critical-fix matched (criticalFailures 1 -> 0)
- Final decision: keep
- Note: bin/experiment.js initially wrote needs_more_data, but production keepRevertRules take precedence for this loop.

## Before

- AQI: 73.1
- Recall@10: 0.89
- Precision@10: 0.31
- p95 latency: 319 ms
- Critical failures: 1
- Permission leakage: 0

## After

- AQI: 74.5
- Gated AQI: 74.5
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 360 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (74.5 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.33 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.97 / 0.75)
- p95LatencyMs: pass (360 / 500)
- p99LatencyMs: pass (360 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
