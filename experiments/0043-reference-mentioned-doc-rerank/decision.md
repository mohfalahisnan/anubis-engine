# Experiment 0043-reference-mentioned-doc-rerank Decision

Decision: keep

Rule precedence: revertIfAny > keepIfAny > default.

## Rule Evaluation

- revertIfAny: none matched
- keepIfAny: `trusted-graph-observable` matched
- Precision@10 improved from 0.49 to 0.53, but AQI remains below target and active module listing still misses.
- Baseline update: yes

## Before

- AQI: 74
- Recall@10: 0.93
- Precision@10: 0.49
- p95 latency: 356 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## After

- AQI: 73.2
- Gated AQI: 73.2
- Recall@10: 0.93
- Precision@10: 0.53
- p95 latency: 330 ms
- Critical failures: 0
- Permission leakage: 0
- Edge evidence coverage: 1
- Visible edges/node: 3.14

## Production Checks

- aqi: fail (73.2 / 85)
- recallAt10: pass (0.93 / 0.92)
- precisionAt10: pass (0.53 / 0.45)
- top3Accuracy: pass (0.93 / 0.8)
- mrrAt10: pass (0.93 / 0.7)
- ndcgAt10: pass (0.92 / 0.75)
- p95LatencyMs: pass (330 / 500)
- p99LatencyMs: pass (330 / 900)
- criticalFailures: pass (0 / 0)
- permissionLeakage: pass (0 / 0)
- indexingCrashed: pass (false / false)
