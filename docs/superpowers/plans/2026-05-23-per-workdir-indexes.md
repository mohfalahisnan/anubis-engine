# Per-Workdir Indexes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route every Anubis `index_*` / `query` / graph call to a per-workdir storage directory under `%APPDATA%/anubis/workdirs/<sha256(path)[..16]>/`, addressable per call via a `workdir` parameter.

**Architecture:** Replace the single Tauri-managed `EngineHandle` with a `WorkdirRegistry` that lazily constructs and caches one `AppState` per canonicalized workdir path. Shared resources (embedder, OCR engine, transcription engine) are hoisted out of `AppState` and held once on the registry. Tauri commands + MCP tools gain a `workdir: String` parameter; the React frontend tracks an `activeWorkdir` in context and passes it on every `invoke()`.

**Tech Stack:** Rust (Tauri 2, rusqlite, tantivy, fastembed, tokio), TypeScript + React 18, shadcn/ui, `@tauri-apps/plugin-dialog`.

**Spec:** [`docs/superpowers/specs/2026-05-23-per-workdir-indexes-design.md`](../specs/2026-05-23-per-workdir-indexes-design.md)

---

## File Structure

### New files

| Path | Responsibility |
|---|---|
| `src-tauri/src/engine/workdir.rs` | `WorkdirId`, `WorkdirInfo`, `resolve_id()` helper, `WorkdirError` variants, hash/canonicalize logic |
| `src-tauri/src/engine/registry.rs` | `WorkdirRegistry` struct: shared engines + `Mutex<HashMap<WorkdirId, Arc<AppState>>>` + `get_or_load` / `list` / `delete` |
| `src-tauri/src/commands/workdir_commands.rs` | `list_workdirs`, `delete_workdir` Tauri commands |
| `src-tauri/tests/workdir_isolation.rs` | Two-workdir disjoint corpus test |
| `src-tauri/tests/workdir_lazy_load.rs` | Registry cache population test |
| `src-tauri/tests/workdir_errors.rs` | `NotFound`, `NotCanonical`, idempotent canonicalize |
| `src/contexts/WorkdirContext.tsx` | React context + `useWorkdir()` hook + types |
| `src/components/WorkdirSwitcher.tsx` | Header dropdown: switch, add, remove |

### Modified files

| Path | Change |
|---|---|
| `src-tauri/src/lib.rs` | `EngineError` gets `Workdir(WorkdirError)`; setup builds shared engines + `WorkdirRegistry`; new commands registered |
| `src-tauri/src/engine/mod.rs` | Add `pub mod workdir; pub mod registry;` |
| `src-tauri/src/engine/state.rs` | `AppState::new` takes injected `Arc` engines; remove `pub embedder` field, keep `embedder()` accessor on `AppState` that returns the shared handle |
| `src-tauri/src/engine/indexer.rs` | `index_folder` / `index_file` accept `workdir_id: &WorkdirId` and propagate into `IndexProgress` events |
| `src-tauri/src/engine/events.rs` | Add `workdir_id` field on `IndexProgress` event (via `types.rs`) |
| `src-tauri/src/types.rs` | `IndexProgress` gains `workdir_id: Option<String>` |
| `src-tauri/src/commands/mod.rs` | Replace `engine_or_error(EngineHandle)` with `registry_or_error(WorkdirRegistry)` + `workdir_state(registry, workdir) -> Arc<AppState>` |
| `src-tauri/src/commands/index_commands.rs` | All 5 commands get `workdir: String` param |
| `src-tauri/src/commands/query_commands.rs` | All 6 commands get `workdir: String` param |
| `src-tauri/src/commands/status_commands.rs` | `get_index_stats`, `list_documents` get `workdir`; `engine_ready`, `get_settings`, `set_transcription_enabled` unchanged |
| `src-tauri/src/mcp/mod.rs` | `run_async` builds shared engines + `WorkdirRegistry`; `handle_request` / `call_tool` get registry |
| `src-tauri/src/mcp/tools.rs` | All index-touching tools gain required `workdir` input; dispatch resolves via registry |
| `src/App.tsx` | Wrap in `<WorkdirProvider>`; consume `useWorkdir()`; pass `activeWorkdir` to every `invoke`; empty state when null |
| `src/components/IndexStatus.tsx` | Take `workdir` from context; pass to invoke calls |
| `src/components/SearchBar.tsx` | Same |
| `src/components/KnowledgeBrowser.tsx` | Same |
| `src/components/GraphVisualizer.tsx` | No direct invokes (callbacks only) — verify; no change expected |

---

## Task 1: Add `WorkdirError` + ID/hash helper

**Files:**
- Create: `src-tauri/src/engine/workdir.rs`
- Modify: `src-tauri/src/engine/mod.rs:1-7`
- Modify: `src-tauri/src/lib.rs:20-41`

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/engine/workdir.rs`:

```rust
//! Per-workdir identity, hashing, and error types. A workdir is identified
//! by its canonical absolute filesystem path; storage on disk is keyed by
//! the first 16 hex chars of sha256(canonical_path).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum WorkdirError {
    #[error("Workdir not found or not a directory: {path}")]
    NotFound { path: String },
    #[error("Workdir path is not absolute or could not be canonicalised ({path}): {source}")]
    NotCanonical {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to initialise storage for workdir {id} at {path}: {source}")]
    StorageInit {
        id: String,
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Stable 16-hex-char identifier derived from sha256(canonical_path).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkdirId(String);

impl WorkdirId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkdirId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkdirInfo {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub last_used: String,
    pub doc_count: Option<i64>,
}

/// Validate and canonicalise a workdir path, then return the canonical path
/// and a stable 16-hex-char id. Returns `NotFound` if the path doesn't exist
/// or isn't a directory; returns `NotCanonical` on permission errors / broken
/// symlinks / relative-path inputs that fail to resolve.
pub fn resolve(input: &str) -> Result<(PathBuf, WorkdirId), WorkdirError> {
    let raw = Path::new(input);
    if !raw.exists() {
        return Err(WorkdirError::NotFound {
            path: input.to_string(),
        });
    }
    if !raw.is_dir() {
        return Err(WorkdirError::NotFound {
            path: input.to_string(),
        });
    }
    let canonical =
        std::fs::canonicalize(raw).map_err(|source| WorkdirError::NotCanonical {
            path: input.to_string(),
            source,
        })?;

    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    let id = WorkdirId(hex[..16].to_string());
    Ok((canonical, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_stable_id_for_same_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (_, id_a) = resolve(dir.path().to_str().expect("utf8")).expect("resolve");
        let (_, id_b) = resolve(dir.path().to_str().expect("utf8")).expect("resolve");
        assert_eq!(id_a, id_b);
        assert_eq!(id_a.as_str().len(), 16);
    }

    #[test]
    fn resolve_rejects_nonexistent_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("does-not-exist");
        let result = resolve(missing.to_str().expect("utf8"));
        assert!(matches!(result, Err(WorkdirError::NotFound { .. })));
    }

    #[test]
    fn resolve_rejects_file_not_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "hi").expect("write");
        let result = resolve(file.to_str().expect("utf8"));
        assert!(matches!(result, Err(WorkdirError::NotFound { .. })));
    }
}
```

- [ ] **Step 2: Wire the module + dep**

Edit `src-tauri/src/engine/mod.rs` (currently lines 1-7) to:

```rust
pub mod download;
pub mod events;
pub mod indexer;
pub mod preprocess;
pub mod registry;
pub mod settings;
pub mod sidecar;
pub mod state;
pub mod workdir;
```

(Note: `registry` module is added now; its body is empty in this task and filled in Task 3. Add a one-line placeholder file `src-tauri/src/engine/registry.rs` containing `// filled in by Task 3` so the build passes.)

Add `sha2` to `src-tauri/Cargo.toml` under `[dependencies]` if not already present:

```bash
cd src-tauri && cargo add sha2 && cargo add --dev tempfile && cd ..
```

If `tempfile` is already a dev-dependency, the second `cargo add --dev` is a no-op.

- [ ] **Step 3: Extend `EngineError` to wrap `WorkdirError`**

Edit `src-tauri/src/lib.rs` lines 20-41 — add a `Workdir` variant at the end of the enum (before the closing brace):

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Parse error for {path}: {msg}")]
    Parse { path: String, msg: String },
    #[error("Embed error: {0}")]
    Embed(String),
    #[error("OCR error: {0}")]
    Ocr(String),
    #[error("Transcription error: {0}")]
    Transcribe(String),
    #[error("No audio track in {0}")]
    NoAudioTrack(String),
    #[error("Index already running")]
    AlreadyIndexing,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Workdir error: {0}")]
    Workdir(#[from] crate::engine::workdir::WorkdirError),
}
```

- [ ] **Step 4: Run the test**

```bash
cd src-tauri && cargo test --lib engine::workdir::tests
```

Expected: 3 tests pass (`resolve_returns_stable_id_for_same_path`, `resolve_rejects_nonexistent_path`, `resolve_rejects_file_not_dir`).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/engine/mod.rs src-tauri/src/engine/workdir.rs src-tauri/src/engine/registry.rs src-tauri/src/lib.rs
git commit -m "feat(engine): add WorkdirId + canonical path resolver"
```

---

## Task 2: Extract shared engines from `AppState`

**Files:**
- Modify: `src-tauri/src/engine/state.rs:25-81`
- Modify: `src-tauri/src/lib.rs:75-89`
- Modify: `src-tauri/src/mcp/mod.rs:56-65`

Goal: `AppState` no longer constructs the embedder/OCR/transcription on its own — it accepts injected handles. The two existing callers (Tauri `lib.rs` setup, MCP `run_async`) construct the shared engines first, then build `AppState` against a single workdir. This keeps the build green; per-workdir routing comes in Task 6.

- [ ] **Step 1: Rewrite `AppState::new` to take injected engines**

Replace the body of `AppState` and `impl AppState` in `src-tauri/src/engine/state.rs` (lines 25-81) with:

