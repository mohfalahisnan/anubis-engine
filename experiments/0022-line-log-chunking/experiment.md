# Experiment 0022: Line Log Chunking

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.39
- Target: 0.45
- Gap: 0.06

## Hypothesis

The remaining low-precision syslog queries have only one relevant text chunk, so even a perfect ranking cannot place many relevant filename hits in top-10. Timestamped line-oriented logs should be chunked by line so each evidence line can rank independently, improving Precision@10 for log-backed queries without applying the broad 256-character chunking tried in 0008.

## Change Scope

One chunking change:

- For `DocFormat::Text` pages whose first non-empty line looks like an ISO timestamped log line, chunk each sufficiently long non-empty line independently.
- Keep default sliding-window chunking for all other documents unchanged.
- Keep scoring, graph construction, graph expansion, benchmark targets, relevance labels, and query cases unchanged.

## Expected Delta

- Precision@10 should improve for syslog-backed queries by allowing multiple relevant log-line chunks to appear.
- Recall@10 should remain at or above 0.93 because log text remains indexed and other document chunking is unchanged.
- p95 latency should stay under 500 ms because this adds only a small number of chunks for short text logs.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule; no auth behavior changes.
- Recall regression: unlikely for unchanged non-log documents, protected by the recall-regression rule.
- Latency regression: possible from extra log chunks, protected by the latency rule.
- Edge bloat: possible from extra chunks/edges; precision must improve to justify it.
