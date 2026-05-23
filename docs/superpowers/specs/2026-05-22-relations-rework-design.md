# Relations rework: separate search from relation, anchor-based evidence

Status: approved (brainstorm)
Date: 2026-05-22

## Problem

The current Anubis index conflates two distinct notions:

1. **Search hits** — chunks that match the user's query.
2. **Relations** — pairs of documents that are actually connected.

Today, any cross-doc graph edge (including weak ones like `same_doc` and
`semantic_topk`) can pull neighbors into the search candidate pool via
`hybrid::expand_via_graph`. Combined with shared-entity edges built from
generic words ("workspace", "briefing", "review", filenames), this lets a
manifest or README dominate Q&A and lets the answering AI claim "X is
related to Y" with only weak co-occurrence evidence.

Five concrete failures motivate this rework:

- A `manifest.txt` listing every file in the project becomes the highest-
  centrality node and surfaces on unrelated queries.
- Two unrelated docs that both say "AI" or "workspace" get a `shared_entity`
  edge and look connected.
- A 30-minute video with `Man_speaking_in_workspace_202605211019.anubis.txt`
  already produced is re-transcribed on every reindex.
- Engine-generated `.anubis.wav` files were briefly indexed as audio docs
  (now filtered, but the related logic remains brittle).
- The MCP tool surface gives the AI no machine-readable reason *why* two
  chunks are linked, so it makes up the reason in prose.

## Goal

After this change:

- A search result means "matched the query." A relation means "shares
  concrete evidence with another doc."
- Manifest-style files are first-class but never dominate normal Q&A and
  never count as content relation evidence.
- Cross-doc relations require either a strong structural anchor
  (`VID-APPROVAL-005`, `INC-2026-ATLAS-014`, `APPROVAL-Q-ATLAS`) or a
  high-confidence semantic match (cosine ≥ 0.62 over a real entity).
- Every relation carries a structured `reason` and (where applicable) a
  literal `evidence` snippet showing the matched span in both chunks. The
  AI can only credibly claim a relation when it can cite that span.
- Video / audio docs reuse their `<stem>.anubis.txt` sidecar transcript
  when it is up-to-date, instead of re-running Whisper.

## Non-goals

- Replacing the embedding model or the chunker.
- Adding a real NER model. The anchor detector is a deterministic regex;
  the existing PROPER/PHRASE/KEYWORD heuristics stay.
- Backfilling old indexes automatically. Schema migrates in place; relation
  data refreshes lazily as the user reindexes content.

## Architecture: search-relation split

Two distinct contributions to a chunk's final score:

- **Search-only signals** (unchanged in shape): `score_bm25`, `score_vec`,
  `score_entity`. They answer "does this chunk match the query?"
- **Relation signal** (`score_graph`): query-DEPENDENT graph reachability
  from top seeds. Computed only over **strong edges**:
  - `edge_type ∈ { shared_anchor, shared_entity, semantic }`
  - AND `weight ≥ STRONG_EDGE_THRESHOLD = 0.62`
- `same_doc`, `semantic_topk`, `manifest_overlap` remain in `graph_edges`
  for UI graph rendering and tie-breaking centrality, but **never** drive
  graph expansion.

### Reference-document down-rank

`documents.doc_class TEXT NOT NULL DEFAULT 'content'` distinguishes
`content` from `reference` (manifest/index/README). Effects:

1. In `hybrid::final_score`, when the chunk's doc is `reference`, multiply
   the `(W_VEC * score_vec + W_BM25 * score_bm25)` component by `0.6`
   before adding entity/graph/centrality. Manifest hits stay reachable
   when nothing else matches but stop dominating Q&A.
2. In `entity_store::build_shared_anchor_edges` and
   `build_shared_entity_edges`, skip any chunk whose doc is `reference`.
3. New `entity_store::build_manifest_overlap_edges` fires only when *both*
   endpoints are reference docs and they share an anchor — emits
   `edge_type='manifest_overlap'`, `weight=0.3`. Excluded from expansion
   (per the rule above) but shown in the UI graph view labeled
   "listed in the same manifest".

## Sidecar transcripts

Today `parser::video::parse` and `parser::audio::parse` always re-run
`transcription::engine::transcribe_file`. The engine *writes* a
`<stem>.anubis.txt` sidecar after each successful transcription
(see `transcription::engine::transcribe_file`), and `is_engine_output()`
already excludes `*.anubis.*` from normal indexing — but the parser never
reads the sidecar back.

Change: in both `parser/video.rs` and `parser/audio.rs`, before invoking
`transcribe_file`:

1. Resolve the sidecar path: `<source_dir>/<stem>.anubis.txt`, honouring
   `ANUBIS_TRANSCRIPT_DIR` the same way `transcription::engine::resolve_output_dir`
   does today.
2. If the sidecar exists AND its mtime is `>=` the source media mtime,
   read it and use the contents verbatim as the transcript. Skip ffmpeg
   and whisper entirely.
3. Otherwise fall through to `transcribe_file`, which will write a fresh
   sidecar.