```rust
pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    pub fts: Arc<Mutex<tantivy::Index>>,
    pub graph: Arc<Mutex<petgraph::Graph<String, f32>>>,
    pub indexing: Arc<Mutex<bool>>,
    pub cancel_token: Arc<AtomicBool>,
}

impl AppState {
    /// Build an `AppState` for one workdir's storage. The embedder is shared
    /// across workdirs (one model loaded into memory once) and is supplied
    /// by the caller. OCR + transcription models are configured globally on
    /// startup before any `AppState` is built.
    pub fn new(
        db_path: &std::path::Path,
        fts_path: &std::path::Path,
        embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    ) -> Result<Self, EngineError> {
        let db = store::db::open(db_path)?;
        crate::engine::settings::load_from_db(&db)?;
        invalidate_on_model_change(&db, EMBEDDING_MODEL_ID)?;
        invalidate_on_chunk_signal_change(&db, CHUNK_SIGNAL_MODEL_ID)?;

        let fts = store::fts::open_or_create(fts_path)?;
        store::fts::rebuild_from_indexed_chunks(&fts, &db)?;
        tracing::info!("reconciled full-text index from SQLite chunks");

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            embedder,
            fts: Arc::new(Mutex::new(fts)),
            graph: Arc::new(Mutex::new(petgraph::Graph::new())),
            indexing: Arc::new(Mutex::new(false)),
            cancel_token: Arc::new(AtomicBool::new(false)),
        })
    }
}
```

The unused `embedder` import at the top of `state.rs` can stay; it's still referenced by the model-change helpers? Check — actually it isn't. If `cargo build` warns, delete the `use crate::{embedder, store, EngineError};` import of `embedder` and replace with `use crate::{store, EngineError};`.

- [ ] **Step 2: Add a model-dir + embedder bootstrap helper**

Append to `src-tauri/src/engine/state.rs`:

```rust
/// Configure global model directories and load the embedder. Runs once on
/// startup before any `AppState` is constructed. The returned handle is
/// shared across all per-workdir `AppState`s built by [`WorkdirRegistry`].
pub fn bootstrap_shared_engines(
    models_dir: &std::path::Path,
) -> Result<Arc<Mutex<fastembed::TextEmbedding>>, EngineError> {
    crate::ocr::engine::set_models_dir(models_dir.to_path_buf());
    crate::embedder::download::set_models_dir(models_dir.to_path_buf());
    crate::transcription::engine::set_models_dir(models_dir.to_path_buf());

    let embedder = crate::embedder::download::load_or_download()?;
    Ok(Arc::new(Mutex::new(embedder)))
}
```

- [ ] **Step 3: Update `lib.rs` setup to use the new signature**

In `src-tauri/src/lib.rs` lines 75-89, replace the worker-thread body:

```rust
            std::thread::spawn(move || {
                events::emit_starting("engine", "Engine bootstrap");
                let models_dir = app_data.clone();
                let embedder = match crate::engine::state::bootstrap_shared_engines(&models_dir) {
                    Ok(handle) => handle,
                    Err(error) => {
                        tracing::error!("engine bootstrap failed: {error}");
                        events::emit_error("engine", "Engine bootstrap", error.to_string());
                        return;
                    }
                };
                match AppState::new(&db_path, &fts_path, embedder) {
                    Ok(state) => {
                        if engine.set(state).is_err() {
                            tracing::warn!("engine handle already initialised");
                        }
                        events::emit_ready("engine", "Engine bootstrap");
                    }
                    Err(error) => {
                        tracing::error!("engine init failed: {error}");
                        events::emit_error("engine", "Engine bootstrap", error.to_string());
                    }
                }
            });
```

- [ ] **Step 4: Update MCP `run_async` to use the new signature**

In `src-tauri/src/mcp/mod.rs` lines 56-65, replace the AppState construction:

```rust
async fn run_async() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let fts_path = get_fts_path(&db_path);

    let models_dir = db_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let embedder = crate::engine::state::bootstrap_shared_engines(&models_dir)
        .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
    let state = AppState::new(&db_path, &fts_path, embedder)
        .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
```

(Keep the rest of `run_async` unchanged.)

- [ ] **Step 5: Build everything**

```bash
cd src-tauri && cargo build
```

Expected: builds without errors. Warnings about unused imports are fine here.

- [ ] **Step 6: Run existing tests**

```bash
cd src-tauri && cargo test --lib
```

Expected: all previously passing tests still pass. (Most are `placeholder_compiles`; the new `engine::workdir::tests` from Task 1 also pass.)

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/engine/state.rs src-tauri/src/lib.rs src-tauri/src/mcp/mod.rs
git commit -m "refactor(engine): inject shared embedder into AppState

Hoist embedder / OCR / transcription bootstrap out of AppState::new
so a single shared embedder can be reused across multiple per-workdir
AppStates. AppState now takes the embedder handle as a constructor arg.
No behaviour change yet; per-workdir routing follows in a later commit."
```

---

## Task 3: Implement `WorkdirRegistry`

**Files:**
- Modify: `src-tauri/src/engine/registry.rs` (replace placeholder)

- [ ] **Step 1: Write the failing test**

Replace the contents of `src-tauri/src/engine/registry.rs` with:

```rust
//! Lazy cache of per-workdir [`AppState`]s. Shared resources (embedder)
//! live on the registry once. Each `get_or_load(workdir)` call resolves the
//! workdir, constructs `AppState` against `<root>/<id>/` on first use, and
//! returns the cached `Arc<AppState>` on subsequent calls. No eviction —
//! cached states live for the process lifetime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::engine::state::AppState;
use crate::engine::workdir::{self, WorkdirError, WorkdirId, WorkdirInfo};
use crate::EngineError;

#[derive(Debug, Serialize, Deserialize)]
struct WorkdirMeta {
    canonical_path: String,
    created_at: String,
    last_used: String,
}

pub struct WorkdirRegistry {
    root: PathBuf,
    embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    states: Mutex<HashMap<WorkdirId, Arc<AppState>>>,
}

