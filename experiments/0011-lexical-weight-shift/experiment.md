# Experiment 0011: Lexical Weight Shift

## Target Priority

- Metric: retrieval.precisionAt10
- Current: 0.33
- Target: 0.45
- Gap: 0.12

## Hypothesis

Precision is limited by semantically similar but lexically weak candidates. The benchmark queries contain exact operational phrases and anchors, so BM25 should carry more weight than vector similarity.

## Change Scope

One ranking change:

- Shift BM25 weight from 0.35 to 0.45.
- Shift vector weight from 0.40 to 0.30.
- Keep total score weight at 1.0 and leave graph/entity weights unchanged.
- Do not change benchmark targets, relevance labels, or query cases.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures: protected by rule.
- Recall regression: possible for semantic-only matches, protected by rule.
- Latency regression: no extra IO/model work.
- Edge bloat: graph construction unchanged.
