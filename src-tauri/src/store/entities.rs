use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::{entities::EntityHit, store::graph_store::GraphEdge, EngineError};

/// Cap: an entity that appears in too many chunks is unhelpful for edges.
/// (E.g. a brand name that's on every page.)
const SHARED_ENTITY_CHUNK_CAP: i64 = 20;
const SHARED_ENTITY_WEIGHT_KEYWORD: f32 = 0.55;
const SHARED_ENTITY_WEIGHT_PROPER: f32 = 0.65;
const SHARED_ENTITY_WEIGHT_DATE: f32 = 0.6;

pub fn insert_entities(conn: &Connection, hits: &[EntityHit]) -> Result<(), EngineError> {
    for hit in hits {
        conn.execute(
            r#"
            INSERT INTO entities (id, chunk_id, entity_type, value, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                Uuid::new_v4().to_string(),
                hit.chunk_id,
                hit.entity_type,
                hit.value,
                hit.confidence as f64,
            ],
        )?;
    }
    Ok(())
}

/// For the given doc's chunks, find chunks in OTHER docs that share entity
/// values and produce shared_entity edges between them. Caps fan-out so a
/// hot keyword doesn't explode the graph.
pub fn build_shared_entity_edges(
    conn: &Connection,
    new_doc_id: &str,
) -> Result<Vec<GraphEdge>, EngineError> {
    let mut new_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id, e.entity_type, e.value
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        WHERE c.doc_id = ?1
        "#,
    )?;
    let new_rows: Vec<(String, String, String)> = new_stmt
        .query_map([new_doc_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut matched_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        WHERE e.entity_type = ?1 AND e.value = ?2 AND c.doc_id != ?3
        LIMIT ?4
        "#,
    )?;

    let mut edges = Vec::new();
    for (new_chunk_id, entity_type, value) in &new_rows {
        let weight = match entity_type.as_str() {
            "DATE" => SHARED_ENTITY_WEIGHT_DATE,
            "PROPER" => SHARED_ENTITY_WEIGHT_PROPER,
            _ => SHARED_ENTITY_WEIGHT_KEYWORD,
        };
        let matches: Vec<String> = matched_stmt
            .query_map(
                params![entity_type, value, new_doc_id, SHARED_ENTITY_CHUNK_CAP],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        for other_chunk_id in matches {
            edges.push(GraphEdge::canonical(
                new_chunk_id,
                &other_chunk_id,
                weight,
                "shared_entity",
            ));
        }
    }
    Ok(edges)
}

pub fn entity_count(conn: &Connection) -> Result<i64, EngineError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::EntityHit;
    use crate::store::db::migrate;

    fn seed(conn: &Connection, doc_id: &str, chunk_ids: &[&str]) {
        conn.execute(
            "INSERT INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status) VALUES (?1, ?1, ?1, 'md', 1, 'h', '2026-05-21T00:00:00Z', 'indexed')",
            [doc_id],
        ).unwrap();
        for (idx, cid) in chunk_ids.iter().enumerate() {
            conn.execute(
                "INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, created_at) VALUES (?1, ?2, ?3, 'x', 0, 1, '2026-05-21T00:00:00Z')",
                params![cid, doc_id, idx as i64],
            ).unwrap();
        }
    }

    #[test]
    fn shared_entity_edges_connect_chunks_across_docs() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "doc-a", &["a1"]);
        seed(&conn, "doc-b", &["b1"]);

        insert_entities(
            &conn,
            &[
                EntityHit {
                    chunk_id: "a1".to_string(),
                    entity_type: "PROPER".to_string(),
                    value: "Anubis".to_string(),
                    confidence: 0.7,
                },
                EntityHit {
                    chunk_id: "b1".to_string(),
                    entity_type: "PROPER".to_string(),
                    value: "Anubis".to_string(),
                    confidence: 0.7,
                },
            ],
        )
        .unwrap();

        let edges = build_shared_entity_edges(&conn, "doc-a").unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_type, "shared_entity");
    }
}
