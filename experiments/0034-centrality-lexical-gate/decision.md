# Experiment 0034-centrality-lexical-gate Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched (permission leakage 0, critical failures 0, recall drop 0.02, p95 399 ms)
- keepIfAny: none matched (Precision@10 decreased 0.39 -> 0.38; AQI decreased 87.9 -> 72.9)
- edge-bloat: not matched (edge count stayed unchanged)
- Final decision: needs_more_data
- Baseline update: no
- Trial centrality code: not promoted

## Tried Hypothesis

Removing centrality from vector-only matches did not improve Precision@10 and caused a large latency/AQI regression. Do not pursue broad vector-only centrality removal.

## Before

- AQI: 87.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 177 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 72.9
- Gated AQI: 72.9
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 399 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (72.9 / 95)
- recallAt10: pass (0.96 / 0.95)
- precisionAt10: fail (0.38 / 0.5)
- top3Accuracy: pass (1 / 0.95)
- mrrAt10: pass (0.97 / 0.9)
- ndcgAt10: pass (0.95 / 0.9)
- p95LatencyMs: pass (399 / 500)
- p99LatencyMs: pass (399 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
