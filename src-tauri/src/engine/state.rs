use std::sync::Arc;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tokio::sync::Mutex;

use crate::{store, EngineError};

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub embedder: Arc<Mutex<fastembed::TextEmbedding>>,
    pub fts: Arc<Mutex<tantivy::Index>>,
    pub graph: Arc<Mutex<petgraph::Graph<String, f32>>>,
    pub indexing: Arc<Mutex<bool>>,
}

impl AppState {
    pub fn new(db_path: &std::path::Path, fts_path: &std::path::Path) -> Result<Self, EngineError> {
        let db = store::db::open(db_path)?;
        let fts = store::fts::open_or_create(fts_path)?;
        let embedder = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))
            .map_err(|error| EngineError::Embed(error.to_string()))?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            embedder: Arc::new(Mutex::new(embedder)),
            fts: Arc::new(Mutex::new(fts)),
            graph: Arc::new(Mutex::new(petgraph::Graph::new())),
            indexing: Arc::new(Mutex::new(false)),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
