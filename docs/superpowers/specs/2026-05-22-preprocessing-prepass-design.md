# Preprocessing pre-pass + JSON-hang fix

Status: approved (brainstorm)
Date: 2026-05-22

## Problem

Two coupled problems, rolled into one change because they share the indexer
hot path:

1. **Indexing a large JSON file freezes the entire app.** A 5 MB structured
   JSON flattens to one giant `ParsedPage`, which sliding-chunks into
   thousands of chunks and tens of thousands of entities. The current
   indexer writes those rows as N implicit transactions (per-row fsync),
   issues per-entity edge-builder SELECTs, and holds `state.db.lock()` the
   entire time. Every UI command (`list_documents`, `get_index_stats`,
   `engine_ready`) blocks on the same mutex → UI is unresponsive for
   minutes → user force-closes the app. There is no infinite loop; only
   pathological serial work under a global lock.

2. **Preprocessing (Whisper transcription, image OCR) runs inline inside
   the parser**, so the indexer can't show the user that "the slow step
   is making the audio transcript" — it just shows the file name with no
   progress. There's also no way to skip already-preprocessed files
   cleanly across formats (today only video/audio honor the sidecar
   cache via `parser::video::read_fresh_sidecar`).

These compound: the user can't tell whether the freeze is JSON indexing,
Whisper running, or the app deadlocked.

## Goal

After this change:

- `index_folder` runs in two visible stages: a **preprocessing pre-pass**
  (transcription, OCR — anything that produces a sidecar) followed by an
  **indexing pass** (chunk, embed, write, link).
- The pre-pass writes `<stem>.anubis.txt` sidecars for video, audio, and
  images. Files whose sidecar is at least as fresh as the source are
  skipped in the pre-pass — same mtime-based cache that already works for
  video/audio.
- A failed preprocess marks one file as `status='error'` and continues;
  other files still index.
- The UI receives a new `preprocess-progress` event and an extended
  `index-progress` event with sub-stage labels (parsing / embedding /
  writing / linking).
- `index_one` runs each file's writes inside a single transaction. The DB
  mutex is released between files. Per-row writes stop being implicit
  transactions.
- A `cancel_indexing` Tauri command sets a token that the indexer checks
  between files and between sub-stages. The user can stop a runaway batch
  without force-closing.
- JSON parsing splits its flattened representation into multiple pages so
  one huge document doesn't produce a single ten-thousand-chunk page.

## Non-goals

- PDF page-image OCR for scanned PDFs. Framework supports it (a new
  `PreprocessKind::Pdf` slot is reserved) but the rasterizer integration
  is deferred to its own brainstorm — picking between pdfium-render,
  shelling out, or a pure-Rust renderer is its own decision.
- Parallel preprocessing across files. Whisper and OCR are already
  multi-core internally; running two instances in parallel thrashes.
- Resumable mid-file indexing. Cancel halts between files; the partial
  document being indexed when cancel fires is allowed to finish.
- Rewriting the cross-doc edge builders into a single GROUP BY query. The
  per-entity SELECT loop stays for now; with the transaction fix it's no
  longer the dominant cost.

## Architecture

```
index_folder
  ├─ Stage A — collect supported files (existing walk)
  ├─ Stage B — preprocess pre-pass
  │     for each file in PreprocessKind::Video | Audio | Image:
  │       if needs_preprocessing(path) and !sidecar_fresh(path):
  │         run kind-specific preprocessing
  │         write sidecar atomically (tmp → rename)
  │         emit `preprocess-progress`
  │       on failure: record file as status='error', continue
  │       check cancel token
  │     return PreprocessReport { ok, skipped_fresh, failed }
  └─ Stage C — indexing pass
        for each file (excluding `failed` from Stage B):
          index_one(path)
            ├─ parse: video/audio/image read the sidecar; other formats parse normally
            ├─ chunk, embed
            ├─ acquire DB mutex
            ├─ ONE transaction: upsert_document → replace_chunks → upsert_vectors_batch
            │                   → insert_entities → build_*_edges → upsert_edges
            ├─ release mutex
            ├─ emit `index-progress { stage }` between sub-stages
            └─ check cancel token
```

## New module: `engine::preprocess`

