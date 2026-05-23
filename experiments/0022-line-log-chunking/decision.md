# Experiment 0022-line-log-chunking Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 0.39 -> 0.37; AQI decreased)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Tried Hypothesis

Timestamped text logs should be chunked by line to create more relevant syslog hits. The benchmark still produced the same chunk and edge counts, and retrieval quality regressed, so this approach is not promoted.

## Before

- AQI: 85.5
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 216 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 75.5
- Gated AQI: 75.5
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 330 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (75.5 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (330 / 500)
- p99LatencyMs: pass (330 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
