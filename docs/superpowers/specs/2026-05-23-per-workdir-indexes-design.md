# Per-Workdir Indexes — Design

**Status:** Approved (brainstorming complete, ready for implementation plan)
**Date:** 2026-05-23

## Problem

Anubis currently maintains a single global index (`anubis.db` + `fts_index`) at the Tauri `app_data_dir`. All indexed folders accumulate into that one store, with no way to separate corpora. Users who want distinct indexes per project must either reset the global index or accept cross-project contamination.

## Goal

Allow callers to address multiple independent indexes — one per "workdir" — through the Tauri commands, MCP tools, and the existing frontend.

## Non-Goals

- Migrating the existing global `anubis.db` / `fts_index` into a workdir. The legacy files at `%APPDATA%/anubis/anubis.db` and `%APPDATA%/anubis/fts_index/` are left untouched; the new code path does not read or write them. Users can delete them manually.
- Sharing or syncing indexes across workdirs.
- Per-workdir model selection. The embedder, OCR engine, and transcription engine remain global (single copy in memory, shared across workdirs).
- A single-active-workdir mode. Multiple workdirs may be loaded simultaneously and addressed independently per call.

## User Model

Multi-workdir, addressable per call. Every index/query call carries a `workdir` parameter (an absolute or user-relative directory path). The engine routes the call to that workdir's storage. The frontend tracks one *active* workdir in UI state purely for convenience and to scope visible content — the engine itself never has a notion of "active".

## Architecture

### Workdir Identity

A workdir is identified by its canonical filesystem path. The on-disk storage directory is derived from a hash of that path:

```
%APPDATA%/anubis/workdirs/<sha256(canonical_path)[..16]>/
  ├── anubis.db
  ├── fts_index/
  └── meta.json   # { canonical_path, created_at, last_used }
```

- 16 hex chars (64 bits) is sufficient — collision probability is negligible for the expected scale (hundreds of workdirs per user).
- `meta.json` is convenience metadata for `list_workdirs`. It is **not** load-bearing for routing — losing it does not break access; the next call simply rewrites it.
- Storage directory is created lazily on first use of that workdir.

### Workdir Registry

The Tauri-managed engine handle no longer holds a single `AppState`. It holds a `WorkdirRegistry`:

```rust
pub struct WorkdirRegistry {
    states: RwLock<HashMap<WorkdirId, Arc<AppState>>>,  // lazy cache, no eviction
    embedder: Arc<LocalEmbedder>,                       // shared across all workdirs
    ocr:      Arc<OcrEngine>,                           // shared
    transcription: Arc<TranscriptionEngine>,            // shared
    root: PathBuf,                                      // %APPDATA%/anubis/workdirs
}
```

`AppState` shrinks: it no longer owns the embedder / OCR / transcription engines (those become global, injected on construction). It owns DB pool + FTS writer/reader + entity store, all keyed to one workdir storage directory.

### Resolution Flow

Every command/tool that touches an index:

```text
canonical_path = canonicalize(workdir_arg)            // fails → NotCanonical / NotFound
id             = sha256(canonical_path)[..16]
state          = registry.get_or_load(id, canonical_path)  // lazy, idempotent
```

`get_or_load` is thread-safe. First call constructs `AppState` against `root/<id>/`, creating the dir, running schema migrations, opening FTS, writing `meta.json`. Subsequent calls hand back the cached `Arc<AppState>`. No LRU eviction — cached states live for the process lifetime (decision: simplicity over bounded memory; acceptable given typical workdir counts per session).

## API Surface

### Workdir Parameter Shape

A single string field on every relevant call: an absolute or user-relative path to a directory. The engine canonicalizes it before hashing. The directory must exist on disk; if not, returns `WorkdirError::NotFound`. The workdir's storage dir is created lazily.

### Tauri Commands

Every command in [src-tauri/src/commands/](../../../src-tauri/src/commands) that touches an index gains a leading `workdir: String` arg:

| Command | After |
|---|---|
| `index_folder` | `(workdir, path)` |
| `index_file` | `(workdir, path)` |
| `cancel_indexing` | `(workdir)` |
| `remove_document` | `(workdir, doc_id)` |
| `reset_index` | `(workdir)` |
| `query` | `(workdir, req)` |
| `get_chunk_neighbors` | `(workdir, ...)` |
| `get_graph_overview` | `(workdir)` |
| `get_graph_neighborhood` | `(workdir, ...)` |
| `get_search_neighborhood` | `(workdir, ...)` |
| `get_doc_chunks` | `(workdir, doc_id)` |
| `get_index_stats` | `(workdir)` |
| `list_documents` | `(workdir)` |

