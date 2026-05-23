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
                let existing = existing.clone();
                drop(states);
                self.write_meta(&id, &canonical)?;
                return Ok((id, existing));
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

        let returned = {
            let mut states = self.states.lock().await;
            states.entry(id.clone()).or_insert_with(|| state.clone()).clone()
        };

        self.write_meta(&id, &canonical)?;
        Ok((id, returned))
    }

    /// Drop the cached state, remove storage on disk. No-op for an id that
    /// was never loaded.
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

    /// Snapshot the cached states — used by commands that need to act on
    /// any loaded workdir (e.g. persisting a global setting). Holds the
    /// inner mutex; callers should release the guard promptly.
    pub async fn loaded_states(
        &self,
    ) -> tokio::sync::MutexGuard<'_, HashMap<WorkdirId, Arc<AppState>>> {
        self.states.lock().await
    }

    fn write_meta(&self, id: &WorkdirId, canonical: &Path) -> Result<(), EngineError> {
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

    use std::sync::OnceLock;
    use tokio::sync::Mutex as TokioMutex;

    // Shared embedder across all tests in this process so we only pay the
    // ~118MB model download once. Cached on disk at target/test-models/
    // via the ANUBIS_EMBED_MODELS_DIR env var so reruns are instant.
    static SHARED_EMBEDDER: OnceLock<Arc<TokioMutex<fastembed::TextEmbedding>>> = OnceLock::new();

    fn shared_embedder() -> Arc<TokioMutex<fastembed::TextEmbedding>> {
        if let Some(handle) = SHARED_EMBEDDER.get() {
            return handle.clone();
        }
        let cache_root = std::env::current_dir()
            .expect("cwd")
            .join("target")
            .join("test-models");
        std::fs::create_dir_all(&cache_root).expect("test models dir");
        std::env::set_var("ANUBIS_EMBED_MODELS_DIR", &cache_root);
        let handle = crate::engine::state::bootstrap_shared_engines(&cache_root)
            .expect("bootstrap embedder");
        let _ = SHARED_EMBEDDER.set(handle.clone());
        handle
    }

    async fn fresh_registry() -> (tempfile::TempDir, WorkdirRegistry) {
        let root = tempfile::tempdir().expect("tempdir for root");
        let embedder = shared_embedder();
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
        let paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
        let canon_a = a.path().canonicalize().unwrap().to_string_lossy().into_owned();
        let canon_b = b.path().canonicalize().unwrap().to_string_lossy().into_owned();
        assert!(paths.contains(&canon_a), "expected {canon_a} in {paths:?}");
        assert!(paths.contains(&canon_b), "expected {canon_b} in {paths:?}");
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
        let states = registry.loaded_states().await;
        assert!(!states.contains_key(&id));
    }
}
