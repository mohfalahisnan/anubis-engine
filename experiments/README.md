# Anubis Experiments

Experiments follow this loop:

```txt
baseline -> hypothesis -> one change -> benchmark -> compare -> keep/revert -> log result
```

## Run an Experiment

Run experiment 0001:

```bash
npm run anubis:experiment -- experiments/0001-score-breakdown/config.json
```

The prompt-requested pnpm form also works in a pnpm environment:

```bash
pnpm anubis:experiment --config experiments/0001-score-breakdown/config.json
```

The runner writes these files into the experiment directory:

- `result.json`
- `report.txt`
- `decision.md`

## Keep/Revert Decision

The decision compares the experiment result against the configured baseline:

- `revert` if permission leakage appears.
- `revert` if critical failures increase.
- `revert` if Recall@10 drops by more than 0.03.
- `revert` if p95 query latency exceeds 500 ms.
- `keep` if Precision@10 improves by at least 0.05 or AQI improves by at least 2.
- Otherwise `needs_more_data`.

## Production Gates

Production goals live in `benchmarks/goals/production.json`. They are stricter than the current alpha baseline and include retrieval quality, latency, graph density, grounding, indexing, and security targets.

Security and leakage metrics are represented in the experiment result shape even where the current quick benchmark cannot yet exercise them. Missing or unavailable metrics are reported as `null` rather than invented.

## Experiment 0001

Experiment 0001 is inspectability-only. It adds query status, ranking metrics, score breakdown in debug benchmark mode, graph density metrics, and indexing phase timing shape. It does not tune retrieval scoring or change normal search behavior.