impl WorkdirRegistry {
    pub fn new(root: PathBuf, embedder: Arc<Mutex<fastembed::TextEmbedding>>) -> Self {
        Self {
            root,
            embedder,
            states: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn embedder(&self) -> &Arc<Mutex<fastembed::TextEmbedding>> {
        &self.embedder
    }

    /// Resolve `workdir_input`, then return the cached or newly-built
    /// `AppState` for that workdir. Updates `meta.json::last_used` on every
    /// call. First call for a workdir creates `<root>/<id>/`, runs schema
    /// migrations, opens FTS, and writes `meta.json`.
    pub async fn get_or_load(
        &self,
        workdir_input: &str,
    ) -> Result<(WorkdirId, Arc<AppState>), EngineError> {
        let (canonical, id) = workdir::resolve(workdir_input)?;

        {
            let states = self.states.lock().await;
            if let Some(existing) = states.get(&id) {
                drop(states);
                self.touch_last_used(&id, &canonical)?;
                return Ok((id, existing.clone()));
            }
        }

        let storage_dir = self.root.join(id.as_str());
        std::fs::create_dir_all(&storage_dir).map_err(|source| WorkdirError::StorageInit {
            id: id.to_string(),
            path: storage_dir.to_string_lossy().into_owned(),
            source,
        })?;
        let db_path = storage_dir.join("anubis.db");
        let fts_path = storage_dir.join("fts_index");

        let state = AppState::new(&db_path, &fts_path, self.embedder.clone())?;
        let state = Arc::new(state);

        let mut states = self.states.lock().await;
        let entry = states.entry(id.clone()).or_insert_with(|| state.clone());
        let returned = entry.clone();
        drop(states);

        self.write_meta(&id, &canonical, /*created=*/ true)?;
        Ok((id, returned))
    }

    /// Drop the cached state, remove storage on disk. Errors if the workdir
    /// id is unknown on disk (no-op for an id that was never loaded).
    pub async fn delete(&self, workdir_input: &str) -> Result<(), EngineError> {
        let (_, id) = workdir::resolve(workdir_input)?;
        {
            let mut states = self.states.lock().await;
            states.remove(&id);
        }
        let storage_dir = self.root.join(id.as_str());
        if storage_dir.exists() {
            std::fs::remove_dir_all(&storage_dir).map_err(|source| WorkdirError::StorageInit {
                id: id.to_string(),
                path: storage_dir.to_string_lossy().into_owned(),
                source,
            })?;
        }
        Ok(())
    }

    /// List every workdir that has a directory on disk under `root`. Reads
    /// `meta.json` for each entry; entries missing or with unreadable meta
    /// are skipped (logged at warn). `doc_count` is `None` here — callers
    /// that want it open the workdir's DB.
    pub fn list(&self) -> Result<Vec<WorkdirInfo>, EngineError> {
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let meta_path = entry.path().join("meta.json");
            let raw = match std::fs::read_to_string(&meta_path) {
                Ok(raw) => raw,
                Err(error) => {
                    tracing::warn!("skipping workdir {:?}: {}", entry.path(), error);
                    continue;
                }
            };
            let meta: WorkdirMeta = match serde_json::from_str(&raw) {
                Ok(meta) => meta,
                Err(error) => {
                    tracing::warn!("skipping workdir {:?}: {}", entry.path(), error);
                    continue;
                }
            };
            out.push(WorkdirInfo {
                id: entry.file_name().to_string_lossy().into_owned(),
                path: meta.canonical_path,
                created_at: meta.created_at,
                last_used: meta.last_used,
                doc_count: None,
            });
        }
        out.sort_by(|a, b| b.last_used.cmp(&a.last_used));
        Ok(out)
    }

    fn touch_last_used(&self, id: &WorkdirId, canonical: &Path) -> Result<(), EngineError> {
        self.write_meta(id, canonical, /*created=*/ false)
    }

    fn write_meta(
        &self,
        id: &WorkdirId,
        canonical: &Path,
        created: bool,
    ) -> Result<(), EngineError> {
        let meta_path = self.root.join(id.as_str()).join("meta.json");
        let now = Utc::now().to_rfc3339();
        let existing = std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<WorkdirMeta>(&raw).ok());
        let meta = WorkdirMeta {
            canonical_path: canonical.to_string_lossy().into_owned(),
            created_at: existing
                .as_ref()
                .map(|m| m.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            last_used: now,
        };
        let _ = created;
        let raw = serde_json::to_string_pretty(&meta).map_err(|error| {
            WorkdirError::StorageInit {
                id: id.to_string(),
                path: meta_path.to_string_lossy().into_owned(),
                source: std::io::Error::new(std::io::ErrorKind::Other, error.to_string()),
            }
        })?;
        std::fs::write(&meta_path, raw).map_err(|source| WorkdirError::StorageInit {
            id: id.to_string(),
            path: meta_path.to_string_lossy().into_owned(),
            source,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh_registry() -> (tempfile::TempDir, WorkdirRegistry) {
        let root = tempfile::tempdir().expect("tempdir for root");
        let models_dir = root.path().join("models");
        std::fs::create_dir_all(&models_dir).expect("models dir");
        let embedder = crate::engine::state::bootstrap_shared_engines(&models_dir)
            .expect("bootstrap embedder");
        let registry = WorkdirRegistry::new(root.path().join("workdirs"), embedder);
        (root, registry)
    }

    #[tokio::test]
    async fn get_or_load_is_idempotent_for_same_path() {
        let (_root, registry) = fresh_registry().await;
        let workdir = tempfile::tempdir().expect("workdir");
        let path = workdir.path().to_str().expect("utf8");

        let (id_a, state_a) = registry.get_or_load(path).await.expect("first load");
        let (id_b, state_b) = registry.get_or_load(path).await.expect("second load");

        assert_eq!(id_a, id_b);
        assert!(Arc::ptr_eq(&state_a, &state_b), "states must be cached");
    }

    #[tokio::test]
    async fn list_reflects_loaded_workdirs() {
        let (_root, registry) = fresh_registry().await;
        let a = tempfile::tempdir().expect("a");
        let b = tempfile::tempdir().expect("b");
        registry.get_or_load(a.path().to_str().unwrap()).await.expect("a");
        registry.get_or_load(b.path().to_str().unwrap()).await.expect("b");

        let entries = registry.list().expect("list");
        assert_eq!(entries.len(), 2);
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&a.path().canonicalize().unwrap().to_str().unwrap()));
        assert!(paths.contains(&b.path().canonicalize().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn delete_removes_storage_and_cache() {
        let (_root, registry) = fresh_registry().await;
        let workdir = tempfile::tempdir().expect("workdir");
        let path = workdir.path().to_str().expect("utf8");

        let (id, _) = registry.get_or_load(path).await.expect("load");
        let storage = registry.root().join(id.as_str());
        assert!(storage.exists());

        registry.delete(path).await.expect("delete");
        assert!(!storage.exists());
        let states = registry.states.lock().await;
        assert!(!states.contains_key(&id));
    }
}
```

- [ ] **Step 2: Run the test**

```bash
cd src-tauri && cargo test --lib engine::registry::tests
```

Expected: 3 tests pass. **Note:** these tests trigger a real fastembed model download on first run if the model isn't already cached. Allow up to a few minutes the first time; subsequent runs reuse the cache under `<tempdir>/models`. If running offline, mark these tests `#[ignore]` and document in the commit.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/engine/registry.rs
git commit -m "feat(engine): WorkdirRegistry with lazy AppState cache

Caches one Arc<AppState> per canonicalised workdir under
<app_data>/workdirs/<id>/. Shared embedder is held once on the
registry and injected into each AppState. Includes list/delete
and meta.json bookkeeping for the UI workdir picker."
```

---

## Task 4: Wire `WorkdirRegistry` into Tauri setup

**Files:**
- Modify: `src-tauri/src/lib.rs:50-92`
- Modify: `src-tauri/src/engine/state.rs:1-17`

Replace the `EngineHandle = OnceCell<AppState>` with `EngineHandle = OnceCell<Arc<WorkdirRegistry>>`. Setup builds the shared embedder, then constructs the registry — no more eager per-workdir `AppState`.

- [ ] **Step 1: Change the engine handle type**

In `src-tauri/src/engine/state.rs` lines 1-17, replace:

```rust
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::engine::registry::WorkdirRegistry;
use crate::{store, EngineError};

/// Shared handle to the workdir registry. `None` while first-run setup is
/// still in progress (embedder download). Once set, commands resolve a
/// per-workdir [`AppState`] via `registry.get_or_load(workdir)`.
pub type EngineHandle = Arc<tokio::sync::OnceCell<Arc<WorkdirRegistry>>>;

pub fn new_engine_handle() -> EngineHandle {
    Arc::new(tokio::sync::OnceCell::new())
}

/// Identifier persisted in index_stats so we can detect a model swap and
/// invalidate stale vectors. Bump the string whenever the embedding model
/// or its prompt template changes.
const EMBEDDING_MODEL_ID: &str = "intfloat/multilingual-e5-small@v1";
const CHUNK_SIGNAL_MODEL_ID: &str = "chunk-signal@v1";
```

- [ ] **Step 2: Update `lib.rs` setup body**

Replace `src-tauri/src/lib.rs` lines 50-92 with:

```rust
fn try_run() -> tauri::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            let workdirs_root = app_data.join("workdirs");
            std::fs::create_dir_all(&workdirs_root)?;

            events::set_app_handle(app.handle().clone());

            let engine = new_engine_handle();
            app.manage(engine.clone());

            let models_dir = app_data.clone();
            std::thread::spawn(move || {
                events::emit_starting("engine", "Engine bootstrap");
                let embedder = match crate::engine::state::bootstrap_shared_engines(&models_dir) {
                    Ok(handle) => handle,
                    Err(error) => {
                        tracing::error!("engine bootstrap failed: {error}");
                        events::emit_error("engine", "Engine bootstrap", error.to_string());
                        return;
                    }
                };
                let registry =
                    std::sync::Arc::new(crate::engine::registry::WorkdirRegistry::new(
                        workdirs_root,
                        embedder,
                    ));
                if engine.set(registry).is_err() {
                    tracing::warn!("engine handle already initialised");
                }
                events::emit_ready("engine", "Engine bootstrap");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::index_commands::index_folder,
            commands::index_commands::index_file,
            commands::index_commands::cancel_indexing,
            commands::index_commands::remove_document,
            commands::index_commands::reset_index,
            commands::query_commands::query,
            commands::query_commands::get_chunk_neighbors,
            commands::query_commands::get_graph_overview,
            commands::query_commands::get_graph_neighborhood,
            commands::query_commands::get_search_neighborhood,
            commands::query_commands::get_doc_chunks,
            commands::status_commands::get_index_stats,
            commands::status_commands::list_documents,
            commands::status_commands::engine_ready,
            commands::status_commands::get_settings,
            commands::status_commands::set_transcription_enabled,
        ])
        .run(tauri::generate_context!())
}
```

Also remove the now-unused import `use crate::engine::state::{new_engine_handle, AppState};` at line 17-18 and replace with:

```rust
use crate::engine::events;
use crate::engine::state::new_engine_handle;
```

- [ ] **Step 3: Add a stopgap helper so the existing commands still compile**

After this task the per-command files still reference `state.get()` and dereference `AppState` fields directly. We'll fix them in Tasks 6-8. To keep `cargo build` green for the moment, replace `src-tauri/src/commands/mod.rs` with:

```rust
pub mod index_commands;
pub mod query_commands;
pub mod status_commands;

use std::sync::Arc;

use tauri::State;

use crate::engine::registry::WorkdirRegistry;
use crate::engine::state::{AppState, EngineHandle};

/// Get the workdir registry from the Tauri-managed handle, or return a
/// user-facing error if first-run setup is still in flight.
pub fn registry_or_error<'a>(
    state: &'a State<'_, EngineHandle>,
) -> Result<&'a Arc<WorkdirRegistry>, String> {
    state
        .get()
        .ok_or_else(|| "Engine still initialising — please wait for setup to finish.".to_string())
}

/// Resolve `workdir` into a cached or lazily-built [`AppState`]. Helper used
/// by every command that touches an index.
pub async fn workdir_state(
    state: &State<'_, EngineHandle>,
    workdir: &str,
) -> Result<Arc<AppState>, String> {
    let registry = registry_or_error(state)?;
    let (_, app_state) = registry
        .get_or_load(workdir)
        .await
        .map_err(|error| error.to_string())?;
    Ok(app_state)
}
```

Delete the previous `engine_or_error` definition.

- [ ] **Step 4: Build — expect it to fail**

```bash
cd src-tauri && cargo build
```

Expected: build fails because `index_commands.rs`, `query_commands.rs`, `status_commands.rs` still call `engine_or_error`. That's the cue for Tasks 6-8. Do not try to make it green here. **Skip Step 5 (commit) for this task — commit at the end of Task 8 once all commands are updated.**

---

## Task 5: Update `index_commands.rs` to use workdir

**Files:**
- Modify: `src-tauri/src/commands/index_commands.rs` (full replace)

- [ ] **Step 1: Replace the file**

Replace the entire contents of `src-tauri/src/commands/index_commands.rs` with:

```rust
use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::commands::workdir_state;
use crate::engine::state::EngineHandle;
use crate::store::{chunks, db, fts};

#[tauri::command]
pub async fn index_folder(
    workdir: String,
    path: String,
    state: State<'_, EngineHandle>,
    app: AppHandle,
) -> Result<(), String> {
    let engine = workdir_state(&state, &workdir).await?;
    crate::engine::indexer::index_folder(&path, &engine, Some(app))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn index_file(
    workdir: String,
    path: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let engine = workdir_state(&state, &workdir).await?;
    crate::engine::indexer::index_file(&path, &engine)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn cancel_indexing(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let engine = workdir_state(&state, &workdir).await?;
    engine.cancel_token.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn remove_document(
    workdir: String,
    doc_id: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let engine = workdir_state(&state, &workdir).await?;
    let chunk_ids = {
        let db = engine.db.lock().await;
        chunks::get_doc_chunks(&db, &doc_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|chunk| chunk.id)
            .collect::<Vec<_>>()
    };

    {
        let fts = engine.fts.lock().await;
        fts::delete_chunks(&fts, &chunk_ids).map_err(|error| error.to_string())?;
    }

    {
        let db = engine.db.lock().await;
        db::delete_document(&db, &doc_id).map_err(|error| error.to_string())
    }
}

#[tauri::command]
pub async fn reset_index(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let engine = workdir_state(&state, &workdir).await?;
    {
        let fts = engine.fts.lock().await;
        fts::clear(&fts).map_err(|error| error.to_string())?;
    }

    {
        let db = engine.db.lock().await;
        db::reset_index(&db).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
```

The engine `indexer::index_folder` / `index_file` now takes `&AppState` (a reference to the `Arc`'s deref target). Since `Arc<AppState>` derefs to `&AppState`, `&engine` is enough.

(No commit yet — Task 8 commits the command-layer refactor in one go.)

---

## Task 6: Update `query_commands.rs` to use workdir

**Files:**
- Modify: `src-tauri/src/commands/query_commands.rs` (full replace)

- [ ] **Step 1: Replace the file**

Replace the entire contents of `src-tauri/src/commands/query_commands.rs` with:

```rust
use tauri::State;

use crate::{
    commands::workdir_state,
    embedder::local,
    engine::state::EngineHandle,
    query::hybrid::{run_query, QueryOpts},
    store::{chunks, graph_store, graph_store::GraphNeighbor, graph_store::GraphOverview},
    types::{Chunk, QueryResult},
};

#[tauri::command]
pub async fn query(
    workdir: String,
    q: String,
    limit: Option<usize>,
    depth: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<Vec<QueryResult>, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let limit = limit.unwrap_or(10);
    let depth = depth.unwrap_or(1).min(3);

    let query_embedding = {
        let mut embedder = engine.embedder.lock().await;
        local::embed_query(&mut embedder, &q).map_err(|e| e.to_string())?
    };

    let db = engine.db.lock().await;
    let fts = engine.fts.lock().await;
    run_query(&db, &fts, &q, &query_embedding, QueryOpts { limit, depth })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_chunk_neighbors(
    workdir: String,
    chunk_id: String,
    depth: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<Vec<GraphNeighbor>, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    graph_store::chunk_neighbors(&db, &chunk_id, depth.unwrap_or(1) * 50)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_graph_overview(
    workdir: String,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    graph_store::graph_overview(&db, limit.unwrap_or(250)).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_graph_neighborhood(
    workdir: String,
    chunk_id: String,
    depth: Option<usize>,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    graph_store::graph_neighborhood(
        &db,
        &chunk_id,
        depth.unwrap_or(2).min(3),
        limit.unwrap_or(160),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_search_neighborhood(
    workdir: String,
    chunk_ids: Vec<String>,
    depth: Option<usize>,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    graph_store::graph_search_neighborhood(
        &db,
        &chunk_ids,
        depth.unwrap_or(1).min(3),
        limit.unwrap_or(200),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_doc_chunks(
    workdir: String,
    doc_id: String,
    state: State<'_, EngineHandle>,
) -> Result<Vec<Chunk>, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    chunks::get_doc_chunks(&db, &doc_id).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
```

---

## Task 7: Update `status_commands.rs` to use workdir

**Files:**
- Modify: `src-tauri/src/commands/status_commands.rs` (full replace)

- [ ] **Step 1: Replace the file**

Replace the entire contents of `src-tauri/src/commands/status_commands.rs` with:

```rust
use serde_json::json;
use tauri::State;

use crate::{
    commands::{registry_or_error, workdir_state},
    engine::{settings as engine_settings, state::EngineHandle},
    store::db,
};

#[tauri::command]
pub async fn get_index_stats(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<serde_json::Value, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    db::get_index_stats(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_documents(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<Vec<serde_json::Value>, String> {
    let engine = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    db::list_documents(&db).map_err(|error| error.to_string())
}

/// Whether the engine has finished its first-run setup (embedder download
/// + registry construction). Workdir-agnostic — the registry is built once
/// per process; individual workdirs are loaded lazily on first use.
#[tauri::command]
pub async fn engine_ready(state: State<'_, EngineHandle>) -> Result<bool, String> {
    Ok(registry_or_error(&state).is_ok())
}

#[tauri::command]
pub async fn get_settings(_state: State<'_, EngineHandle>) -> Result<serde_json::Value, String> {
    Ok(json!({
        "transcription_enabled": engine_settings::transcription_enabled(),
    }))
}

/// Toggle transcription on disk. Persisted in the *first* loaded workdir's
/// DB since the setting is a process-global, not per-workdir, switch. If
/// no workdir is loaded yet we just hold it in memory; the next loaded
/// workdir picks it up via `load_from_db`.
#[tauri::command]
pub async fn set_transcription_enabled(
    enabled: bool,
    state: State<'_, EngineHandle>,
) -> Result<bool, String> {
    let registry = registry_or_error(&state)?;
    let cached_state = {
        let states = registry.states_for_test().await;
        states.values().next().cloned()
    };
    if let Some(engine) = cached_state {
        let db = engine.db.lock().await;
        engine_settings::persist(&db, enabled).map_err(|error| error.to_string())?;
    } else {
        engine_settings::set_in_memory(enabled);
    }
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
```

This introduces two new helpers we need to add:

- [ ] **Step 2: Add `states_for_test` accessor on `WorkdirRegistry`**

Append to `src-tauri/src/engine/registry.rs` (inside `impl WorkdirRegistry`):

```rust
    /// Snapshot the cached states — used by commands that need to act on
    /// "any loaded workdir" (today: global transcription toggle persistence).
    pub async fn states_for_test(
        &self,
    ) -> tokio::sync::MutexGuard<'_, HashMap<WorkdirId, Arc<AppState>>> {
        self.states.lock().await
    }
```

Rename the method to `loaded_states` (the `_for_test` name was misleading — it's used in production):

```rust
    /// Snapshot the cached states — used by commands that need to act on
    /// any loaded workdir (e.g. persisting a global setting). Holds the
    /// inner mutex; callers should release the guard promptly.
    pub async fn loaded_states(
        &self,
    ) -> tokio::sync::MutexGuard<'_, HashMap<WorkdirId, Arc<AppState>>> {
        self.states.lock().await
    }
```

And update `status_commands.rs` Step 1 above: change `registry.states_for_test()` to `registry.loaded_states()`.

- [ ] **Step 3: Add `set_in_memory` to `engine_settings`**

Check `src-tauri/src/engine/settings.rs` — if it already has a global `AtomicBool`, expose an `pub fn set_in_memory(enabled: bool)` that writes to that atomic without touching SQLite. If the module currently only persists via `persist`, add:

```rust
pub fn set_in_memory(enabled: bool) {
    TRANSCRIPTION_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}
```

(Read the existing file before editing — the atomic name may differ; mirror what `transcription_enabled()` reads.)

---

## Task 8: Build the command layer + commit

- [ ] **Step 1: Build**

```bash
cd src-tauri && cargo build
```

Expected: builds without errors. Warnings about unused imports in command files are fine.

- [ ] **Step 2: Run library tests**

```bash
cd src-tauri && cargo test --lib
```

Expected: all `placeholder_compiles` tests pass; `engine::workdir::tests` (3) pass; `engine::registry::tests` (3) pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/engine/registry.rs src-tauri/src/engine/state.rs src-tauri/src/engine/settings.rs src-tauri/src/lib.rs src-tauri/src/commands/
git commit -m "feat(commands): route every Tauri command through WorkdirRegistry

All index/query/status commands now take a workdir: String and
resolve their AppState via registry.get_or_load. engine_ready and
get_settings stay global; set_transcription_enabled persists into
whichever workdir is loaded first (or in memory if none yet)."
```

---

## Task 9: Add `list_workdirs` + `delete_workdir` commands

**Files:**
- Create: `src-tauri/src/commands/workdir_commands.rs`
- Modify: `src-tauri/src/commands/mod.rs:1-3`
- Modify: `src-tauri/src/lib.rs:93-110`

- [ ] **Step 1: Write the new commands file**

Create `src-tauri/src/commands/workdir_commands.rs`:

```rust
use tauri::State;

use crate::commands::registry_or_error;
use crate::engine::state::EngineHandle;
use crate::engine::workdir::WorkdirInfo;
use crate::store::db;

#[tauri::command]
pub async fn list_workdirs(
    state: State<'_, EngineHandle>,
) -> Result<Vec<WorkdirInfo>, String> {
    let registry = registry_or_error(&state)?;
    let mut infos = registry.list().map_err(|error| error.to_string())?;

    // Best-effort doc_count by opening already-loaded states. We avoid
    // forcing a lazy load just for counts — unloaded workdirs return None
    // and the UI can show a "—" placeholder until the user clicks in.
    let loaded = registry.loaded_states().await;
    for info in infos.iter_mut() {
        if let Some(state_arc) = loaded.iter().find_map(|(id, state)| {
            if id.as_str() == info.id {
                Some(state.clone())
            } else {
                None
            }
        }) {
            let db = state_arc.db.lock().await;
            info.doc_count = db::list_documents(&db).ok().map(|docs| docs.len() as i64);
        }
    }
    Ok(infos)
}

#[tauri::command]
pub async fn delete_workdir(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let registry = registry_or_error(&state)?;
    registry
        .delete(&workdir)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
```

- [ ] **Step 2: Register the module**

Edit `src-tauri/src/commands/mod.rs` line 1-3, add the new module:

```rust
pub mod index_commands;
pub mod query_commands;
pub mod status_commands;
pub mod workdir_commands;
```

- [ ] **Step 3: Register the commands in `lib.rs`**

Edit `src-tauri/src/lib.rs` lines 93-110 — append the two new commands inside `tauri::generate_handler!`:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::index_commands::index_folder,
            commands::index_commands::index_file,
            commands::index_commands::cancel_indexing,
            commands::index_commands::remove_document,
            commands::index_commands::reset_index,
            commands::query_commands::query,
            commands::query_commands::get_chunk_neighbors,
            commands::query_commands::get_graph_overview,
            commands::query_commands::get_graph_neighborhood,
            commands::query_commands::get_search_neighborhood,
            commands::query_commands::get_doc_chunks,
            commands::status_commands::get_index_stats,
            commands::status_commands::list_documents,
            commands::status_commands::engine_ready,
            commands::status_commands::get_settings,
            commands::status_commands::set_transcription_enabled,
            commands::workdir_commands::list_workdirs,
            commands::workdir_commands::delete_workdir,
        ])
```

- [ ] **Step 4: Build + commit**

```bash
cd src-tauri && cargo build
git add src-tauri/src/commands/mod.rs src-tauri/src/commands/workdir_commands.rs src-tauri/src/lib.rs
git commit -m "feat(commands): add list_workdirs + delete_workdir"
```

---

## Task 10: Update MCP tools to take `workdir`

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs` (schemas + dispatch)

- [ ] **Step 1: Add `workdir` to every tool schema**

In `src-tauri/src/mcp/tools.rs::list_tools`, every tool's JSON schema must add `workdir` to `properties` and `required`. Apply this delta to each `json!({...})` block:

For `anubis_search`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir whose index to query." },
        "q": { "type": "string", "description": "Search query." },
        "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
        "depth": { "type": "integer", "minimum": 0, "maximum": 3, "default": 1 }
    },
    "required": ["workdir", "q"]
}),
```

For `anubis_context_pack`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir whose index to query." },
        "q": { "type": "string", "description": "Search query to pack context for." },
        "budget_tokens": { "type": "integer", "minimum": 1, "maximum": 200000, "default": 6000 },
        "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
        "depth": { "type": "integer", "minimum": 0, "maximum": 3, "default": 1 },
        "include_graph": { "type": "boolean", "default": true }
    },
    "required": ["workdir", "q"]
}),
```

For `anubis_index_file`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir to index into." },
        "path": { "type": "string", "description": "Absolute path to a supported file." }
    },
    "required": ["workdir", "path"]
}),
```

For `anubis_index_folder`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir to index into." },
        "path": { "type": "string", "description": "Absolute path to a folder." }
    },
    "required": ["workdir", "path"]
}),
```

For `anubis_get_index_stats`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir." }
    },
    "required": ["workdir"]
}),
```

For `anubis_list_documents`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir." }
    },
    "required": ["workdir"]
}),
```

For `anubis_get_doc_chunks`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir." },
        "doc_id": { "type": "string", "description": "Document id." }
    },
    "required": ["workdir", "doc_id"]
}),
```

For `anubis_get_chunk_neighbors`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir." },
        "chunk_id": { "type": "string", "description": "Chunk id." },
        "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 1 }
    },
    "required": ["workdir", "chunk_id"]
}),
```

For `anubis_get_graph_overview`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir." },
        "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 250 }
    },
    "required": ["workdir"]
}),
```

For `anubis_get_graph_neighborhood`:

```rust
json!({
    "type": "object",
    "properties": {
        "workdir": { "type": "string", "description": "Absolute path to the workdir." },
        "chunk_id": { "type": "string", "description": "Chunk id." },
        "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 2 },
        "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 160 }
    },
    "required": ["workdir", "chunk_id"]
}),
```

- [ ] **Step 2: Change `call_tool` + `dispatch` signatures to take the registry**

Replace the `pub async fn call_tool` and `async fn dispatch` in `src-tauri/src/mcp/tools.rs`:

```rust
pub async fn call_tool(
    registry: &std::sync::Arc<crate::engine::registry::WorkdirRegistry>,
    name: &str,
    arguments: Value,
) -> CallToolResult {
    match dispatch(registry, name, arguments).await {
        Ok(structured) => tool_result(structured),
        Err(message) => error_result(&message),
    }
}

