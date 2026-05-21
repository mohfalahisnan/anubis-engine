use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::EngineError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VectorHit {
    pub chunk_id: String,
    pub score: f32,
}

pub fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub fn decode_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

pub fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (left, right) in a.iter().zip(b.iter()) {
        dot += left * right;
        norm_a += left * left;
        norm_b += right * right;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(0.0, 1.0)
}

pub fn upsert_vector(
    conn: &Connection,
    chunk_id: &str,
    embedding: &[f32],
) -> Result<(), EngineError> {
    conn.execute(
        r#"
        INSERT INTO vectors (chunk_id, embedding, dim)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(chunk_id) DO UPDATE SET
            embedding = excluded.embedding,
            dim = excluded.dim
        "#,
        params![
            chunk_id,
            encode_embedding(embedding),
            embedding.len() as i64
        ],
    )?;
    Ok(())
}

/// Existing chunk vectors EXCLUDING those belonging to the given document.
/// Used by the indexer to build cross-document semantic edges without
/// re-comparing against the document being indexed.
pub fn vectors_excluding_doc(
    conn: &Connection,
    exclude_doc_id: &str,
) -> Result<Vec<(String, String, Vec<f32>)>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT v.chunk_id, c.doc_id, v.embedding
        FROM vectors v
        JOIN chunks c ON c.id = v.chunk_id
        WHERE c.doc_id != ?1
        "#,
    )?;
    let rows = stmt.query_map([exclude_doc_id], |row| {
        let chunk_id: String = row.get(0)?;
        let doc_id: String = row.get(1)?;
        let blob: Vec<u8> = row.get(2)?;
        Ok((chunk_id, doc_id, decode_embedding(&blob)))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn search_vectors(
    conn: &Connection,
    query: &[f32],
    limit: usize,
) -> Result<Vec<VectorHit>, EngineError> {
    let mut stmt = conn.prepare("SELECT chunk_id, embedding FROM vectors")?;
    let rows = stmt.query_map([], |row| {
        let chunk_id: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        Ok((chunk_id, blob))
    })?;

    let mut hits = Vec::new();
    for row in rows {
        let (chunk_id, blob) = row?;
        let embedding = decode_embedding(&blob);
        hits.push(VectorHit {
            chunk_id,
            score: cosine_sim(query, &embedding),
        });
    }

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::{cosine_sim, decode_embedding, encode_embedding, search_vectors, upsert_vector};
    use crate::store::db::migrate;

    #[test]
    fn cosine_sim_matches_known_values() {
        assert!((cosine_sim(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < f32::EPSILON);
        assert!(cosine_sim(&[1.0, 0.0], &[0.0, 1.0]).abs() < f32::EPSILON);
        assert!((cosine_sim(&[1.0, 1.0], &[1.0, 0.0]) - 0.70710677).abs() < 0.00001);
    }

    #[test]
    fn encodes_embedding_as_little_endian_float_blob() {
        let blob = encode_embedding(&[1.0, -2.5]);

        assert_eq!(blob.len(), 8);
        assert_eq!(decode_embedding(&blob), vec![1.0, -2.5]);
    }

    #[test]
    fn stores_and_searches_vectors_by_cosine() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory sqlite");
        migrate(&conn).expect("migration");
        conn.execute(
            "INSERT INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status) VALUES ('doc-1', 'sample.md', 'sample.md', 'md', 1, 'hash', '2026-05-21T00:00:00Z', 'indexed')",
            [],
        )
        .expect("insert document");
        conn.execute(
            "INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, created_at) VALUES ('chunk-a', 'doc-1', 0, 'alpha', 0, 5, '2026-05-21T00:00:00Z')",
            [],
        )
        .expect("insert chunk a");
        conn.execute(
            "INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, created_at) VALUES ('chunk-b', 'doc-1', 1, 'beta', 6, 10, '2026-05-21T00:00:00Z')",
            [],
        )
        .expect("insert chunk b");

        upsert_vector(&conn, "chunk-a", &[1.0, 0.0]).expect("insert vector a");
        upsert_vector(&conn, "chunk-b", &[0.0, 1.0]).expect("insert vector b");

        let results = search_vectors(&conn, &[1.0, 0.0], 2).expect("vector search");

        assert_eq!(results[0].chunk_id, "chunk-a");
        assert_eq!(results[0].score, 1.0);
        assert_eq!(results[1].chunk_id, "chunk-b");
    }
}
