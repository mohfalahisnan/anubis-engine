# Experiment 0035-overlap-192 Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched under rules active at run time (permission leakage 0, critical failures 0, recall drop 0.02, p95 297 ms)
- keepIfAny: none matched (Precision@10 improved only 0.02; AQI decreased 87.9 -> 77.5)
- edge-bloat: not matched because Precision@10 improved, but edges grew 29340 -> 40116
- Final decision: needs_more_data
- Baseline update: no
- Trial overlap code: not promoted

## Tried Hypothesis

Overlap 192 improved some single-file module queries but hurt active-module listing and AQI. Do not promote broad overlap growth.

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 77.5
- Gated AQI: 77.5
- Recall@10: 0.96
- Precision@10: 0.41
- p95 latency: 297 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (77.5 / 95)
- recallAt10: pass (0.96 / 0.95)
- precisionAt10: fail (0.41 / 0.5)
- top3Accuracy: fail (0.93 / 0.95)
- mrrAt10: pass (0.94 / 0.9)
- ndcgAt10: pass (0.94 / 0.9)
- p95LatencyMs: pass (297 / 500)
- p99LatencyMs: pass (297 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
