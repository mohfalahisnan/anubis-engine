# Experiment 0002-critical-failure-diagnosis Decision

Decision: keep

## Before

- AQI: 69.6
- Recall@10: 0.89
- Precision@10: 0.22
- p95 latency: 377 ms
- Critical failures: 1
- Permission leakage: 0

## After

- AQI: 69.6
- Gated AQI: 69.6
- Recall@10: 0.89
- Precision@10: 0.31
- p95 latency: 377 ms
- Critical failures: 1
- Permission leakage: 0

## Production Checks

- aqi: fail (69.6 / 85)
- recallAt10: fail (0.89 / 0.92)
- precisionAt10: fail (0.31 / 0.45)
- top3Accuracy: pass (0.87 / 0.8)
- mrrAt10: pass (0.88 / 0.7)
- ndcgAt10: pass (0.87 / 0.75)
- p95LatencyMs: pass (377 / 500)
- p99LatencyMs: pass (377 / 900)
- criticalFailures: fail (1 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