async fn dispatch(
    registry: &std::sync::Arc<crate::engine::registry::WorkdirRegistry>,
    name: &str,
    arguments: Value,
) -> Result<Value, String> {
    let workdir = string_arg(&arguments, "workdir")?;
    let (_, state) = registry
        .get_or_load(&workdir)
        .await
        .map_err(|error| error.to_string())?;
    let state = state.as_ref();

    match name {
        "anubis_search" => {
            let q = string_arg(&arguments, "q")?;
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(10).min(50);
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let query_embedding = {
                let mut embedder = state.embedder.lock().await;
                local::embed_query(&mut embedder, &q).map_err(|e| e.to_string())?
            };
            let db = state.db.lock().await;
            let fts = state.fts.lock().await;
            let results = run_query(&db, &fts, &q, &query_embedding, QueryOpts { limit, depth })
                .map_err(|e| e.to_string())?;
            to_json(results)
        }
        "anubis_context_pack" => {
            let q = string_arg(&arguments, "q")?;
            let budget_tokens = usize_arg(&arguments, "budget_tokens")?
                .unwrap_or(6000)
                .min(200_000);
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(10).min(50);
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let include_graph = bool_arg(&arguments, "include_graph")?.unwrap_or(true);
            let query_embedding = {
                let mut embedder = state.embedder.lock().await;
                local::embed_query(&mut embedder, &q).map_err(|e| e.to_string())?
            };
            let db = state.db.lock().await;
            let fts = state.fts.lock().await;
            let results = run_query(&db, &fts, &q, &query_embedding, QueryOpts { limit, depth })
                .map_err(|e| e.to_string())?;
            let pack = build_context_pack(
                &db,
                &q,
                &results,
                ContextPackOpts {
                    budget_tokens,
                    limit,
                    depth,
                    include_graph,
                },
            )
            .map_err(|e| e.to_string())?;
            to_json(pack)
        }
        "anubis_index_file" => {
            let path = string_arg(&arguments, "path")?;
            indexer::index_file(&path, state)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "indexed": path }))
        }
        "anubis_index_folder" => {
            let path = string_arg(&arguments, "path")?;
            indexer::index_folder(&path, state, None)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "indexed_folder": path }))
        }
        "anubis_get_index_stats" => {
            let db = state.db.lock().await;
            db::get_index_stats(&db).map_err(|e| e.to_string())
        }
        "anubis_list_documents" => {
            let db = state.db.lock().await;
            let docs = db::list_documents(&db).map_err(|e| e.to_string())?;
            Ok(json!({ "documents": docs }))
        }
        "anubis_get_doc_chunks" => {
            let doc_id = string_arg(&arguments, "doc_id")?;
            let db = state.db.lock().await;
            let doc_chunks = chunks::get_doc_chunks(&db, &doc_id).map_err(|e| e.to_string())?;
            Ok(json!({ "chunks": doc_chunks }))
        }
        "anubis_get_chunk_neighbors" => {
            let chunk_id = string_arg(&arguments, "chunk_id")?;
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let db = state.db.lock().await;
            let neighbors = graph_store::chunk_neighbors(&db, &chunk_id, depth * 50)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "neighbors": neighbors }))
        }
        "anubis_get_graph_overview" => {
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(250).min(1000);
            let db = state.db.lock().await;
            let overview = graph_store::graph_overview(&db, limit).map_err(|e| e.to_string())?;
            to_json(overview)
        }
        "anubis_get_graph_neighborhood" => {
            let chunk_id = string_arg(&arguments, "chunk_id")?;
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(2).min(3);
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(160).min(1000);
            let db = state.db.lock().await;
            let overview = graph_store::graph_neighborhood(&db, &chunk_id, depth, limit)
                .map_err(|e| e.to_string())?;
            to_json(overview)
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}
```

Also remove the now-unused `use crate::engine::{indexer, state::AppState};` and replace with:

```rust
use crate::engine::indexer;
```

Update the `lists_all_ten_tools` test at the bottom (lines 297-323) so it still passes — it doesn't check schemas, only names, so no change needed beyond confirming the run.

---

## Task 11: Update MCP main loop to use the registry

**Files:**
- Modify: `src-tauri/src/mcp/mod.rs:56-194`

- [ ] **Step 1: Replace `run_async` + `handle_request`**

In `src-tauri/src/mcp/mod.rs`, replace `run_async` and `handle_request` so they hold a `WorkdirRegistry` instead of an `AppState`:

```rust
async fn run_async() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // ANUBIS_DB_PATH overrides the storage root entirely — its *parent* is
    // treated as the workdirs root so existing power users can keep a flat
    // single-DB layout if they want (one workdir per ANUBIS_DB_PATH invocation).
    let workdirs_root = db_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("workdirs");
    std::fs::create_dir_all(&workdirs_root)?;

    let models_dir = db_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let embedder = crate::engine::state::bootstrap_shared_engines(&models_dir)
        .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
    let registry = std::sync::Arc::new(crate::engine::registry::WorkdirRegistry::new(
        workdirs_root,
        embedder,
    ));

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let req_str = line.trim();
        if req_str.is_empty() {
            line.clear();
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(req_str) {
            Ok(req) => {
                if let Some(res) = handle_request(&registry, req).await {
                    let out = serde_json::to_string(&res)?;
                    writeln!(stdout, "{}", out)?;
                    stdout.flush()?;
                }
            }
            Err(e) => {
                let err_res = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let out = serde_json::to_string(&err_res)?;
                writeln!(stdout, "{}", out)?;
                stdout.flush()?;
            }
        }
        line.clear();
    }

    Ok(())
}