Unchanged (global): `engine_ready`, `get_settings`, `set_transcription_enabled`.

### New Tauri Commands

- `list_workdirs() -> Vec<WorkdirInfo>` — reads `%APPDATA%/anubis/workdirs/*/meta.json` and returns `[{id, path, last_used, doc_count}]`. Used by the frontend picker.
- `delete_workdir(workdir: String) -> ()` — evicts the workdir's `AppState` from the registry, drops the `Arc`, removes the storage directory. Required for cleanup of stale workdirs.

### MCP Tools

Every `anubis_*` tool in [src-tauri/src/mcp/tools.rs](../../../src-tauri/src/mcp/tools.rs) that touches an index gains a required `workdir` string input. JSON schemas are updated. Tool descriptions document that `workdir` is the project root the index belongs to.

## Frontend

### State

A single React context (`WorkdirContext`) at [src/App.tsx](../../../src/App.tsx) holding `{ activeWorkdir, setActiveWorkdir, knownWorkdirs, refreshKnownWorkdirs }`. All existing components (`SearchBar`, `IndexStatus`, `KnowledgeBrowser`, `GraphVisualizer`) consume it via a `useWorkdir()` hook and pass `activeWorkdir` to every Tauri `invoke()` call.

### Persistence

Last-active workdir is persisted in `localStorage` under `anubis.activeWorkdir`. On app boot the context hydrates from localStorage, then calls `list_workdirs` to populate the picker. If the stored path no longer appears in `list_workdirs`, the active selection is cleared and the user is prompted to pick one.

### Picker UI

A new `WorkdirSwitcher` component in the app header (top-right of [src/App.tsx](../../../src/App.tsx)):

- Displays the current workdir's basename + truncated path.
- Dropdown shows all known workdirs with `last_used` timestamps and doc counts.
- "Add workdir…" item opens the Tauri `dialog` folder picker (`@tauri-apps/plugin-dialog`, already a dependency), then sets it active. No backend call is needed to "register" — the first `index_*` call creates the storage dir lazily, and `list_workdirs` reflects it on next refresh.
- "Remove…" item per entry calls `delete_workdir`, with a confirm step.

### Empty State

When `activeWorkdir` is `null`, the existing index/search/graph panels render a placeholder ("Pick or add a workdir to get started") instead of their normal content. The model-download banner and engine-readiness state are unaffected (they're global).

## Error Handling

New error variants in the engine error enum (alongside existing `Io`):

- `WorkdirError::NotFound { path }` — path doesn't exist or isn't a directory.
- `WorkdirError::NotCanonical { path, source }` — `canonicalize` failed (permission, broken symlink, etc.).
- `WorkdirError::StorageInit { id, source }` — failed to create/open the storage dir or DB pool for that workdir.

All three surface as Tauri command errors and MCP tool error responses with a stable `code` field so the frontend can show actionable messages (e.g. "This folder no longer exists — remove it from your list?").

## Events

The existing `events::emit_*` functions in [src-tauri/src/engine/events.rs](../../../src-tauri/src/engine/events.rs) emit globally to the Tauri window. Indexing progress events gain a `workdir_id` field in their payload, so the frontend can ignore events not belonging to the active workdir. Engine readiness and model-download events stay workdir-agnostic.

## Concurrency

- Two simultaneous `index_folder` calls into the **same** workdir are serialized by the existing per-`AppState` locking (no change).
- Two calls into **different** workdirs run in parallel against separate `AppState`s. They share the embedder (already `Send + Sync`) but write to independent DB pools / FTS indexes.

## Testing

Three new integration tests under `src-tauri/tests/`:

1. **`workdir_isolation.rs`** — create two temp workdirs, index different files into each, assert that `list_documents(A)` and `list_documents(B)` are disjoint and that `search(A, q)` never returns hits from B.
2. **`workdir_lazy_load.rs`** — call `engine_ready`, assert no `AppState`s exist yet; call `index_file(w1, ...)` then `index_file(w2, ...)`, assert the registry holds exactly two `AppState`s.
3. **`workdir_errors.rs`** — call `index_folder` with (a) a nonexistent path → `NotFound`, (b) a file rather than a directory → `NotFound`, (c) a path that canonicalizes to the same dir as an existing workdir → resolves to the *same* `AppState` (idempotent).

Existing tests that hard-coded a single `AppState` get a fixture helper `tempfile_workdir()` returning a `TempDir` + canonical path.

## Out of Scope (Future Work)

- Migrating the legacy global index into a default workdir.
- LRU eviction for the registry cache.
- Cross-workdir search ("search all my workdirs at once").
- Workdir labels / nicknames (currently identified by path only).
