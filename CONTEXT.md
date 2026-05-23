# Project glossary — anubis-engine

The shared vocabulary for this codebase. Architecture reviews and design
docs use these terms; modules and types are named after them where
practical.

Add a term here when you introduce a domain concept that names a
non-obvious thing. Don't add types from `std`, third-party crate
vocabulary, or words that only describe code structure (those live in
the architecture review's glossary, not here).

## Terms

**Document** — a single file the user has indexed. Has a stable id, a
path, a format, and a `doc_class` (Content or Reference).

**Chunk** — a slice of a document's text, the unit of embedding and the
unit of retrieval. Chunks belong to exactly one document and carry their
own id, char offsets, and optional page number.

**Vector** — the dense embedding of one chunk's content, stored as a
`BLOB` of little-endian `f32` values keyed by chunk id.

**Entity** — a typed signal extracted from a chunk's text: `ANCHOR`
(structural IDs like `INC-2026-ATLAS-014`), `DATE`, `PROPER`, `PHRASE`,
`KEYWORD`. Only the first four contribute to relations.

**Anchor** — the strongest entity kind. A hyphenated all-caps ID with
three or more segments. Anchors that appear in two documents create
the highest-confidence cross-doc relation (`shared_anchor` edge,
weight 0.9).

**Edge** — a graph relation between two chunks. Each edge has an
`edge_type` (`shared_anchor`, `shared_entity`, `semantic`,
`semantic_topk`, `same_doc`, `manifest_overlap`), a weight, and a
`reason` (a short structured tag explaining the relation, e.g.
`anchor:INC-2026-ATLAS-014`). Edges with `edge_type ∈ {shared_anchor,
shared_entity, semantic}` and `weight ≥ 0.62` are *strong* and drive
search-time graph expansion; others are visualisation-only context.

**Evidence** — the materialised proof of an edge: the literal anchor or
phrase plus a short text span from each endpoint chunk. Computed on
read so consumers (UI, AI) can cite the connection without trusting it
on the edge_type alone.

**Reference document** (`doc_class = 'reference'`) — a manifest, README,
file list, relation map, or TOC. Recognised by filename. Search blends
multiply their `(W_VEC · score_vec + W_BM25 · score_bm25)` component by
0.6 so they don't dominate Q&A, and they're excluded as relation
endpoints from cross-doc edges (they get their own `manifest_overlap`
edges instead).

**Content document** (`doc_class = 'content'`) — everything that isn't
a reference document. Default classification.

**Sidecar** — a cached preprocessing artifact written next to a source
file with the `.anubis.txt` infix. Examples: `clip.anubis.txt`
(Whisper transcript next to `clip.mp4`), `photo.anubis.txt` (OCR text
next to `photo.png`). The indexer's parsers read sidecars instead of
re-running the heavy preprocessing step. A sidecar is *fresh* when its
mtime is greater than or equal to its source's mtime; freshness is
the cache-invalidation signal. Sidecar paths default to the source's
directory and honour `ANUBIS_TRANSCRIPT_DIR` for relocation.

**Preprocessing pre-pass** — the first stage of `index_folder`. Walks
the input files, runs Whisper / OCR / future scanned-PDF OCR, writes
sidecars. Distinct from the indexing pass so the UI can surface
preprocess progress separately and so failures (a corrupt video) don't
poison the whole batch.

**Indexing pass** — the second stage of `index_folder`. Parses each
file (parsers read sidecars where applicable), chunks the text, embeds
the chunks, extracts entities, writes everything to SQLite and to the
Tantivy full-text index, and builds graph edges.

**Hybrid query** — the search blend used by `query::hybrid::run_query`:
BM25 + dense-vector similarity + entity match + graph expansion +
gated centrality, with diversity caps per document.

**Context pack** — a hybrid-query result rendered for an LLM caller:
the top chunks plus their graph relations plus inline evidence,
token-budgeted.

## How to update

When a refactor names a deepened module after a concept, the concept
goes here first. When a fuzzy term gets sharpened during a grilling
conversation, the entry here gets updated in the same change. Keep
entries one paragraph long; this file is for *names*, not behaviour
specs.
