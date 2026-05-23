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

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    pub fts: Arc<Mutex<tantivy::Index>>,
    pub graph: Arc<Mutex<petgraph::Graph<String, f32>>>,
    pub indexing: Arc<Mutex<bool>>,
    /// Cooperative cancellation signal. The indexer (preprocess + index
    /// passes) checks this between files and between sub-stages. Set by
    /// the `cancel_indexing` Tauri command. Reset to `false` at the start
    /// of each `index_folder` call so a previous cancel doesn't poison
    /// the next run.
    pub cancel_token: Arc<AtomicBool>,
    /// Workdir this AppState belongs to. Set by `WorkdirRegistry` after
    /// construction; `None` when an `AppState` is built directly (legacy
    /// path / tests). Indexer + preprocess attach it to progress events
    /// so the frontend can filter by active workdir.
    pub workdir_id: Option<crate::engine::workdir::WorkdirId>,
}

impl AppState {
    /// Build an `AppState` for one workdir's storage. The embedder is shared
    /// across workdirs (one model loaded into memory once) and is supplied
    /// by the caller. OCR + transcription models are configured globally on
    /// startup before any `AppState` is built — see [`bootstrap_shared_engines`].
    pub fn new(
        db_path: &std::path::Path,
        fts_path: &std::path::Path,
        embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    ) -> Result<Self, EngineError> {
        let db = store::db::open(db_path)?;
        // Hydrate persisted runtime settings (transcription toggle, etc.).
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
            workdir_id: None,
        })
    }

    /// Convenience accessor that returns the workdir id as a `String` for
    /// inclusion in event payloads.
    pub fn workdir_id_str(&self) -> Option<String> {
        self.workdir_id.as_ref().map(|id| id.as_str().to_string())
    }
}

/// Configure global model directories and load the embedder. Runs once on
/// startup before any [`AppState`] is constructed. The returned handle is
/// shared across all per-workdir `AppState`s built by `WorkdirRegistry`.
pub fn bootstrap_shared_engines(
    models_dir: &std::path::Path,
) -> Result<Arc<Mutex<fastembed::TextEmbedding>>, EngineError> {
    crate::ocr::engine::set_models_dir(models_dir.to_path_buf());
    crate::embedder::download::set_models_dir(models_dir.to_path_buf());
    crate::transcription::engine::set_models_dir(models_dir.to_path_buf());

    // MultilingualE5Small: 384 dim, trained on 100+ languages with strong
    // recall on Indonesian content. `load_or_download` emits its own
    // start/downloading/ready/error events.
    let embedder = crate::embedder::download::load_or_download()?;
    Ok(Arc::new(Mutex::new(embedder)))
}

fn invalidate_on_chunk_signal_change(
    conn: &rusqlite::Connection,
    current_model: &str,
) -> Result<(), EngineError> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM index_stats WHERE key = 'chunk_signal_model'",
            [],
            |row| row.get(0),
        )
        .ok();

    if stored.as_deref() == Some(current_model) {
        return Ok(());
    }

    let chunk_count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
    if chunk_count > 0 {
        tracing::warn!(
            "chunk signal model changed ({:?} -> {}); clearing stale chunks and graph data",
            stored,
            current_model
        );
        conn.execute_batch(
            r#"
            DELETE FROM entity_terms;
            DELETE FROM entities;
            DELETE FROM graph_edges;
            DELETE FROM vectors;
            DELETE FROM chunks;
            UPDATE documents SET status = 'pending', error_msg = NULL, hash = '';
            "#,
        )?;
    }

    conn.execute(
        "INSERT INTO index_stats (key, value) VALUES ('chunk_signal_model', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [current_model],
    )?;
    Ok(())
}

fn invalidate_on_model_change(
    conn: &rusqlite::Connection,
    current_model: &str,
) -> Result<(), EngineError> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM index_stats WHERE key = 'embedding_model'",
            [],
            |row| row.get(0),
        )
        .ok();

    match stored.as_deref() {
        Some(value) if value == current_model => return Ok(()),
        Some(other) => {
            tracing::warn!(
                "embedding model changed ({} -> {}); clearing stale vectors and re-marking documents as pending",
                other,
                current_model
            );
            conn.execute_batch(
                r#"
                DELETE FROM entity_terms;
                DELETE FROM entities;
                DELETE FROM graph_edges;
                DELETE FROM vectors;
                DELETE FROM chunks;
                UPDATE documents SET status = 'pending', error_msg = NULL, hash = '';
                "#,
            )?;
        }
        None => {
            // First boot under the new tracking scheme — if the DB already
            // contains chunks/vectors from before model tracking existed, they
            // must be invalidated too.
            let chunk_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
            if chunk_count > 0 {
                tracing::warn!(
                    "no embedding_model recorded but {} chunks exist; clearing as a precaution",
                    chunk_count
                );
                conn.execute_batch(
                    r#"
                    DELETE FROM entity_terms;
                    DELETE FROM entities;
                    DELETE FROM graph_edges;
                    DELETE FROM vectors;
                    DELETE FROM chunks;
                    UPDATE documents SET status = 'pending', error_msg = NULL, hash = '';
                    "#,
                )?;
            }
        }
    }

    conn.execute(
        "INSERT INTO index_stats (key, value) VALUES ('embedding_model', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [current_model],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
