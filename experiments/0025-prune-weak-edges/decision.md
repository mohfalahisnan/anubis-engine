# Experiment 0025-prune-weak-edges Decision

Decision: needs_more_data

## Before

- AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 84.1
- Gated AQI: 84.1
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 213 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (84.1 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.95 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (213 / 500)
- p99LatencyMs: pass (213 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
