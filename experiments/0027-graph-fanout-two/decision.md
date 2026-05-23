# Experiment 0027-graph-fanout-two Decision

Decision: keep

## Before

- AQI: 83.1
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 87.9
- Gated AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: pass (87.9 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.39 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (1 / 0.7)
- ndcgAt10: pass (0.98 / 0.75)
- p95LatencyMs: pass (177 / 500)
- p99LatencyMs: pass (177 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
