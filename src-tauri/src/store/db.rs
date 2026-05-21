use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{types::DocFormat, EngineError};

pub const SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentRecord {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub format: DocFormat,
    pub size_bytes: u64,
    pub hash: String,
    pub indexed_at: String,
    pub status: String,
    pub error_msg: Option<String>,
}

pub fn open(path: &Path) -> Result<Connection, EngineError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<(), EngineError> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS documents (
            id          TEXT PRIMARY KEY,
            path        TEXT NOT NULL UNIQUE,
            filename    TEXT NOT NULL,
            format      TEXT NOT NULL,
            size_bytes  INTEGER NOT NULL,
            hash        TEXT NOT NULL,
            indexed_at  TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'pending',
            error_msg   TEXT
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id          TEXT PRIMARY KEY,
            doc_id      TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            chunk_index INTEGER NOT NULL,
            content     TEXT NOT NULL,
            char_start  INTEGER NOT NULL,
            char_end    INTEGER NOT NULL,
            page        INTEGER,
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS vectors (
            chunk_id    TEXT PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
            embedding   BLOB NOT NULL,
            dim         INTEGER NOT NULL DEFAULT 384
        );

        CREATE TABLE IF NOT EXISTS graph_edges (
            src_chunk   TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
            dst_chunk   TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
            weight      REAL NOT NULL,
            edge_type   TEXT NOT NULL,
            PRIMARY KEY (src_chunk, dst_chunk)
        );

        CREATE TABLE IF NOT EXISTS entities (
            id          TEXT PRIMARY KEY,
            chunk_id    TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
            entity_type TEXT NOT NULL,
            value       TEXT NOT NULL,
            confidence  REAL NOT NULL DEFAULT 1.0
        );

        CREATE TABLE IF NOT EXISTS communities (
            id          TEXT PRIMARY KEY,
            label       TEXT NOT NULL,
            chunk_ids   TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS index_stats (
            key         TEXT PRIMARY KEY,
            value       TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_doc ON chunks(doc_id);
        CREATE INDEX IF NOT EXISTS idx_entities_chunk ON entities(chunk_id);
        CREATE INDEX IF NOT EXISTS idx_edges_src ON graph_edges(src_chunk);
        CREATE INDEX IF NOT EXISTS idx_docs_status ON documents(status);
        "#,
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

pub fn upsert_document(conn: &Connection, doc: &DocumentRecord) -> Result<(), EngineError> {
    conn.execute(
        r#"
        INSERT INTO documents (
            id, path, filename, format, size_bytes, hash, indexed_at, status, error_msg
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(path) DO UPDATE SET
            id = excluded.id,
            filename = excluded.filename,
            format = excluded.format,
            size_bytes = excluded.size_bytes,
            hash = excluded.hash,
            indexed_at = excluded.indexed_at,
            status = excluded.status,
            error_msg = excluded.error_msg
        "#,
        params![
            doc.id,
            doc.path,
            doc.filename,
            doc.format.as_str(),
            doc.size_bytes as i64,
            doc.hash,
            doc.indexed_at,
            doc.status,
            doc.error_msg
        ],
    )?;
    Ok(())
}

pub fn get_document_by_path(
    conn: &Connection,
    path: &str,
) -> Result<Option<DocumentRecord>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, path, filename, format, size_bytes, hash, indexed_at, status, error_msg
        FROM documents
        WHERE path = ?1
        "#,
    )?;
    let mut rows = stmt.query([path])?;

    match rows.next()? {
        Some(row) => Ok(Some(row_to_document(row)?)),
        None => Ok(None),
    }
}

pub fn get_document_by_id(
    conn: &Connection,
    doc_id: &str,
) -> Result<Option<DocumentRecord>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, path, filename, format, size_bytes, hash, indexed_at, status, error_msg
        FROM documents
        WHERE id = ?1
        "#,
    )?;
    let mut rows = stmt.query([doc_id])?;

    match rows.next()? {
        Some(row) => Ok(Some(row_to_document(row)?)),
        None => Ok(None),
    }
}

