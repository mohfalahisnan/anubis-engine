# Experiment 0002: Critical Failure Diagnosis

## Goal

Diagnose why the `invoice approval` benchmark query has Recall@10 = 0 without changing retrieval ranking behavior.

## Hypothesis

The expected invoice evidence is indexed, but the expected result disappears before final Top-K output because either candidate generation does not surface it deeply enough or final ranking/diversification pushes it below the reported cutoff.

## Change

Add benchmark/debug-only diagnostics for critical failure queries:
- Expected document and chunk indexing checks
- Exact query-term and alias-term checks against expected chunks
- Component-sorted BM25, vector, and graph candidate views from diagnostic search results
- Final merged top results
- Stage-drop inference
- Score breakdowns and source paths for expected and retrieved candidates

## Success Criteria

- Existing benchmark still runs.
- Retrieval behavior remains unchanged.
- Critical failure diagnostics appear in the experiment report.
- `result.json` includes `criticalFailureDiagnostics`.
- No normal app runtime behavior changes.
- `npm test` passes.

## Decision Rule

Keep if diagnostics identify where the expected result disappeared or clearly prove the benchmark label/data is wrong.

Revert if normal retrieval behavior changes or benchmark becomes unstable.