```rust
// src-tauri/src/engine/preprocess.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocessKind {
    Video,
    Audio,
    Image,
    // Reserved for future scanned-PDF OCR; not produced today.
    Pdf,
}

pub fn needs_preprocessing(path: &Path) -> Option<PreprocessKind> {
    match parser::format_from_path(path) {
        DocFormat::Video => Some(PreprocessKind::Video),
        DocFormat::Audio => Some(PreprocessKind::Audio),
        DocFormat::Image => Some(PreprocessKind::Image),
        _ => None,
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessReport {
    pub ok: Vec<PathBuf>,
    pub skipped_fresh: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

pub async fn run_preprocessing(
    paths: &[PathBuf],
    state: &AppState,
    app: Option<AppHandle>,
) -> PreprocessReport;
```

Internally:

- **Video / Audio**: reuse `transcription::engine::transcribe_file`. It
  already writes a `.anubis.txt` sidecar (and optional `.anubis.wav`).
  The pre-pass just invokes it; the indexer's video/audio parsers read
  the sidecar via the existing `read_fresh_sidecar` path.
- **Image**: extract OCR text via `ocr::engine::run` (already exists,
  called inline by `parser::image::parse` today). Write to
  `<stem>.anubis.txt` atomically via the same temp-rename helper used
  by the transcription pipeline.

Cache check (`sidecar_fresh`) is shared logic:
```rust
fn sidecar_fresh(source: &Path, sidecar: &Path) -> bool {
    let src = std::fs::metadata(source).and_then(|m| m.modified()).ok();
    let car = std::fs::metadata(sidecar).and_then(|m| m.modified()).ok();
    matches!((src, car), (Some(s), Some(c)) if c >= s)
}
```

## Parser changes

- `parser::image::parse` becomes mirror of `parser::video::parse`:
  - If `<stem>.anubis.txt` is fresh, read it.
  - Else (parser is being called outside a pre-pass — e.g. single-file
    reindex), fall back to inline OCR. This keeps `index_file` working
    for one-off reindex from the UI's reindex button.
- `parser::video::parse` and `parser::audio::parse` are unchanged in
  behaviour — already cache-first.
- `parser::json::parse` is rewritten to emit **multiple pages**: top-level
  array elements and top-level object keys each become their own page,
  capped at `JSON_PAGE_MAX_CHARS = 16_384`. Sub-trees larger than the cap
  are split greedily into sibling pages. Nothing changes for small JSONs
  (1–2 pages) but a 5 MB JSON now produces ~300 pages instead of 1.

## Event shape changes

