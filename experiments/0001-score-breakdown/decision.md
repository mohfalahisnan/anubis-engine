# Experiment 0001-score-breakdown Decision

Decision: keep

## Before

- AQI: 69.6
- Recall@10: 0.89
- Precision@10: 0.22
- p95 latency: 377 ms
- Critical failures: 1
- Permission leakage: 0

## After

- AQI: 74.4
- Gated AQI: 74.4
- Recall@10: 0.91
- Precision@10: 0.31
- p95 latency: 324 ms
- Critical failures: 1
- Permission leakage: 0

## Production Checks

- aqi: fail (74.4 / 85)
- recallAt10: fail (0.91 / 0.92)
- precisionAt10: fail (0.31 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.91 / 0.75)
- p95LatencyMs: pass (324 / 500)
- p99LatencyMs: pass (324 / 900)
- criticalFailures: fail (1 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
