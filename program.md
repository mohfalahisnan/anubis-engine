# Anubis Engine Research Program

You are improving Anubis Engine using benchmark-driven development.

## Current Baseline

AQI: 69.6
Recall@K: 0.89
Precision@K: 0.22
p95 Latency: 377ms
Index throughput: 1.3 KB/s
Chunks: 417
Edges: 29618
Critical failures: 1

## Production Target

AQI >= 85
Recall@10 >= 0.92
Precision@10 >= 0.45
Top-3 Accuracy >= 0.80
p95 latency <= 500ms
visible edges <= 15 per node
source coverage >= 95%
permission leakage = 0
critical failures = 0

## Rules

1. Make one meaningful change per experiment.
2. Do not change benchmark expectations to make results pass.
3. Preserve or improve Recall@10.
4. Optimize Precision@10 first.
5. Do not increase p95 latency above 500ms.
6. Do not add dependencies unless necessary.
7. Every result must include score breakdown.
8. Every edge should have evidence or be hidden from trusted graph.
9. Security failures require immediate revert.
10. After each experiment, write result.json and decision.md.

## Current Research Question

How can we improve Precision@10 from 0.22 to at least 0.35 without reducing Recall@10 below 0.86?

## Suggested Experiments

1. Add score breakdown logging.
2. Add hybrid reranker.
3. Add query alias expansion.
4. Add title and metadata boost.
5. Add boilerplate downranking.
6. Add top-N edge pruning.
7. Split candidate graph and trusted graph.
8. Add source evidence coverage metric.

## Keep Criteria

Keep if:
- Precision@10 improves by >= 0.05, or
- AQI improves by >= 2, and
- Recall@10 drops by <= 0.03, and
- p95 latency <= 500ms, and
- critical failures do not increase.

Revert if:
- permission leakage appears
- critical failures increase
- Recall@10 drops by more than 0.03
- p95 latency exceeds 500ms
- graph edge count increases without quality improvement