# Experiment 0011-lexical-weight-shift Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (no precision jump; no AQI jump)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Before

- AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.33
- p95 latency: 205 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 82.8
- Gated AQI: 82.8
- Recall@10: 0.96
- Precision@10: 0.33
- p95 latency: 209 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (82.8 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.33 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (209 / 500)
- p99LatencyMs: pass (209 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
