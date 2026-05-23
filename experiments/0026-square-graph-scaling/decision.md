# Experiment 0026-square-graph-scaling Decision

Decision: needs_more_data

## Before

- AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 81.7
- Gated AQI: 81.7
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 214 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (81.7 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.39 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (214 / 500)
- p99LatencyMs: pass (214 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
