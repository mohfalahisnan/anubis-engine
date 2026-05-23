# Experiment 0003-image-sidecar-indexing Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: precision-jump matched (0.22 -> 0.31); aqi-jump matched (69.6 -> 73.1)
- Final decision: keep

## Follow-up Finding

The target priority was not closed. `retrieval.criticalFailures` stayed at 1, and the critical failure diagnostics still show `img_invoice_02.png` indexed with zero chunks. The next hypothesis should target the image preprocessing/indexing gap before ranking changes.

## Before

- AQI: 69.6
- Recall@10: 0.89
- Precision@10: 0.22
- p95 latency: 377 ms
- Critical failures: 1
- Permission leakage: 0

## After

- AQI: 73.1
- Gated AQI: 73.1
- Recall@10: 0.89
- Precision@10: 0.31
- p95 latency: 319 ms
- Critical failures: 1
- Permission leakage: 0

## Production Checks

- aqi: fail (73.1 / 85)
- recallAt10: fail (0.89 / 0.92)
- precisionAt10: fail (0.31 / 0.45)
- top3Accuracy: pass (0.87 / 0.8)
- mrrAt10: pass (0.88 / 0.7)
- ndcgAt10: pass (0.87 / 0.75)
- p95LatencyMs: pass (319 / 500)
- p99LatencyMs: pass (319 / 900)
- criticalFailures: fail (1 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
