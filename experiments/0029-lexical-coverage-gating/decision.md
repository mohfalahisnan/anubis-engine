# Experiment 0029-lexical-coverage-gating Decision

Decision: needs_more_data (revert)

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 81.0
- Gated AQI: 81.0
- Recall@10: 0.96
- Precision@10: 0.37
- p95 latency: 265 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (81.0 / 85)
- recallAt10: pass (0.96 / 0.92)
- precisionAt10: fail (0.37 / 0.45)
- top3Accuracy: pass (1.0 / 0.8)
- mrrAt10: pass (1.0 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (265 / 500)
- p99LatencyMs: pass (265 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)

## Rationale

Applying a lexical coverage penalty (`signal_multiplier`) based on the max of BM25 and entity scores regressed AQI (from 87.9 to 81.0), Recall@10 (from 0.98 to 0.96), and Precision@10 (from 0.39 to 0.37), while increasing p95 latency to 265 ms. It did not successfully resolve false positives since noisy non-relevant content modules still matched strongly on lexical/entity criteria and scored 1.0, while some true positive chunks with lower BM25 matches were penalized. We will revert this change.
