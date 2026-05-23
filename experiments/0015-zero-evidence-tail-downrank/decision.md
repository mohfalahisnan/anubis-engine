# Experiment 0015-zero-evidence-tail-downrank Decision

Decision: needs_more_data

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: none matched (Precision@10 0.38 -> 0.39, below +0.05; AQI 83.1 -> 84, below +2)
- Final decision: needs_more_data
- Baseline update: no
- Trial code: not promoted into next iteration

## Tried Hypothesis

Zero-evidence vector-only tail candidates should receive a final-score multiplier. This produced a small Precision@10 and AQI lift, but not enough to satisfy a keep rule.

## Before

- AQI: 83.1
- Recall@10: 0.96
- Precision@10: 0.38
- p95 latency: 230 ms
- Critical failures: 0
- Permission leakage: 0

## After

- AQI: 84
- Gated AQI: 84
- Recall@10: 0.98
- Precision@10: 0.39
- p95 latency: 215 ms
- Critical failures: 0
- Permission leakage: 0

## Production Checks

- aqi: fail (84 / 85)
- recallAt10: pass (0.98 / 0.92)
- precisionAt10: fail (0.39 / 0.45)
- top3Accuracy: pass (1 / 0.8)
- mrrAt10: pass (0.97 / 0.7)
- ndcgAt10: pass (0.96 / 0.75)
- p95LatencyMs: pass (215 / 500)
- p99LatencyMs: pass (215 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