async fn handle_request(
    registry: &std::sync::Arc<crate::engine::registry::WorkdirRegistry>,
    req: JsonRpcRequest,
) -> Option<JsonRpcResponse> {
    let id = req.id.clone().unwrap_or(Value::Null);
    if id.is_null() && req.method != "notifications/initialized" {
        return None;
    }

    match req.method.as_str() {
        "initialize" => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!(InitializeResult {
                protocolVersion: "2025-06-18".to_string(),
                capabilities: json!({
                    "tools": {
                        "listChanged": false
                    }
                }),
                serverInfo: ServerInfo {
                    name: "anubis-engine".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                }
            })),
            error: None,
        }),
        "notifications/initialized" => None,
        "ping" => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        }),
        "tools/list" => {
            let tools = tools::list_tools();
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::to_value(tools).unwrap()),
                error: None,
            })
        }
        "tools/call" => {
            if let Some(params) = req.params {
                if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let result = tools::call_tool(registry, name, args).await;
                    return Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::to_value(result).unwrap()),
                        error: None,
                    });
                }
            }
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Invalid params for tools/call".to_string(),
                    data: None,
                }),
            })
        }
        _ => {
            if !id.is_null() {
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Method not found: {}", req.method),
                        data: None,
                    }),
                })
            } else {
                None
            }
        }
    }
}
```

Also delete the now-unused `use crate::engine::state::AppState;` import at the top.

- [ ] **Step 2: Build + run mcp tool tests**

```bash
cd src-tauri && cargo build && cargo test --lib mcp::
```

Expected: `lists_all_ten_tools` still passes.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/mcp/
git commit -m "feat(mcp): every anubis_* tool now takes a workdir parameter

Schemas require 'workdir' on every index-touching tool. The MCP
server holds a WorkdirRegistry rather than a single AppState; each
tool call resolves its target workdir lazily on demand."
```