Rust side (`src-tauri/src/types.rs`):

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreprocessStage { Transcribing, Ocr, CachedSkipped }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexStage { Parsing, Embedding, Writing, Linking }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessProgress {
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub kind: PreprocessKind,         // serialized lowercase
    pub stage: PreprocessStage,
    pub status: IndexStatus,          // running | done | error | cancelled
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub status: IndexStatus,
    pub errors: Vec<String>,
    /// New — `None` for legacy "between files" events.
    pub stage: Option<IndexStage>,
}
```

Tauri events:

- `preprocess-progress` — new event, emitted during Stage B.
- `index-progress` — existing event, extended with `stage`. The UI
  treats missing `stage` as a between-files event.

`IndexStatus::Cancelled` is added so the UI can show a distinct state.

## Cancellation

`AppState`:

```rust
pub cancel_token: Arc<AtomicBool>,
```

New Tauri command:

```rust
#[tauri::command]
pub async fn cancel_indexing(state: State<'_, EngineHandle>) -> Result<(), String>;
```

Indexer checks `state.cancel_token.load(Ordering::Relaxed)`:
- Between files in Stage B.
- Between files in Stage C.
- Before each sub-stage in `index_one` (parsing/embedding/writing).

On cancel, the indexer emits `IndexStatus::Cancelled` with the current
counters and returns Ok. The token is reset to `false` at the *start* of
the next `index_folder` call so a previous cancel doesn't poison the
next run.

## Perf fixes (the JSON-hang root cause)

### Fix 1 — wrap `insert_entities` in a transaction

```rust
pub fn insert_entities(conn: &mut Connection, hits: &[EntityHit]) -> Result<(), EngineError> {
    let tx = conn.transaction()?;
    {
        let mut entity_stmt = tx.prepare(
            "INSERT INTO entities (id, chunk_id, entity_type, value, normalized_value, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let mut term_stmt = tx.prepare(
            "INSERT OR IGNORE INTO entity_terms (entity_id, chunk_id, term) VALUES (?1, ?2, ?3)",
        )?;
        for hit in hits {
            let entity_id = Uuid::new_v4().to_string();
            let normalized = normalize_entity_value(&hit.value);
            entity_stmt.execute(params![
                entity_id, hit.chunk_id, hit.entity_type,
                hit.value, normalized, hit.confidence as f64,
            ])?;
            for term in entity_terms_for_value(&normalized) {
                term_stmt.execute(params![entity_id, hit.chunk_id, term])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}
```

Signature changes from `&Connection` to `&mut Connection`. Callers
(only the indexer) get a `&mut` from `db.lock().await`.

### Fix 2 — batch vector upserts

```rust
// src-tauri/src/store/vectors.rs
pub fn upsert_vectors_batch(
    conn: &mut Connection,
    items: &[(&str, &[f32])],
) -> Result<(), EngineError>;
```

One transaction, prepared statement reused across rows. Indexer replaces
the existing `for (chunk, embedding) … upsert_vector` loop with one
call.

### Fix 3 — ANCHOR cap per chunk

`entities::extract_anchors` caps at 20 unique anchors per chunk. Same
seen-set pattern as `extract_phrases`. Bounds the worst-case entity-row
explosion when a JSON line lists hundreds of IDs.

### Fix 4 — JSON pagination

`parser::json::parse` emits one `ParsedPage` per top-level object key /
top-level array element. Sub-trees larger than `JSON_PAGE_MAX_CHARS`
(16 KB) get split into sibling pages along the next-deepest boundary.
Tiny JSONs (under the cap total) still produce a single page.

## Indexer changes

```rust
async fn index_one(path: &Path, state: &AppState, app: Option<&AppHandle>) {
    emit_index_progress(app, IndexStage::Parsing, ...);
    let parsed = parser::parse(path)?;

    emit_index_progress(app, IndexStage::Embedding, ...);
    let chunks = sliding::chunk_document(&parsed);
    let embeddings = embed(...);

    let entity_hits = entities::extract_from_chunks(&chunks);

    emit_index_progress(app, IndexStage::Writing, ...);
    {
        let mut db = state.db.lock().await;
        // Each bulk write below runs in its OWN explicit transaction.
        // Per-file cost: ~5 transactions instead of ~50,000 implicit ones.
        // We deliberately do NOT wrap all five into a single outer txn —
        // doing so would force `replace_doc_chunks` / `upsert_edges` /
        // the new `upsert_vectors_batch` / `insert_entities` to take a
        // borrowed `&Transaction` instead of `&mut Connection`, churning
        // their signatures and all their tests. The per-fix speedup is
        // already ~100× from removing per-row implicit txns; further
        // collapsing the 5 inner txns into 1 buys at most 5× more fsyncs.
        let existing_vectors = vectors::vectors_excluding_doc(&db, ...)?;
        db::upsert_document(&db, &doc)?;
        chunks::replace_doc_chunks(&mut db, ...)?;       // already inner-txn
        vectors::upsert_vectors_batch(&mut db, ...)?;    // new — inner-txn
        entity_store::insert_entities(&mut db, ...)?;    // fixed — inner-txn
        let edges = build_all_edges(&db, ...);
        graph_store::upsert_edges(&mut db, &edges)?;     // already inner-txn
    }

    emit_index_progress(app, IndexStage::Linking, ...);
    {
        let fts = state.fts.lock().await;
        fts::delete_chunks(...)?;
        fts::replace_chunks(...)?;
    }
}
```

The critical change is *per-row* writes becoming batched. The DB mutex is
still held across all five inner transactions for one file — that's
acceptable because the inner txns now complete in seconds rather than
minutes. The mutex is released between files, so the UI thaws as each
file finishes.

## AppState changes

```rust
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub fts: Arc<Mutex<tantivy::Index>>,
    pub embedder: Arc<Mutex<TextEmbedding>>,
    pub indexing: Arc<Mutex<bool>>,
    pub cancel_token: Arc<AtomicBool>,   // NEW
}
```

## Failure semantics

- **Preprocess fails** (Whisper crashes, OCR returns empty / errors,
  ffmpeg can't find audio): record file as `status='error'` with
  `error_msg = "<kind> preprocessing failed: <reason>"`. Skip Stage C
  for that file. User sees it in the documents list and can retry.
- **Indexing fails**: existing behavior — record error doc, continue.
- **Cancel mid-batch**: emit `IndexStatus::Cancelled`, return Ok.
  Completed files stay indexed; uncompleted files are unchanged.
- **App crash mid-pre-pass**: sidecars are written atomically (`*.tmp`
  → rename). Partial sidecars don't exist; the file is re-preprocessed
  on next run.

## Re-run logic

Re-running `index_folder` on a folder where all media is already
preprocessed:

- Stage B walks the files, sees all sidecars are fresh, emits
  `PreprocessStage::CachedSkipped` for each. Completes in milliseconds.
- Stage C indexes everything. Already-indexed files with matching hash
  are short-circuited (existing logic).

## Test plan (TDD)

Each fix and each new function gets a failing test first.

### Perf fixes

- `store::entities::insert_entities`: timing test — insert 5,000 hits;
  must complete under 200 ms in-memory. Today's per-row pattern would
  fail this (or take seconds on disk).
- `store::vectors::upsert_vectors_batch`: timing test, same shape, 5,000
  vectors.
- `entities::extract_anchors`: input with 50 anchor-shaped IDs in one
  chunk → at most 20 hits returned.
- `parser::json::parse`: 1 MB synthetic JSON → returns > 1 page; each
  page text ≤ 16 KB; total chunk count is bounded.

### Preprocess module

- `needs_preprocessing`: table-driven over each format.
- `sidecar_fresh`: positive (newer/equal) and negative (older) cases.
- `run_preprocessing` integration: temp dir with mock video that has a
  pre-written sidecar → `skipped_fresh` populated, transcription not
  invoked.

### Cancellation

- `engine::indexer::index_folder` with cancel_token preset to `true` →
  returns immediately with `IndexStatus::Cancelled`, no DB writes.

### Indexer

- `index_one` writes are wrapped in a single transaction — assert by
  injecting a forced error after `insert_entities` and confirming the
  chunks/vectors/entities rolled back together.

## Migration / rollout

- No schema changes — pure code refactor.
- `IndexProgress.stage` is optional, defaults to `None` for old emitters
  → existing UI keeps working until updated.
- `AppState.cancel_token` initializes to `false`; existing callers see
  no behavior change.
- Sidecar `.anubis.txt` files written by the new image OCR path follow
  the existing `is_engine_output` filename rule, so they're already
  skipped by the indexer walk.

## UI changes (frontend)

- `IndexStatus.tsx` adds a second `Progress` row when `preprocess-progress`
  is active. Shows e.g. "Preprocessing 3 of 12 · transcribing big.mp4".
- When pre-pass finishes, switches to the existing index-progress row
  with stage labels: "Indexing 50 of 89 · writing big.json".
- Adds a "Cancel" button next to the progress bar that invokes
  `cancel_indexing`. The button is enabled whenever a stage is running.

## Out of scope (deferred)

- PDF page-image OCR. `PreprocessKind::Pdf` is reserved but unused.
- Parallel preprocessing across files.
- Resumable mid-file indexing.
- Replacing the per-entity edge-builder query loops with a single
  GROUP BY query. With Fix 1+2 in place, the loop's ~30K SELECTs cost
  ~3 seconds — acceptable.
- A separate `preprocess-only` Tauri command. Folded into `index_folder`
  for now; can split later if a use case emerges.

## Why this lands as one change

The pre-pass refactor and the JSON-hang fix share the indexer hot path
and the UI's progress contract. Shipping them separately would mean
either:

- Pre-pass alone: JSON still hangs because the index pass still holds
  the mutex for minutes.
- Hang fix alone: indexer is fast, but transcription/OCR still runs
  inline inside the parser, so the user can't see the slow stages, can't
  cancel, and can't tell whether the freeze is gone.

Together they give the user a system that (a) doesn't freeze, (b) shows
honest progress, (c) caches preprocessed artifacts, and (d) can be
cancelled. The combined surface area is still small — ~6 files touched.
