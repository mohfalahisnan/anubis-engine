# Experiment 0036-trusted-graph-metrics Decision

Decision: keep

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 0.04
- Visible edges/node: null

## After

- AQI: 77.2
- Gated AQI: 77.2
- Recall@10: 0.98
- Precision@10: 0.38
- p95 latency: 328 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.1

## Production Checks

- aqi: fail (77.2 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.38 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.97 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (328 / 500)
- p99LatencyMs: pass (328 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
