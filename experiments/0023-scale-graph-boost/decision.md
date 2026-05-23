# Experiment 0023-scale-graph-boost Decision

Decision: keep

## Before

- AQI: 81
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 239 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 83.1
- Gated AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (83.1 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.39 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.97 / 0.75)
- p95LatencyMs: pass (230 / 500)
- p99LatencyMs: pass (230 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
