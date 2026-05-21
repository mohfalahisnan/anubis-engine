use chrono::Utc;
use rusqlite::{params, Connection};

use crate::{
    types::{Chunk, QueryResult},
    EngineError,
};

pub fn replace_doc_chunks(
    conn: &mut Connection,
    doc_id: &str,
    chunks: &[Chunk],
) -> Result<(), EngineError> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM chunks WHERE doc_id = ?1", [doc_id])?;
    let created_at = Utc::now().to_rfc3339();

    for chunk in chunks {
        tx.execute(
            r#"
            INSERT INTO chunks (
                id, doc_id, chunk_index, content, char_start, char_end, page, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                chunk.id,
                chunk.doc_id,
                chunk.chunk_index as i64,
                chunk.content,
                chunk.char_start as i64,
                chunk.char_end as i64,
                chunk.page.map(|page| page as i64),
                created_at
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn get_doc_chunks(conn: &Connection, doc_id: &str) -> Result<Vec<Chunk>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, doc_id, chunk_index, content, char_start, char_end, page
        FROM chunks
        WHERE doc_id = ?1
        ORDER BY chunk_index ASC
        "#,
    )?;
    let rows = stmt.query_map([doc_id], row_to_chunk)?;

    let mut chunks = Vec::new();
    for row in rows {
        chunks.push(row?);
    }
    Ok(chunks)
}

pub fn query_result_for_chunk(
    conn: &Connection,
    chunk_id: &str,
    score: f32,
    score_bm25: f32,
    score_vec: f32,
    score_graph: f32,
    score_entity: f32,
    score_centrality: f32,
) -> Result<Option<QueryResult>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT c.id, c.doc_id, c.content, d.filename, c.page
        FROM chunks c
        JOIN documents d ON d.id = c.doc_id
        WHERE c.id = ?1
        "#,
    )?;
    let mut rows = stmt.query([chunk_id])?;

    match rows.next()? {
        Some(row) => Ok(Some(QueryResult {
            chunk_id: row.get(0)?,
            doc_id: row.get(1)?,
            content: row.get(2)?,
            filename: row.get(3)?,
            page: row.get::<_, Option<i64>>(4)?.map(|page| page as u32),
            score,
            score_bm25,
            score_vec,
            score_graph,
            score_entity,
            score_centrality,
        })),
        None => Ok(None),
    }
}

fn row_to_chunk(row: &rusqlite::Row<'_>) -> rusqlite::Result<Chunk> {
    let page: Option<i64> = row.get(6)?;
    Ok(Chunk {
        id: row.get(0)?,
        doc_id: row.get(1)?,
        chunk_index: row.get::<_, i64>(2)? as usize,
        content: row.get(3)?,
        char_start: row.get::<_, i64>(4)? as usize,
        char_end: row.get::<_, i64>(5)? as usize,
        page: page.map(|value| value as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::{get_doc_chunks, replace_doc_chunks};
    use crate::{store::db::migrate, types::Chunk};

    #[test]
    fn replaces_and_fetches_document_chunks() {
        let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");
        migrate(&conn).expect("migration");
        conn.execute(
            "INSERT INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status) VALUES ('doc-1', 'sample.md', 'sample.md', 'md', 1, 'hash', '2026-05-21T00:00:00Z', 'indexed')",
            [],
        )
        .expect("insert document");
        let chunks = vec![Chunk {
            id: "chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_index: 0,
            content: "A useful chunk".to_string(),
            char_start: 0,
            char_end: 14,
            page: None,
        }];

        replace_doc_chunks(&mut conn, "doc-1", &chunks).expect("replace chunks");
        let loaded = get_doc_chunks(&conn, "doc-1").expect("load chunks");

        assert_eq!(loaded, chunks);
    }
}