pub fn list_documents(conn: &Connection) -> Result<Vec<serde_json::Value>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, path, filename, format, size_bytes, hash, indexed_at, status, error_msg
        FROM documents
        ORDER BY indexed_at DESC, filename ASC
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        let doc = row_to_document(row)?;
        Ok(json!({
            "id": doc.id,
            "path": doc.path,
            "filename": doc.filename,
            "format": doc.format,
            "size_bytes": doc.size_bytes,
            "hash": doc.hash,
            "indexed_at": doc.indexed_at,
            "status": doc.status,
            "error_msg": doc.error_msg,
        }))
    })?;

    let mut docs = Vec::new();
    for row in rows {
        docs.push(row?);
    }
    Ok(docs)
}

pub fn mark_document_status(
    conn: &Connection,
    doc_id: &str,
    status: &str,
    error_msg: Option<&str>,
) -> Result<(), EngineError> {
    conn.execute(
        "UPDATE documents SET status = ?1, error_msg = ?2 WHERE id = ?3",
        params![status, error_msg, doc_id],
    )?;
    Ok(())
}

pub fn delete_document(conn: &Connection, doc_id: &str) -> Result<(), EngineError> {
    conn.execute("DELETE FROM documents WHERE id = ?1", [doc_id])?;
    Ok(())
}

pub fn reset_index(conn: &Connection) -> Result<(), EngineError> {
    conn.execute_batch(
        r#"
        DELETE FROM communities;
        DELETE FROM entities;
        DELETE FROM graph_edges;
        DELETE FROM vectors;
        DELETE FROM chunks;
        DELETE FROM documents;
        DELETE FROM index_stats;
        "#,
    )?;
    Ok(())
}

pub fn get_index_stats(conn: &Connection) -> Result<serde_json::Value, EngineError> {
    let documents: i64 = conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
    let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
    let graph_edges: i64 =
        conn.query_row("SELECT COUNT(*) FROM graph_edges", [], |row| row.get(0))?;
    let entities: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;
    let last_indexed: Option<String> =
        conn.query_row("SELECT MAX(indexed_at) FROM documents", [], |row| {
            row.get(0)
        })?;

    let edges_by_type = crate::store::graph_store::edge_count_by_type(conn)?;
    let edges_by_type_json: serde_json::Map<String, serde_json::Value> = edges_by_type
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::from(v)))
        .collect();

    Ok(json!({
        "documents": documents,
        "chunks": chunks,
        "graph_edges": graph_edges,
        "entities": entities,
        "edges_by_type": edges_by_type_json,
        "last_indexed": last_indexed,
    }))
}

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentRecord> {
    let format: String = row.get(3)?;
    let size_bytes: i64 = row.get(4)?;
    Ok(DocumentRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        filename: row.get(2)?,
        format: DocFormat::from_db(&format),
        size_bytes: size_bytes.max(0) as u64,
        hash: row.get(5)?,
        indexed_at: row.get(6)?,
        status: row.get(7)?,
        error_msg: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        delete_document, get_document_by_path, get_index_stats, migrate, upsert_document,
        DocumentRecord,
    };
    use crate::types::DocFormat;

    #[test]
    fn migrate_creates_schema_and_sets_user_version() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");

        migrate(&conn).expect("migration");

        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("user_version");
        let documents_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'documents'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        let chunks_doc_index_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_chunks_doc'",
                [],
                |row| row.get(0),
            )
            .expect("index count");

        assert_eq!(version, 1);
        assert_eq!(documents_exists, 1);
        assert_eq!(chunks_doc_index_exists, 1);
    }

    #[test]
    fn upsert_get_stats_and_delete_document() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");
        migrate(&conn).expect("migration");
        let doc = DocumentRecord {
            id: "doc-1".to_string(),
            path: "D:\\knowledge\\sample.md".to_string(),
            filename: "sample.md".to_string(),
            format: DocFormat::Markdown,
            size_bytes: 128,
            hash: "hash-a".to_string(),
            indexed_at: "2026-05-21T00:00:00Z".to_string(),
            status: "indexed".to_string(),
            error_msg: None,
        };

        upsert_document(&conn, &doc).expect("insert document");
        let loaded = get_document_by_path(&conn, &doc.path)
            .expect("get document")
            .expect("document exists");
        let stats = get_index_stats(&conn).expect("stats");

        assert_eq!(loaded.id, "doc-1");
        assert_eq!(loaded.format, DocFormat::Markdown);
        assert_eq!(stats["documents"], serde_json::json!(1));
        assert_eq!(stats["chunks"], serde_json::json!(0));

        delete_document(&conn, "doc-1").expect("delete document");

        assert!(get_document_by_path(&conn, &doc.path)
            .expect("get deleted document")
            .is_none());
    }
}