Empty sidecar (`text.trim().is_empty()`) is authoritative — it means a
prior run found no usable speech, and the user can delete the file to
force a retry.

`.anubis.wav` keeps its current behavior: not indexed as a regular doc,
optionally removed via `ANUBIS_KEEP_WAV=0`.

## Manifest / reference detection

`parser::doc_class_from_path(path) -> DocClass` returns `Reference` when
the filename (lowercased) matches any of:

- `manifest.{txt,md,json,csv}`
- `readme*` (matches `README`, `README.md`, `README-internal.md`)
- `index.{txt,md,json}` and `*_index.{txt,md,json}`, but **not** `index.html`
- `file_list.*`, `filelist.*`, `files.{txt,md}`
- `relation_map.*`, `relations.{json,md}`
- `toc.{md,txt}`, `table_of_contents.*`

Anything else is `Content`. Stored once on the documents row at index
time. The rule lives in `parser/mod.rs` so every format path agrees.

## Strong-anchor entities

New entity type `ANCHOR` added alongside the existing
`DATE | PROPER | KEYWORD | PHRASE`.

Detection in `entities::extract_anchors`:

```text
\b[A-Z][A-Z0-9]+(?:-[A-Z0-9]+){2,}\b
```

with whole-match length capped at 64 chars.

- Accepts: `VID-APPROVAL-005`, `INC-2026-ATLAS-014`, `APPROVAL-Q-ATLAS`,
  `SHIP-NODE-SURYA`.
- Rejects: `Anubis` (single token), `INC-014` (only 2 segments),
  `Q1-2026` (first segment too short / single-letter start).
- Confidence `1.0` (deterministic).
- `entity_store::normalize_entity_value` is special-cased so `ANCHOR`
  values are stored verbatim (no case-folding / no whitespace split),
  preserving the literal `VID-APPROVAL-005` for exact match in
  `match_entities`.

Adds `regex = "1"` to `src-tauri/Cargo.toml`. The regex is compiled once
behind a `OnceLock<Regex>`.

## Edge generation: anchor / entity / manifest split

`entity_store::build_shared_entity_edges` is split into three functions,
each emitting a distinct `edge_type`:

| Function | Entity types | edge_type | weight | Skip ref docs? |
|---|---|---|---|---|
| `build_shared_anchor_edges` | `ANCHOR` | `shared_anchor` | `0.9` | yes (per endpoint) |
| `build_shared_entity_edges` | `PROPER`, `PHRASE`, `DATE` (existing logic) | `shared_entity` | existing (0.6 / 0.65 / 0.7) | yes (per endpoint) |
| `build_manifest_overlap_edges` | `ANCHOR` | `manifest_overlap` | `0.3` | only when BOTH endpoints are reference |

`KEYWORD` entities continue to produce no edges (already the case).
Stopword + doc-fraction caps continue to apply to `shared_entity` only;
anchors are inherently discriminative and skip the cap.

`build_shared_anchor_edges` weight `0.9` is above `STRONG_EDGE_THRESHOLD`
(0.62) → fully participates in graph expansion. PHRASE (0.70) and PROPER
(0.65) participate; DATE (0.60) sits just under and is therefore relation-
visible in the UI but does not drive expansion (matches user intent: a
shared date alone is rarely actual content connection).

## Edge `reason` column

Schema:

```sql
ALTER TABLE graph_edges ADD COLUMN reason TEXT;
```

`reason` is a short structured tag, never freeform prose:

| edge_type | reason example |
|---|---|
| `shared_anchor` | `anchor:VID-APPROVAL-005` |
| `shared_entity` | `proper:Atlas` / `phrase:thermal printer` / `date:2026-05-21` |
| `semantic` | `cos:0.71` |
| `semantic_topk` | `cos:0.48` |
| `same_doc` | `same_doc` (literal — no extra info) |
| `manifest_overlap` | `manifest:INC-2026-ATLAS-014` |

`store::graph_store::GraphEdge` gains `pub reason: Option<String>`.
`GraphEdge::canonical_with_reason(src, dst, weight, edge_type, reason)`
becomes the primary constructor; the existing `canonical()` is kept as a
thin wrapper that passes `None` to minimize churn in callers and tests
that don't care about reason.

`upsert_edges` adds `reason = excluded.reason` to its conflict-update
branch alongside the existing "max weight wins" logic.

Legacy edges (pre-migration) keep `reason = NULL`. Read paths tolerate
`NULL` and fall back to displaying just `edge_type`.

## Citation surface: `evidence`

Computed on read, not stored. Returned wherever the engine surfaces a
relation (the `anubis_get_chunk_neighbors` and
`anubis_get_graph_neighborhood` MCP tools; the UI graph view via the same
endpoints).

```rust
pub struct Evidence {
    pub kind: String,              // "shared_anchor" | "shared_entity" | "semantic" | "manifest"
    pub anchor: Option<String>,    // literal e.g. "VID-APPROVAL-005"
    pub src_span: Option<String>,  // ~80-char snippet around the match in the src chunk
    pub dst_span: Option<String>,  // ~80-char snippet around the match in the dst chunk
}
```