---

## Task 12: Add `workdir_id` to indexing progress events

**Files:**
- Modify: `src-tauri/src/types.rs:167-180`
- Modify: `src-tauri/src/engine/indexer.rs`

- [ ] **Step 1: Add the field to `IndexProgress`**

In `src-tauri/src/types.rs` lines 167-180, replace `IndexProgress` with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub status: IndexStatus,
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<IndexStage>,
    /// Workdir id this progress event belongs to. The frontend filters
    /// events by `active_workdir_id` so multi-workdir indexing doesn't leak
    /// into unrelated panels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir_id: Option<String>,
}
```

- [ ] **Step 2: Plumb `workdir_id` into the indexer**

The cleanest way: derive the workdir id once at the top of `index_folder` and `index_file` via `crate::engine::workdir::resolve` on the *AppState's path* — but `AppState` doesn't know its own path. Simpler: have the registry-aware `workdir_state` helper return the `WorkdirId` too, and pass it into the indexer.

Update `src-tauri/src/commands/mod.rs::workdir_state` to return both:

```rust
pub async fn workdir_state(
    state: &State<'_, EngineHandle>,
    workdir: &str,
) -> Result<(crate::engine::workdir::WorkdirId, Arc<AppState>), String> {
    let registry = registry_or_error(state)?;
    registry
        .get_or_load(workdir)
        .await
        .map_err(|error| error.to_string())
}
```

Update every caller in `index_commands.rs`, `query_commands.rs`, `status_commands.rs`, `workdir_commands.rs` to destructure the tuple — most can rename `let engine = workdir_state(...)` to `let (workdir_id, engine) = workdir_state(...)` and ignore `workdir_id` with `_workdir_id` when unused.

For `index_commands::index_folder` and `index_file`, update the indexer call signature:

```rust
crate::engine::indexer::index_folder(&path, &engine, Some(app), Some(workdir_id))
crate::engine::indexer::index_file(&path, &engine, Some(workdir_id))
```

Update `src-tauri/src/engine/indexer.rs` `index_folder` and `index_file` signatures + `emit_progress` calls to thread the id through. Concretely, add a parameter:

```rust
pub async fn index_folder(
    path: &str,
    state: &AppState,
    app: Option<AppHandle>,
    workdir_id: Option<crate::engine::workdir::WorkdirId>,
) -> Result<(), EngineError> { ... }

pub async fn index_file(
    path: &str,
    state: &AppState,
    workdir_id: Option<crate::engine::workdir::WorkdirId>,
) -> Result<(), EngineError> { ... }
```

Update `emit_progress` to accept `&Option<WorkdirId>` and populate `IndexProgress::workdir_id`:

```rust
fn emit_progress(
    app: &Option<AppHandle>,
    workdir_id: &Option<crate::engine::workdir::WorkdirId>,
    total: usize,
    done: usize,
    current: &str,
    status: IndexStatus,
    errors: Vec<String>,
    stage: Option<IndexStage>,
) {
    if let Some(app) = app {
        if let Err(error) = app.emit(
            "index-progress",
            IndexProgress {
                total,
                done,
                current: current.to_string(),
                status,
                errors,
                stage,
                workdir_id: workdir_id.as_ref().map(|id| id.as_str().to_string()),
            },
        ) {
            tracing::warn!("failed to emit index progress: {}", error);
        }
    }
}
```

Thread `workdir_id` through `index_folder_inner`, `run_index_paths`, `index_one`, `check_cancelled`, and every internal `emit_progress` call. There are roughly 8 emit sites in this file — update each to pass `&workdir_id`.

Also do the same for the *preprocess* stage in `src-tauri/src/engine/preprocess.rs` if it emits its own progress events (read the file first; if it uses `PreprocessProgress`, add a `workdir_id` field there too and thread it similarly). If `preprocess.rs` doesn't emit events, skip.

- [ ] **Step 3: Build + commit**

```bash
cd src-tauri && cargo build
git add src-tauri/src/types.rs src-tauri/src/engine/indexer.rs src-tauri/src/engine/preprocess.rs src-tauri/src/commands/
git commit -m "feat(events): include workdir_id on index-progress events"
```

---

## Task 13: Integration test — workdir isolation

**Files:**
- Create: `src-tauri/tests/workdir_isolation.rs`

- [ ] **Step 1: Write the test**

Create `src-tauri/tests/workdir_isolation.rs`:

```rust
//! Two workdirs must not see each other's documents or search hits.

use std::sync::Arc;

use anubis_engine::engine::registry::WorkdirRegistry;
use anubis_engine::engine::state;
use anubis_engine::engine::indexer;

#[tokio::test]
async fn two_workdirs_have_disjoint_corpora() {
    let root = tempfile::tempdir().expect("root tempdir");
    let models_dir = root.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();
    let embedder = state::bootstrap_shared_engines(&models_dir).expect("bootstrap");
    let registry = Arc::new(WorkdirRegistry::new(root.path().join("workdirs"), embedder));

    let wd_a = tempfile::tempdir().expect("workdir A");
    let wd_b = tempfile::tempdir().expect("workdir B");
    let file_a = wd_a.path().join("alpha.txt");
    let file_b = wd_b.path().join("bravo.txt");
    std::fs::write(&file_a, "alpha apple aurora").unwrap();
    std::fs::write(&file_b, "bravo banana butter").unwrap();

    let (_, state_a) = registry
        .get_or_load(wd_a.path().to_str().unwrap())
        .await
        .expect("load A");
    let (_, state_b) = registry
        .get_or_load(wd_b.path().to_str().unwrap())
        .await
        .expect("load B");

    indexer::index_file(file_a.to_str().unwrap(), state_a.as_ref(), None)
        .await
        .expect("index A");
    indexer::index_file(file_b.to_str().unwrap(), state_b.as_ref(), None)
        .await
        .expect("index B");

    // A's document list contains alpha.txt only.
    let docs_a = {
        let db = state_a.db.lock().await;
        anubis_engine::store::db::list_documents(&db).expect("list A")
    };
    let docs_b = {
        let db = state_b.db.lock().await;
        anubis_engine::store::db::list_documents(&db).expect("list B")
    };
    let names_a: Vec<String> = docs_a
        .iter()
        .filter_map(|v| v.get("filename").and_then(|f| f.as_str()).map(String::from))
        .collect();
    let names_b: Vec<String> = docs_b
        .iter()
        .filter_map(|v| v.get("filename").and_then(|f| f.as_str()).map(String::from))
        .collect();
    assert_eq!(names_a, vec!["alpha.txt".to_string()]);
    assert_eq!(names_b, vec!["bravo.txt".to_string()]);
}
```

- [ ] **Step 2: Add `pub use` exports so tests can reach internals**

The test crate is external — it can only see items declared `pub` in `lib.rs`. Verify that `anubis_engine::engine::registry::WorkdirRegistry`, `anubis_engine::engine::state::bootstrap_shared_engines`, `anubis_engine::engine::indexer::index_file`, and `anubis_engine::store::db::list_documents` are all reachable through `pub mod` chains. They should be, since `lib.rs` declares `pub mod engine; pub mod store;`. If anything is gated behind `pub(crate)`, raise it to `pub`.

- [ ] **Step 3: Run the test**

```bash
cd src-tauri && cargo test --test workdir_isolation
```

Expected: passes after the (one-time, slow) embedder download.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/workdir_isolation.rs
git commit -m "test(workdir): two workdirs have disjoint corpora"
```

---

## Task 14: Integration test — lazy load

**Files:**
- Create: `src-tauri/tests/workdir_lazy_load.rs`

- [ ] **Step 1: Expose `loaded_states` to tests**

Confirm `WorkdirRegistry::loaded_states` is `pub` (added in Task 7 Step 2). If it's `pub(crate)`, change to `pub`.

- [ ] **Step 2: Write the test**

Create `src-tauri/tests/workdir_lazy_load.rs`:

```rust
use std::sync::Arc;

use anubis_engine::engine::registry::WorkdirRegistry;
use anubis_engine::engine::state;

#[tokio::test]
async fn registry_starts_empty_and_loads_on_demand() {
    let root = tempfile::tempdir().expect("root");
    let models_dir = root.path().join("models");
    std::fs::create_dir_all(&models_dir).unwrap();
    let embedder = state::bootstrap_shared_engines(&models_dir).expect("bootstrap");
    let registry = Arc::new(WorkdirRegistry::new(root.path().join("workdirs"), embedder));

    {
        let states = registry.loaded_states().await;
        assert_eq!(states.len(), 0, "expected empty cache at start");
    }

    let w1 = tempfile::tempdir().unwrap();
    let w2 = tempfile::tempdir().unwrap();
    registry.get_or_load(w1.path().to_str().unwrap()).await.unwrap();
    registry.get_or_load(w2.path().to_str().unwrap()).await.unwrap();

    let states = registry.loaded_states().await;
    assert_eq!(states.len(), 2, "expected two cached states");
}
```

- [ ] **Step 3: Run + commit**

```bash
cd src-tauri && cargo test --test workdir_lazy_load
git add src-tauri/tests/workdir_lazy_load.rs
git commit -m "test(workdir): registry loads lazily on first use"
```

---

## Task 15: Integration test — errors

**Files:**
- Create: `src-tauri/tests/workdir_errors.rs`

- [ ] **Step 1: Write the test**

Create `src-tauri/tests/workdir_errors.rs`:

