use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{embedder, store, EngineError};

/// Shared handle to the engine. `None` while first-run initialisation is
/// still in progress (model downloads, schema migration, FTS reconcile).
/// Wrapped in [`tokio::sync::OnceCell`] so commands can `.get()` it without
/// blocking — they return a friendly "still initialising" error when the
/// engine isn't ready.
pub type EngineHandle = Arc<tokio::sync::OnceCell<AppState>>;

pub fn new_engine_handle() -> EngineHandle {
    Arc::new(tokio::sync::OnceCell::new())
}

/// Identifier persisted in index_stats so we can detect a model swap and
/// invalidate stale vectors. Bump the string whenever the embedding model
/// or its prompt template changes.
const EMBEDDING_MODEL_ID: &str = "intfloat/multilingual-e5-small@v1";

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    pub fts: Arc<Mutex<tantivy::Index>>,
    pub graph: Arc<Mutex<petgraph::Graph<String, f32>>>,
    pub indexing: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new(db_path: &std::path::Path, fts_path: &std::path::Path) -> Result<Self, EngineError> {
        // Wire the OCR + embedding model directories before any module
        // touches them. Both live next to the SQLite database.
        if let Some(parent) = db_path.parent() {
            crate::ocr::engine::set_models_dir(parent.to_path_buf());
            embedder::download::set_models_dir(parent.to_path_buf());
            crate::transcription::engine::set_models_dir(parent.to_path_buf());
        }

        let db = store::db::open(db_path)?;
        // If the user previously indexed with a different embedding model, the
        // old vectors are no longer comparable to fresh query embeddings.
        // Detect a swap and clear stale derived data (vectors + chunks + edges
        // + entities), then mark every document as pending so the next index
        // pass regenerates them.
        invalidate_on_model_change(&db, EMBEDDING_MODEL_ID)?;

        let fts = store::fts::open_or_create(fts_path)?;
        store::fts::rebuild_from_indexed_chunks(&fts, &db)?;
        tracing::info!("reconciled full-text index from SQLite chunks");
        // MultilingualE5Small: 384 dim (same as previous AllMiniLML6V2 — no
        // vector-store migration needed), but trained on 100+ languages with
        // strong recall on Indonesian content.
        //
        // Downloading is done by our own ureq client (longer timeout, real
        // progress events). `load_or_download` emits start/downloading/ready
        // / error itself.
        let embedder = embedder::download::load_or_download()?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            embedder: Arc::new(Mutex::new(embedder)),
            fts: Arc::new(Mutex::new(fts)),
            graph: Arc::new(Mutex::new(petgraph::Graph::new())),
            indexing: Arc::new(Mutex::new(false)),
        })
    }
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
