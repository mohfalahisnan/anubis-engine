# Experiment 0028-reduce-graph-weight Decision

Decision: needs_more_data

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 83.3
- Gated AQI: 83.3
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 201 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (83.3 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.94 / 0.7)
- ndcgAt10: pass (0.94 / 0.75)
- p95LatencyMs: pass (201 / 500)
- p99LatencyMs: pass (201 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