For `shared_anchor`, `shared_entity`, and `manifest_overlap`: parse the
literal from `reason`, substring-search each chunk's content, take ±40
chars around the first hit. Cheap because both chunks' text is already in
memory after the SQL join. For `semantic`: returns `None` for `src_span`
/ `dst_span` — the AI sees `kind=semantic` and must describe the relation
as similarity, not content link.

MCP tool descriptions in `mcp::tools::list_tools` gain one extra line for
`anubis_get_chunk_neighbors` and `anubis_get_graph_neighborhood`:

> Each returned relation carries `edge_type`, `edge_reason`, and (where
> applicable) an `evidence` block showing the literal shared anchor and
> the spans where it appears in both chunks. Only claim that two
> documents are content-related when `evidence.kind` is `shared_anchor`
> or `shared_entity` and both `src_span` and `dst_span` are present. A
> `semantic` relation is similarity-only and must be described as such
> ("semantically similar," not "related to"). A `manifest` relation
> means the docs are listed together in a reference file ("listed in the
> same manifest"), not content-related.

This is a tool-description nudge, not a hard gate — the engine cannot
force the LLM to behave. But the data being present and structured
removes the excuse to hallucinate the connection.

## Hybrid query changes

`hybrid::expand_via_graph` ([hybrid.rs:399](src-tauri/src/query/hybrid.rs:399))
adds a WHERE clause to its `neighbor_stmt`:

```sql
WHERE (src_chunk = ?1 OR dst_chunk = ?1)
  AND edge_type IN ('shared_anchor', 'shared_entity', 'semantic')
  AND weight >= 0.62
```

`hybrid::final_score` gains a reference-doc multiplier. Implementation
note: `final_score` currently takes only `&ScoreParts`. To apply the
multiplier we either thread `doc_class` through the score record or
look it up during materialization in `run_query` and apply the multiplier
before sorting. The plan will pick one; the cleanest is to add
`pub doc_class: Option<String>` to `ScoreParts` populated alongside
centrality.

## Schema migrations

All idempotent via `column_exists` checks in `store::db::migrate`. No
data backfill. Bump `SCHEMA_VERSION` to `3`.

```sql
ALTER TABLE documents    ADD COLUMN doc_class TEXT NOT NULL DEFAULT 'content';
ALTER TABLE graph_edges  ADD COLUMN reason TEXT;
CREATE INDEX IF NOT EXISTS idx_docs_doc_class ON documents(doc_class);
```

## Test plan (TDD)

Each change ships with a failing test written first.

### parser

- `parser::doc_class_from_path` table-driven over manifest names and
  control content names.
- `parser::video`: sidecar present and newer than source → no whisper
  invocation, transcript is the sidecar contents verbatim.
- `parser::video`: sidecar older than source → transcribes, writes
  fresh sidecar.
- `parser::audio`: same two cases.

### entities

- `extract_anchors`: positive (`VID-APPROVAL-005`, `INC-2026-ATLAS-014`,
  `APPROVAL-Q-ATLAS`, `SHIP-NODE-SURYA`) and negative (`Anubis`,
  `INC-014`, `Q1-2026`, lowercase `vid-approval-005`).
- `normalize_entity_value` for `ANCHOR` returns the value unchanged.

### edge building

- `build_shared_anchor_edges` produces `shared_anchor` weight 0.9 with
  `reason=anchor:VID-APPROVAL-005` when two docs share the anchor.
- `build_shared_anchor_edges` skips reference-class docs.
- `build_manifest_overlap_edges` fires only when both endpoints are
  reference; emits `manifest_overlap` weight 0.3.
- `build_shared_entity_edges` excludes reference-class chunks.

### graph store

- `upsert_edges` round-trips `reason`.
- Legacy rows with `reason=NULL` read back as `Option::None`.

### hybrid

- `expand_via_graph` does not follow `same_doc` or `semantic_topk` edges.
- `expand_via_graph` does not follow edges with weight < 0.62.
- `final_score` applies the 0.6 multiplier on the bm25+vec portion when
  the doc is `reference`.

### mcp

- `anubis_get_chunk_neighbors` includes `edge_type`, `edge_reason`,
  `evidence` for a `shared_anchor` edge between two seeded chunks.
- `evidence.kind = "semantic"` returns null `src_span` / `dst_span`.

## Migration / rollout

- Startup `migrate()` adds the columns. Existing rows take the column
  defaults (`doc_class='content'`, `reason=NULL`).
- New docs pick up the new logic on first index. Existing docs pick it
  up on next reindex (watcher pass or user-triggered).
- No forced full reindex. The user keeps their embeddings.

## Out of scope

- Parameterizing the anchor regex via settings.
- A separate `edge_evidence` table for multi-evidence edges.
- A new `anubis_explain_relation` MCP tool.
- Backfilling edges for indexes that existed before the schema bump.

These were considered and dropped: the simpler shape covers the user's
five concrete failure cases without overbuilding.
