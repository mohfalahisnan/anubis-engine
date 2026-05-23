# Experiment 0003: Image Sidecar Indexing

## Target Priority

- Metric: retrieval.criticalFailures
- Current: 1
- Target: 0
- Gap: 1

## Hypothesis

The `invoice approval` critical failure is caused by the expected image evidence not producing searchable text chunks. When image OCR sidecar text is ingested with stable image filename context, the expected `img_invoice_02.png` content should enter the candidate set and eliminate the zero-recall query.

## Change Scope

One behavior change in the image parser:

- Preserve fresh OCR sidecar text as the primary searchable text for image documents.
- Add filename context to non-empty OCR text so image chunks carry a stable document identity signal.
- Leave empty OCR output empty so blank images do not create noisy metadata-only chunks.

## Revert Risk Check

- Permission leakage: unchanged.
- Critical failures increase: unlikely by construction; image text ingestion only adds signal to image documents.
- Recall regression: unlikely; existing text documents and ranking expectations are unchanged.
- Latency regression: low; only a small string prefix is added to non-empty image OCR text.
- Edge bloat: possible only through new image chunks; keep only if retrieval improves enough under the rule set.