```rust
use std::sync::Arc;

use anubis_engine::engine::registry::WorkdirRegistry;
use anubis_engine::engine::state;
use anubis_engine::EngineError;

#[tokio::test]
async fn nonexistent_path_returns_not_found() {
    let root = tempfile::tempdir().unwrap();
    let embedder = state::bootstrap_shared_engines(root.path()).unwrap();
    let registry = Arc::new(WorkdirRegistry::new(root.path().join("workdirs"), embedder));

    let result = registry.get_or_load("Z:/definitely/does/not/exist").await;
    let err = result.expect_err("must fail");
    assert!(
        matches!(err, EngineError::Workdir(anubis_engine::engine::workdir::WorkdirError::NotFound { .. })),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn file_instead_of_dir_returns_not_found() {
    let root = tempfile::tempdir().unwrap();
    let embedder = state::bootstrap_shared_engines(root.path()).unwrap();
    let registry = Arc::new(WorkdirRegistry::new(root.path().join("workdirs"), embedder));

    let file = root.path().join("not-a-dir.txt");
    std::fs::write(&file, "x").unwrap();
    let result = registry.get_or_load(file.to_str().unwrap()).await;
    let err = result.expect_err("must fail");
    assert!(
        matches!(err, EngineError::Workdir(anubis_engine::engine::workdir::WorkdirError::NotFound { .. })),
        "expected NotFound, got {err:?}"
    );
}

#[tokio::test]
async fn same_canonical_path_returns_same_state() {
    let root = tempfile::tempdir().unwrap();
    let embedder = state::bootstrap_shared_engines(root.path()).unwrap();
    let registry = Arc::new(WorkdirRegistry::new(root.path().join("workdirs"), embedder));

    let wd = tempfile::tempdir().unwrap();
    let path_a = wd.path().to_str().unwrap().to_string();
    // Build a different string spelling that canonicalises to the same path
    // (trailing slash); on Windows this triggers the canonicalize roundtrip.
    let path_b = format!("{}{}", path_a, std::path::MAIN_SEPARATOR);

    let (id_a, state_a) = registry.get_or_load(&path_a).await.unwrap();
    let (id_b, state_b) = registry.get_or_load(&path_b).await.unwrap();
    assert_eq!(id_a, id_b);
    assert!(Arc::ptr_eq(&state_a, &state_b));
}
```

- [ ] **Step 2: Run + commit**

```bash
cd src-tauri && cargo test --test workdir_errors
git add src-tauri/tests/workdir_errors.rs
git commit -m "test(workdir): error variants + canonicalize idempotence"
```

---

## Task 16: Frontend — `WorkdirContext` + hook

**Files:**
- Create: `src/contexts/WorkdirContext.tsx`

- [ ] **Step 1: Write the context**

Create `src/contexts/WorkdirContext.tsx`:

```tsx
import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

export type WorkdirInfo = {
  id: string;
  path: string;
  created_at: string;
  last_used: string;
  doc_count: number | null;
};

type WorkdirContextValue = {
  activeWorkdir: string | null;
  activeWorkdirId: string | null;
  knownWorkdirs: WorkdirInfo[];
  setActiveWorkdir: (path: string | null) => void;
  refreshKnownWorkdirs: () => Promise<void>;
  deleteWorkdir: (path: string) => Promise<void>;
};

const STORAGE_KEY = "anubis.activeWorkdir";

const WorkdirContext = createContext<WorkdirContextValue | null>(null);

export function WorkdirProvider({ children }: { children: ReactNode }) {
  const [activeWorkdir, setActiveWorkdirState] = useState<string | null>(() => {
    if (typeof window === "undefined") return null;
    return window.localStorage.getItem(STORAGE_KEY);
  });
  const [knownWorkdirs, setKnownWorkdirs] = useState<WorkdirInfo[]>([]);

  const refreshKnownWorkdirs = useCallback(async () => {
    try {
      const list = await invoke<WorkdirInfo[]>("list_workdirs");
      setKnownWorkdirs(list);
      // Clear stale active selection if the path no longer exists on disk.
      if (activeWorkdir && !list.some((w) => w.path === activeWorkdir)) {
        // Don't auto-clear during the very first poll — the user may have
        // just picked a path that isn't indexed yet (registered lazily on
        // first index call). Only clear if list_workdirs has at least one
        // entry AND none of them match.
        if (list.length > 0) {
          setActiveWorkdirState(null);
          window.localStorage.removeItem(STORAGE_KEY);
        }
      }
    } catch (reason) {
      const message = String(reason);
      if (!message.toLowerCase().includes("still initialising")) {
        console.warn("list_workdirs failed:", message);
      }
    }
  }, [activeWorkdir]);

  const setActiveWorkdir = useCallback((path: string | null) => {
    setActiveWorkdirState(path);
    if (typeof window !== "undefined") {
      if (path) {
        window.localStorage.setItem(STORAGE_KEY, path);
      } else {
        window.localStorage.removeItem(STORAGE_KEY);
      }
    }
  }, []);

  const deleteWorkdir = useCallback(
    async (path: string) => {
      await invoke<void>("delete_workdir", { workdir: path });
      if (activeWorkdir === path) {
        setActiveWorkdir(null);
      }
      await refreshKnownWorkdirs();
    },
    [activeWorkdir, refreshKnownWorkdirs, setActiveWorkdir],
  );

  useEffect(() => {
    void refreshKnownWorkdirs();
  }, [refreshKnownWorkdirs]);

  const activeWorkdirId = useMemo(() => {
    if (!activeWorkdir) return null;
    const match = knownWorkdirs.find((w) => w.path === activeWorkdir);
    return match?.id ?? null;
  }, [activeWorkdir, knownWorkdirs]);

  const value = useMemo<WorkdirContextValue>(
    () => ({
      activeWorkdir,
      activeWorkdirId,
      knownWorkdirs,
      setActiveWorkdir,
      refreshKnownWorkdirs,
      deleteWorkdir,
    }),
    [
      activeWorkdir,
      activeWorkdirId,
      knownWorkdirs,
      setActiveWorkdir,
      refreshKnownWorkdirs,
      deleteWorkdir,
    ],
  );

  return <WorkdirContext.Provider value={value}>{children}</WorkdirContext.Provider>;
}

export function useWorkdir(): WorkdirContextValue {
  const ctx = useContext(WorkdirContext);
  if (!ctx) {
    throw new Error("useWorkdir must be used inside <WorkdirProvider>");
  }
  return ctx;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/contexts/WorkdirContext.tsx
git commit -m "feat(ui): WorkdirContext + useWorkdir hook"
```

---

## Task 17: Frontend — `WorkdirSwitcher` component

**Files:**
- Create: `src/components/WorkdirSwitcher.tsx`

- [ ] **Step 1: Write the component**

Create `src/components/WorkdirSwitcher.tsx`:

```tsx
import { open } from "@tauri-apps/plugin-dialog";
import { Check, ChevronDown, FolderPlus, Trash2 } from "lucide-react";
import { useState } from "react";

import { useWorkdir, WorkdirInfo } from "../contexts/WorkdirContext";
import { Button } from "./ui/button";

function basename(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export default function WorkdirSwitcher() {
  const { activeWorkdir, knownWorkdirs, setActiveWorkdir, deleteWorkdir, refreshKnownWorkdirs } =
    useWorkdir();
  const [open_, setOpen] = useState(false);

  async function pickFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Pick a workdir to index into",
    });
    if (typeof selected === "string") {
      setActiveWorkdir(selected);
      await refreshKnownWorkdirs();
    }
    setOpen(false);
  }

  async function handleDelete(entry: WorkdirInfo) {
    const confirmed = window.confirm(
      `Delete the Anubis index for ${entry.path}?\n\nThe folder itself stays on disk; only the index data is removed.`,
    );
    if (!confirmed) return;
    await deleteWorkdir(entry.path);
  }

  const label = activeWorkdir
    ? basename(activeWorkdir)
    : "No workdir";

  return (
    <div className="relative">
      <Button
        variant="outline"
        size="sm"
        className="gap-2"
        onClick={() => setOpen((value) => !value)}
      >
        <span className="max-w-[220px] truncate text-sm">{label}</span>
        <ChevronDown className="size-3.5" />
      </Button>

      {open_ && (
        <div
          className="absolute right-0 z-50 mt-1 w-[360px] rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-1 shadow-lg"
          onMouseLeave={() => setOpen(false)}
        >
          {knownWorkdirs.length === 0 ? (
            <div className="px-3 py-4 text-center text-xs text-[var(--color-muted-foreground)]">
              No workdirs yet — pick a folder to get started.
            </div>
          ) : (
            <div className="max-h-[280px] overflow-y-auto py-1">
              {knownWorkdirs.map((entry) => {
                const isActive = entry.path === activeWorkdir;
                return (
                  <div
                    key={entry.id}
                    className="group flex items-center gap-2 rounded-md px-2 py-2 hover:bg-[var(--color-accent)]"
                  >
                    <button
                      type="button"
                      className="flex flex-1 items-start gap-2 text-left"
                      onClick={() => {
                        setActiveWorkdir(entry.path);
                        setOpen(false);
                      }}
                    >
                      <Check
                        className={`mt-0.5 size-3.5 shrink-0 ${
                          isActive ? "text-[var(--color-primary)]" : "text-transparent"
                        }`}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-sm font-medium">{basename(entry.path)}</div>
                        <div className="truncate text-[11px] text-[var(--color-muted-foreground)]">
                          {entry.path}
                        </div>
                        <div className="text-[10px] text-[var(--color-muted-foreground)]">
                          {entry.doc_count != null ? `${entry.doc_count} docs · ` : ""}
                          last used {new Date(entry.last_used).toLocaleString()}
                        </div>
                      </div>
                    </button>
                    <button
                      type="button"
                      className="rounded-md p-1 text-[var(--color-muted-foreground)] opacity-0 transition group-hover:opacity-100 hover:bg-[var(--color-destructive)]/10 hover:text-[var(--color-destructive)]"
                      onClick={(e) => {
                        e.stopPropagation();
                        void handleDelete(entry);
                      }}
                      title="Delete index"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          <div className="border-t border-[var(--color-border)] p-1">
            <button
              type="button"
              className="flex w-full items-center gap-2 rounded-md px-2 py-2 text-sm hover:bg-[var(--color-accent)]"
              onClick={() => void pickFolder()}
            >
              <FolderPlus className="size-3.5" />
              Add workdir…
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/WorkdirSwitcher.tsx
git commit -m "feat(ui): WorkdirSwitcher dropdown component"
```

---

## Task 18: Wire context + switcher into `App.tsx`; add empty state

**Files:**
- Modify: `src/main.tsx` (wrap in provider)
- Modify: `src/App.tsx` (consume hook, pass workdir to invokes, empty state, header switcher)

- [ ] **Step 1: Wrap the root with `WorkdirProvider`**

