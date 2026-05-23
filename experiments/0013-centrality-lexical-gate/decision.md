# Experiment 0013-centrality-lexical-gate Decision

Decision: revert

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: latency-regression matched (`p95LatencyMs` 1431 > 500)
- keepIfAny: not considered after revert precedence
- Final decision: revert
- Baseline update: no
- Trial code: reverted

## Tried Hypothesis

Centrality should require BM25 or entity evidence before contributing its graph-derived tie-break bonus. This produced a small Precision@10 gain, but the latency regression makes the hypothesis non-promotable under the production rules.

## Before

- AQI: 81
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 239 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 66.9
- Gated AQI: 66.9
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 1431 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (66.9 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.39 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.97 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: fail (1431 / 500)
- p99LatencyMs: fail (1431 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
