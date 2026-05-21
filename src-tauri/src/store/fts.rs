use std::path::Path;

use rusqlite::Connection;
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Schema, Value, STORED, STRING, TEXT},
    Index, TantivyDocument, Term,
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
        .writer::<TantivyDocument>(50_000_000)
        .map_err(|error| EngineError::Embed(error.to_string()))?;

    for chunk in chunks {
        writer.delete_term(Term::from_field_text(chunk_id, &chunk.id));
        writer
            .add_document(doc!(chunk_id => chunk.id.clone(), content => chunk.content.clone()))
            .map_err(|error| EngineError::Embed(error.to_string()))?;
    }

    writer
        .commit()
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    Ok(())
}

pub fn delete_chunks(index: &Index, chunk_ids: &[String]) -> Result<(), EngineError> {
    if chunk_ids.is_empty() {
        return Ok(());
    }

    let schema = index.schema();
    let chunk_id = schema
        .get_field("chunk_id")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let mut writer = index
        .writer::<TantivyDocument>(50_000_000)
        .map_err(|error| EngineError::Embed(error.to_string()))?;

    for id in chunk_ids {
        writer.delete_term(Term::from_field_text(chunk_id, id));
    }

    writer
        .commit()
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    Ok(())
}

pub fn clear(index: &Index) -> Result<(), EngineError> {
    let mut writer = index
        .writer::<TantivyDocument>(50_000_000)
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    writer
        .delete_all_documents()
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    writer
        .commit()
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    Ok(())
}

pub fn rebuild_from_indexed_chunks(index: &Index, conn: &Connection) -> Result<(), EngineError> {
    let schema = index.schema();
    let chunk_id = schema
        .get_field("chunk_id")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let content = schema
        .get_field("content")
        .map_err(|error| EngineError::Embed(error.to_string()))?;
    let mut writer = index
        .writer::<TantivyDocument>(50_000_000)
        .map_err(|error| EngineError::Embed(error.to_string()))?;

    writer
        .delete_all_documents()
        .map_err(|error| EngineError::Embed(error.to_string()))?;

    let mut stmt = conn.prepare(
        r#"
        SELECT c.id, c.content
        FROM chunks c
        JOIN documents d ON d.id = c.doc_id
        WHERE d.status = 'indexed'
        ORDER BY d.indexed_at ASC, c.chunk_index ASC
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (id, text) = row?;
        writer
            .add_document(doc!(chunk_id => id, content => text))
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
    use super::{
        clear, create_in_ram, delete_chunks, rebuild_from_indexed_chunks, replace_chunks, search,
    };
    use crate::store::db::migrate;
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

    #[test]
    fn deletes_old_chunk_docs_during_reindex() {
        let index = create_in_ram();
        let old_chunks = vec![Chunk {
            id: "chunk-old".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_index: 0,
            content: "alpha legacy text".to_string(),
            char_start: 0,
            char_end: 17,
            page: None,
        }];
        let new_chunks = vec![Chunk {
            id: "chunk-new".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_index: 0,
            content: "beta current text".to_string(),
            char_start: 0,
            char_end: 17,
            page: None,
        }];

        replace_chunks(&index, &old_chunks).expect("index old chunks");
        delete_chunks(&index, &["chunk-old".to_string()]).expect("delete old chunks");
        replace_chunks(&index, &new_chunks).expect("index new chunks");

        assert!(search(&index, "alpha", 5)
            .expect("search old term")
            .is_empty());
        let hits = search(&index, "beta", 5).expect("search new term");
        assert_eq!(hits[0].chunk_id, "chunk-new");
    }

    #[test]
    fn clear_removes_all_indexed_docs() {
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
        clear(&index).expect("clear fts");

        assert!(search(&index, "thermal", 5)
            .expect("search cleared index")
            .is_empty());
    }

    #[test]
    fn rebuild_from_sqlite_indexes_only_indexed_documents() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");
        migrate(&conn).expect("migration");
        conn.execute_batch(
            r#"
            INSERT INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status)
            VALUES
                ('doc-indexed', 'indexed.md', 'indexed.md', 'md', 1, 'h1', '2026-05-21T00:00:00Z', 'indexed'),
                ('doc-pending', 'pending.md', 'pending.md', 'md', 1, 'h2', '2026-05-21T00:00:00Z', 'pending');

            INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, created_at)
            VALUES
                ('chunk-indexed', 'doc-indexed', 0, 'current searchable content', 0, 26, '2026-05-21T00:00:00Z'),
                ('chunk-pending', 'doc-pending', 0, 'pending stale content', 0, 21, '2026-05-21T00:00:00Z');
            "#,
        )
        .expect("seed sqlite");
        let index = create_in_ram();
        replace_chunks(
            &index,
            &[Chunk {
                id: "stale".to_string(),
                doc_id: "old".to_string(),
                chunk_index: 0,
                content: "legacy stale content".to_string(),
                char_start: 0,
                char_end: 20,
                page: None,
            }],
        )
        .expect("seed stale fts");

        rebuild_from_indexed_chunks(&index, &conn).expect("rebuild fts");

        assert!(search(&index, "legacy", 5)
            .expect("search stale")
            .is_empty());
        assert!(search(&index, "pending", 5)
            .expect("search pending")
            .is_empty());
        let hits = search(&index, "current", 5).expect("search current");
        assert_eq!(hits[0].chunk_id, "chunk-indexed");
    }
}
