use std::collections::HashSet;

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::{entities::EntityHit, store::graph_store::GraphEdge, EngineError};

/// Cap: an entity that appears in too many chunks is unhelpful for edges.
/// (E.g. a brand name that's on every page.) Kept tight so the graph isn't
/// dominated by `shared_entity` noise.
const SHARED_ENTITY_CHUNK_CAP: i64 = 8;
/// If an entity value appears in more than this fraction of all documents we
/// treat it as a stopword and skip edge creation entirely. Catches things
/// like the project name itself or a common org acronym.
const SHARED_ENTITY_DOC_FRAC_CAP: f32 = 0.5;
const SHARED_ENTITY_WEIGHT_PROPER: f32 = 0.65;
const SHARED_ENTITY_WEIGHT_DATE: f32 = 0.6;
const SHARED_ENTITY_WEIGHT_PHRASE: f32 = 0.7;

pub fn insert_entities(conn: &Connection, hits: &[EntityHit]) -> Result<(), EngineError> {
    for hit in hits {
        let entity_id = Uuid::new_v4().to_string();
        let normalized_value = normalize_entity_value(&hit.value);
        conn.execute(
            r#"
            INSERT INTO entities (id, chunk_id, entity_type, value, normalized_value, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                entity_id,
                hit.chunk_id,
                hit.entity_type,
                hit.value,
                normalized_value,
                hit.confidence as f64,
            ],
        )?;
        for term in entity_terms_for_value(&normalized_value) {
            conn.execute(
                r#"
                INSERT OR IGNORE INTO entity_terms (entity_id, chunk_id, term)
                VALUES (?1, ?2, ?3)
                "#,
                params![entity_id, hit.chunk_id, term],
            )?;
        }
    }
    Ok(())
}

pub fn normalize_entity_value(value: &str) -> String {
    if let Some(date) = normalize_date(value) {
        return date;
    }

    let mut normalized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                normalized.push(lower);
            }
        } else {
            normalized.push(' ');
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn entity_terms_for_value(normalized_value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    normalized_value
        .split_whitespace()
        .filter(|term| term.len() >= 2)
        .filter_map(|term| {
            if seen.insert(term.to_string()) {
                Some(term.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn normalize_date(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let sep = if trimmed.contains('/') {
        '/'
    } else if trimmed.contains('-') {
        '-'
    } else {
        return None;
    };
    let parts: Vec<&str> = trimmed.split(sep).collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    if !parts
        .iter()
        .all(|part| part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return None;
    }

    let nums = parts
        .iter()
        .map(|part| part.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let (year, month, day) = if parts[0].len() == 4 {
        (nums[0], nums[1], nums[2])
    } else {
        let year = if nums[2] < 100 {
            2000 + nums[2]
        } else {
            nums[2]
        };
        (year, nums[1], nums[0])
    };

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// For the given doc's chunks, find chunks in OTHER docs that share entity
/// values and produce shared_entity edges between them. Caps fan-out so a
/// hot keyword doesn't explode the graph.
pub fn build_shared_entity_edges(
    conn: &Connection,
    new_doc_id: &str,
) -> Result<Vec<GraphEdge>, EngineError> {
    // KEYWORD entities (top-N most frequent tokens per chunk) are far too
    // common to make for useful cross-doc edges — they explode the graph with
    // weak signal. Restrict edges to PROPER, DATE, and PHRASE, where shared
    // occurrence actually implies topical overlap.
    let mut new_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id, e.entity_type, COALESCE(e.normalized_value, e.value)
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        WHERE c.doc_id = ?1
          AND e.entity_type IN ('PROPER', 'DATE', 'PHRASE')
        "#,
    )?;
    let new_rows: Vec<(String, String, String)> = new_stmt
        .query_map([new_doc_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let total_docs: i64 =
        conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
    let doc_frac_cap = ((total_docs as f32) * SHARED_ENTITY_DOC_FRAC_CAP).ceil() as i64;

    let mut doc_freq_stmt = conn.prepare(
        r#"
        SELECT COUNT(DISTINCT c.doc_id)
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        WHERE e.entity_type = ?1 AND COALESCE(e.normalized_value, e.value) = ?2
        "#,
    )?;

    let mut matched_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        WHERE e.entity_type = ?1 AND COALESCE(e.normalized_value, e.value) = ?2 AND c.doc_id != ?3
        LIMIT ?4
        "#,
    )?;

    let mut edges = Vec::new();
    for (new_chunk_id, entity_type, value) in &new_rows {
        // Stopword guard: if this value already shows up in too many docs, it
        // isn't discriminative — skip edge creation.
        if total_docs > 4 {
            let doc_freq: i64 = doc_freq_stmt
                .query_row(params![entity_type, value], |row| row.get(0))
                .unwrap_or(0);
            if doc_freq > doc_frac_cap {
                continue;
            }
        }

        let weight = match entity_type.as_str() {
            "DATE" => SHARED_ENTITY_WEIGHT_DATE,
            "PROPER" => SHARED_ENTITY_WEIGHT_PROPER,
            "PHRASE" => SHARED_ENTITY_WEIGHT_PHRASE,
            _ => continue,
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

    #[test]
    fn stores_normalized_entity_values_and_terms() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "doc-a", &["a1"]);

        insert_entities(
            &conn,
            &[EntityHit {
                chunk_id: "a1".to_string(),
                entity_type: "PHRASE".to_string(),
                value: "Anubis OS".to_string(),
                confidence: 0.8,
            }],
        )
        .unwrap();

        let normalized: String = conn
            .query_row(
                "SELECT normalized_value FROM entities WHERE chunk_id = 'a1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let terms: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entity_terms WHERE chunk_id = 'a1' AND term IN ('anubis', 'os')",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(normalized, "anubis os");
        assert_eq!(terms, 2);
    }

    #[test]
    fn normalizes_date_variants_to_same_value() {
        assert_eq!(normalize_entity_value("21/05/2026"), "2026-05-21");
        assert_eq!(normalize_entity_value("21-05-26"), "2026-05-21");
        assert_eq!(normalize_entity_value("2026-05-21"), "2026-05-21");
    }
}
