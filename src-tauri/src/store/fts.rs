use std::path::Path;

use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Schema, Value, STORED, STRING, TEXT},
    Index, TantivyDocument,
};

use crate::{types::Chunk, EngineError};

#[derive(Debug, Clone, PartialEq)]
pub struct FtsHit {
    pub chunk_id: String,
    pub score: f32,
}

pub fn create_schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("chunk_id", STRING | STORED);
    builder.add_text_field("content", TEXT | STORED);
    builder.build()
}

pub fn create_in_ram() -> Index {
    Index::create_in_ram(create_schema())
}

pub fn open_or_create(path: &Path) -> Result<Index, EngineError> {
    std::fs::create_dir_all(path)?;
    match Index::open_in_dir(path) {
        Ok(index) => Ok(index),
        Err(_) => Index::create_in_dir(path, create_schema())
            .map_err(|error| EngineError::Embed(error.to_string())),
    }
}

pub fn replace_chunks(index: &Index, chunks: &[Chunk]) -> Result<(), EngineError> {
    let schema = index.schema();
    let chunk_id = schema
        .get_field("chunk_id")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let content = schema
        .get_field("content")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let mut writer = index
        .writer(50_000_000)
        .map_err(|error| EngineError::Embed(error.to_string()))?;

    for chunk in chunks {
        writer
            .add_document(doc!(chunk_id => chunk.id.clone(), content => chunk.content.clone()))
            .map_err(|error| EngineError::Embed(error.to_string()))?;
    }

    writer
        .commit()
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    Ok(())
}

pub fn search(index: &Index, query: &str, limit: usize) -> Result<Vec<FtsHit>, EngineError> {
    let schema = index.schema();
    let chunk_id_field = schema
        .get_field("chunk_id")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let content_field = schema
        .get_field("content")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let reader = index
        .reader()
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(index, vec![content_field]);
    let parsed = parser
        .parse_query(query)
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let top_docs = searcher
        .search(&parsed, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|error| EngineError::Embed(error.to_string()))?;

    let mut hits = Vec::new();
    for (score, address) in top_docs {
        let doc: TantivyDocument = searcher
            .doc(address)
            .map_err(|error| EngineError::Embed(error.to_string()))?;
        if let Some(chunk_id) = doc
            .get_first(chunk_id_field)
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
        {
            hits.push(FtsHit { chunk_id, score });
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::{create_in_ram, replace_chunks, search};
    use crate::types::Chunk;

    #[test]
    fn indexes_and_searches_chunks_with_bm25() {
        let index = create_in_ram();
        let chunks = vec![Chunk {
            id: "chunk-a".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_index: 0,
            content: "promo printer thermal".to_string(),
            char_start: 0,
            char_end: 21,
            page: None,
        }];

        replace_chunks(&index, &chunks).expect("index chunks");
        let hits = search(&index, "thermal", 5).expect("search chunks");

        assert_eq!(hits[0].chunk_id, "chunk-a");
        assert!(hits[0].score > 0.0);
    }
}