Read `src/main.tsx` first to see the current root. Then update it to:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { WorkdirProvider } from "./contexts/WorkdirContext";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <WorkdirProvider>
      <App />
    </WorkdirProvider>
  </React.StrictMode>,
);
```

(Match the existing file's import style — keep whatever it already had for `./index.css` / strict mode.)

- [ ] **Step 2: Update `App.tsx` to consume the hook + add the switcher**

In `src/App.tsx`:

1. Import the hook + switcher near the top:

```tsx
import { useWorkdir } from "./contexts/WorkdirContext";
import WorkdirSwitcher from "./components/WorkdirSwitcher";
```

2. Inside `App()`, add `const { activeWorkdir, refreshKnownWorkdirs } = useWorkdir();` near the top of the function body (right after `useState` declarations).

3. Update every `invoke()` call inside `App.tsx` to include `workdir: activeWorkdir`. There are calls for `get_graph_overview`, `get_search_neighborhood`, `get_graph_neighborhood`, `get_doc_chunks`, `index_file`. Each becomes:

```tsx
const payload = await invoke<GraphOverviewPayload>("get_graph_overview", {
  workdir: activeWorkdir,
  limit: 250,
});
```

```tsx
const neighborhood = await invoke<GraphOverviewPayload>("get_search_neighborhood", {
  workdir: activeWorkdir,
  chunkIds: results.map((r) => r.chunk_id),
  depth,
  limit: 200,
});
```

```tsx
const neighborhood = await invoke<GraphOverviewPayload>("get_graph_neighborhood", {
  workdir: activeWorkdir,
  chunkId: result.chunk_id,
  depth,
  limit: 160,
});
```

```tsx
const chunks = await invoke<Chunk[]>("get_doc_chunks", {
  workdir: activeWorkdir,
  docId: document.id,
});
```

```tsx
await invoke("index_file", { workdir: activeWorkdir, path: document.path });
```

4. Guard each of those callbacks with an early return when `activeWorkdir` is null:

```tsx
const loadGlobalGraph = useCallback(async () => {
  if (!activeWorkdir) {
    setGraphData({ nodes: [], links: [] });
    setHasIndex(false);
    return;
  }
  setGraphLoading(true);
  // ... rest unchanged but pass workdir: activeWorkdir
}, [activeWorkdir]);
```

Apply the same `if (!activeWorkdir) return;` guard to `showSearchConstellation`, `focusOnChunk`, `handleSelectDocument`, `handleReindexDocument`. Add `activeWorkdir` to each `useCallback` dependency list.

5. Add a `useEffect` that re-loads the global graph when `activeWorkdir` changes:

```tsx
useEffect(() => {
  loadGlobalGraph();
}, [loadGlobalGraph, refreshKey, activeWorkdir]);
```

(Replace the existing `useEffect` that depends on `[loadGlobalGraph, refreshKey]`.)

6. Add the switcher to the header area. In the existing aside (line 218-231 of the original `App.tsx`), find the brand block and append `<WorkdirSwitcher />` after `<Separator />` (between brand and `IndexStatus`):

```tsx
        <Separator />

        <WorkdirSwitcher />

        <Separator />

        <IndexStatus onIndexed={handleIndexed} onCleared={handleCleared} />
```

7. Add an empty-state guard for the main panel: when `activeWorkdir` is null, render a "Pick a workdir" placeholder instead of `<GraphVisualizer />` / `<EmptyState />`. Add a new component at the bottom of the file:

```tsx
function NoWorkdirState() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 rounded-xl border border-dashed border-[var(--color-border)] p-12 text-center">
      <div className="space-y-1">
        <h2 className="text-lg font-semibold">Pick or add a workdir</h2>
        <p className="max-w-md text-sm text-[var(--color-muted-foreground)]">
          Use the dropdown in the sidebar to pick an existing workdir or add a
          new one. Anubis keeps a separate index per workdir.
        </p>
      </div>
    </div>
  );
}
```

And in the main `<main>` block:

```tsx
{!activeWorkdir ? (
  <NoWorkdirState />
) : hasIndex || graphData.nodes.length > 0 || graphLoading ? (
  <GraphVisualizer ... />
) : (
  <EmptyState />
)}
```

- [ ] **Step 3: Pass `activeWorkdir` into child components**

The child components (`SearchBar`, `IndexStatus`, `KnowledgeBrowser`) also call `invoke`. There are two options:

- (a) Have each component import `useWorkdir()` itself.
- (b) Pass `activeWorkdir` as a prop from `App.tsx`.

Use option (a) — it keeps the prop surface stable. For each child component:

For `src/components/IndexStatus.tsx`:
- Add `import { useWorkdir } from "../contexts/WorkdirContext";`
- Add `const { activeWorkdir } = useWorkdir();` at the top of the component.
- Every `invoke("index_folder", { path })` → `invoke("index_folder", { workdir: activeWorkdir, path })`.
- Every `invoke("cancel_indexing")` → `invoke("cancel_indexing", { workdir: activeWorkdir })`.
- Every `invoke("reset_index")` → `invoke("reset_index", { workdir: activeWorkdir })`.
- Every `invoke("get_index_stats")` → `invoke("get_index_stats", { workdir: activeWorkdir })`.
- Disable the action buttons when `!activeWorkdir`.

For `src/components/SearchBar.tsx`:
- Add `useWorkdir()`.
- `invoke("query", { q, limit, depth })` → `invoke("query", { workdir: activeWorkdir, q, limit, depth })`.
- Disable the search input when `!activeWorkdir`.

For `src/components/KnowledgeBrowser.tsx`:
- Add `useWorkdir()`.
- `invoke("list_documents")` → `invoke("list_documents", { workdir: activeWorkdir })`.
- `invoke("remove_document", { docId })` → `invoke("remove_document", { workdir: activeWorkdir, docId })`.
- Render an empty list when `!activeWorkdir`.

Also make each component re-fetch when `activeWorkdir` changes by adding it to the relevant `useEffect` dependency arrays.

- [ ] **Step 4: Filter index-progress events by workdir**

In `IndexStatus.tsx` (and anywhere else that calls `listen("index-progress", ...)`), filter:

```tsx
const { activeWorkdirId } = useWorkdir();

useEffect(() => {
  const unlisten = listen<IndexProgress>("index-progress", (event) => {
    if (
      event.payload.workdir_id &&
      activeWorkdirId &&
      event.payload.workdir_id !== activeWorkdirId
    ) {
      return; // event belongs to a different workdir
    }
    setProgress(event.payload);
  });
  return () => {
    unlisten.then((fn) => fn());
  };
}, [activeWorkdirId]);
```

Make sure to extend the local `IndexProgress` type in the component file to include `workdir_id?: string | null;`.

- [ ] **Step 5: Verify the dev build**

```bash
npm run dev
```

Open the app. The expected behaviour:
- On first launch with no `localStorage` key, the main panel shows the "Pick or add a workdir" placeholder.
- The `WorkdirSwitcher` shows "No workdir" and reveals an empty list + "Add workdir…" button.
- Picking a folder sets it as active; clicking "Index folder" indexes into that workdir's `<id>` directory under `%APPDATA%/anubis/workdirs/`.
- Switching to a second workdir clears the visible graph and loads the new one's data on demand.
- The MCP sidecar still works — verify by running an `anubis_search` with `workdir` arg.

If anything is broken, fix it before continuing. Don't claim done until you've actually clicked through both workdirs.

- [ ] **Step 6: Commit**

```bash
git add src/main.tsx src/App.tsx src/components/IndexStatus.tsx src/components/SearchBar.tsx src/components/KnowledgeBrowser.tsx
git commit -m "feat(ui): pass activeWorkdir to every invoke; filter events by id

Every Tauri command call now includes workdir from useWorkdir().
The main panel shows a 'pick a workdir' placeholder when none is
active. index-progress events that belong to a different workdir
are dropped so multi-workdir indexing doesn't leak into unrelated
panels."
```

---

## Task 19: Full smoke test + final commit

- [ ] **Step 1: Clean build**

```bash
cd src-tauri && cargo clean && cargo build && cd ..
```

- [ ] **Step 2: Run the full Rust test suite**

```bash
cd src-tauri && cargo test
```

Expected: all library and integration tests pass.

- [ ] **Step 3: Build the Tauri app**

```bash
npm run tauri build
```

Or for a faster check, `npm run tauri dev` and walk through:

1. App starts; engine bootstrap completes (model download bar disappears).
2. Switcher shows "No workdir"; main panel shows the placeholder.
3. Pick a folder → switcher updates; placeholder still showing (no docs yet).
4. Click "Index folder" → progress events flow; document list populates; graph renders.
5. Pick a second folder → main panel shows placeholder again until indexing.
6. Index a different folder into it → verify it does *not* appear in the first workdir's list (switch back to confirm).
7. Hover the first workdir's entry in the switcher → click trash → confirm dialog → verify the storage directory is gone from `%APPDATA%/anubis/workdirs/`.

- [ ] **Step 4: Verify MCP**

Run the MCP server manually (`cargo run --bin mcp-server` or whichever binary your project exposes — read `src-tauri/Cargo.toml` for `[[bin]]` entries). Send an `initialize` then a `tools/call` for `anubis_search` with a `workdir` arg. Confirm:

- Tool list response includes `workdir` in every `inputSchema.required`.
- A call without `workdir` returns an `isError: true` result with `"Missing or invalid string argument: workdir"`.
- A call with a valid `workdir` succeeds.

- [ ] **Step 5: Final commit**

If any tweaks were needed during smoke testing, commit them now:

```bash
git add -A
git commit -m "chore: per-workdir indexes smoke-test fixes" || echo "nothing to commit"
```

---

## Verification checklist

Before declaring done, the following must be true:

- [ ] `cargo build` clean.
- [ ] `cargo test` green (library + 3 integration tests).
- [ ] Adding two workdirs in the UI yields two distinct `<id>` directories under `%APPDATA%/anubis/workdirs/`, each with its own `anubis.db` + `fts_index/` + `meta.json`.
- [ ] Indexing into workdir A does not show up in workdir B's document list.
- [ ] Search in A does not return hits from B.
- [ ] Deleting workdir B via the switcher removes its `<id>` directory and clears it from `list_workdirs`.
- [ ] MCP `anubis_search` (and every other index tool) requires `workdir`; calls without it return a structured error.
- [ ] Frontend persists last-active workdir across reloads (kill + restart the app, the switcher remembers the selection).
- [ ] Engine bootstrap (`engine_ready` returning true) does not require a workdir to be set.
