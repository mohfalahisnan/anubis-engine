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
/// Anchors (`VID-APPROVAL-005` etc.) are inherently discriminative — they
/// produce the highest-confidence cross-doc relation. Above
/// `STRONG_EDGE_THRESHOLD` in the hybrid query so anchor matches fully
/// drive graph expansion.
const SHARED_ANCHOR_WEIGHT: f32 = 0.9;
/// Manifest-overlap edges (both endpoints are reference-class docs that
/// happen to share an anchor) are stored for UI visibility but never drive
/// graph expansion. Low weight keeps them visible without surfacing on Q&A.
const MANIFEST_OVERLAP_WEIGHT: f32 = 0.3;

/// Insert a batch of entity hits and their derived terms.
///
/// Wrapped in a single explicit transaction so the entire batch costs one
/// commit (one fsync), not one per row. Without this, a JSON file producing
/// ~30K entity hits would take minutes — see
/// `docs/superpowers/specs/2026-05-22-preprocessing-prepass-design.md`.
pub fn insert_entities(conn: &mut Connection, hits: &[EntityHit]) -> Result<(), EngineError> {
    if hits.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    {
        let mut entity_stmt = tx.prepare(
            r#"
            INSERT INTO entities (id, chunk_id, entity_type, value, normalized_value, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )?;
        let mut term_stmt = tx.prepare(
            r#"
            INSERT OR IGNORE INTO entity_terms (entity_id, chunk_id, term)
            VALUES (?1, ?2, ?3)
            "#,
        )?;
        for hit in hits {
            let entity_id = Uuid::new_v4().to_string();
            let normalized_value = normalize_entity_value(&hit.value);
            entity_stmt.execute(params![
                entity_id,
                hit.chunk_id,
                hit.entity_type,
                hit.value,
                normalized_value,
                hit.confidence as f64,
            ])?;
            for term in entity_terms_for_value(&normalized_value) {
                term_stmt.execute(params![entity_id, hit.chunk_id, term])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn normalize_entity_value(value: &str) -> String {
    if let Some(date) = normalize_date(value) {
        return date;
    }
    // Preserve anchor-shaped IDs literally so `VID-APPROVAL-005` round-trips
    // through both the insert and query paths without losing its hyphens or
    // case. Both sides call this function, so the canonical form stays
    // consistent.
    if is_anchor_shaped(value.trim()) {
        return value.trim().to_string();
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

fn is_anchor_shaped(value: &str) -> bool {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"^[A-Z][A-Z0-9]+(?:-[A-Z0-9]+){2,}$")
            .expect("anchor shape regex compiles")
    });
    value.len() <= 64 && re.is_match(value)
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

/// For the given doc's chunks, find chunks in OTHER content (non-reference)
/// docs that share PROPER / PHRASE / DATE entity values, and produce
/// `shared_entity` edges. Caps fan-out so a hot keyword doesn't explode the
/// graph. Reference-class docs (manifests, README) on EITHER endpoint are
/// excluded — they get the manifest-overlap path instead.
pub fn build_shared_entity_edges(
    conn: &Connection,
    new_doc_id: &str,
) -> Result<Vec<GraphEdge>, EngineError> {
    // KEYWORD entities (top-N most frequent tokens per chunk) are far too
    // common to make for useful cross-doc edges — they explode the graph with
    // weak signal. ANCHOR has its own path (build_shared_anchor_edges).
    // Restrict here to PROPER, DATE, and PHRASE, where shared occurrence
    // actually implies topical overlap.
    let new_rows = fetch_new_entities(
        conn,
        new_doc_id,
        &["PROPER", "DATE", "PHRASE"],
        EndpointFilter::ContentOnly,
    )?;

    let total_docs: i64 = conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
    let doc_frac_cap = ((total_docs as f32) * SHARED_ENTITY_DOC_FRAC_CAP).ceil() as i64;

    let mut doc_freq_stmt = conn.prepare(
        r#"
        SELECT COUNT(DISTINCT c.doc_id)
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        WHERE e.entity_type = ?1 AND COALESCE(e.normalized_value, e.value) = ?2
          AND c.chunk_signal = 'content'
        "#,
    )?;

    // Only match against chunks in OTHER docs whose doc_class is 'content'.
    // A weak-signal entity sitting in a manifest must not form a content edge
    // with a real document.
    let mut matched_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        JOIN documents d ON d.id = c.doc_id
        WHERE e.entity_type = ?1
          AND COALESCE(e.normalized_value, e.value) = ?2
          AND c.doc_id != ?3
          AND d.doc_class = 'content'
          AND c.chunk_signal = 'content'
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
            edges.push(GraphEdge::canonical_with_reason(
                new_chunk_id,
                &other_chunk_id,
                weight,
                "shared_entity",
                Some(format_entity_reason(entity_type, value)),
            ));
        }
    }
    Ok(edges)
}

/// Cross-doc edges driven by structural ID anchors. Both endpoints must be
/// content-class — anchors that show up only in manifests get the
/// `manifest_overlap` path instead so search expansion isn't dominated by
/// reference docs that list every ticket in the project.
pub fn build_shared_anchor_edges(
    conn: &Connection,
    new_doc_id: &str,
) -> Result<Vec<GraphEdge>, EngineError> {
    let new_rows = fetch_new_entities(conn, new_doc_id, &["ANCHOR"], EndpointFilter::ContentOnly)?;

    let mut matched_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        JOIN documents d ON d.id = c.doc_id
        WHERE e.entity_type = 'ANCHOR'
          AND COALESCE(e.normalized_value, e.value) = ?1
          AND c.doc_id != ?2
          AND d.doc_class = 'content'
          AND c.chunk_signal = 'content'
        LIMIT ?3
        "#,
    )?;

    let mut edges = Vec::new();
    for (new_chunk_id, _entity_type, anchor_value) in &new_rows {
        let matches: Vec<String> = matched_stmt
            .query_map(
                params![anchor_value, new_doc_id, SHARED_ENTITY_CHUNK_CAP],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        for other_chunk_id in matches {
            edges.push(GraphEdge::canonical_with_reason(
                new_chunk_id,
                &other_chunk_id,
                SHARED_ANCHOR_WEIGHT,
                "shared_anchor",
                Some(format!("anchor:{anchor_value}")),
            ));
        }
    }
    Ok(edges)
}

/// Edges produced when BOTH endpoints are reference-class docs that share an
/// anchor (e.g. two manifests both referencing `INC-2026-ATLAS-014`). Stored
/// at low weight so the UI graph view can label them "listed in the same
/// manifest" while the search expansion path ignores them entirely.
pub fn build_manifest_overlap_edges(
    conn: &Connection,
    new_doc_id: &str,
) -> Result<Vec<GraphEdge>, EngineError> {
    // Only fires when THIS new doc is itself reference-class.
    let class: Option<String> = conn
        .query_row(
            "SELECT doc_class FROM documents WHERE id = ?1",
            [new_doc_id],
            |row| row.get(0),
        )
        .ok();
    if class.as_deref() != Some("reference") {
        return Ok(Vec::new());
    }

    let new_rows = fetch_new_entities(conn, new_doc_id, &["ANCHOR"], EndpointFilter::Any)?;

    let mut matched_stmt = conn.prepare(
        r#"
        SELECT e.chunk_id
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        JOIN documents d ON d.id = c.doc_id
        WHERE e.entity_type = 'ANCHOR'
          AND COALESCE(e.normalized_value, e.value) = ?1
          AND c.doc_id != ?2
          AND d.doc_class = 'reference'
          AND c.chunk_signal = 'content'
        LIMIT ?3
        "#,
    )?;

    let mut edges = Vec::new();
    for (new_chunk_id, _entity_type, anchor_value) in &new_rows {
        let matches: Vec<String> = matched_stmt
            .query_map(
                params![anchor_value, new_doc_id, SHARED_ENTITY_CHUNK_CAP],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        for other_chunk_id in matches {
            edges.push(GraphEdge::canonical_with_reason(
                new_chunk_id,
                &other_chunk_id,
                MANIFEST_OVERLAP_WEIGHT,
                "manifest_overlap",
                Some(format!("manifest:{anchor_value}")),
            ));
        }
    }
    Ok(edges)
}

#[derive(Copy, Clone)]
enum EndpointFilter {
    /// Only return entities from chunks in content-class docs (used for
    /// shared_anchor / shared_entity, which must not originate from manifest
    /// rows).
    ContentOnly,
    /// Return entities regardless of doc_class (used for manifest_overlap,
    /// which explicitly wants reference-class rows).
    Any,
}

fn fetch_new_entities(
    conn: &Connection,
    new_doc_id: &str,
    types: &[&str],
    filter: EndpointFilter,
) -> Result<Vec<(String, String, String)>, EngineError> {
    let placeholders = std::iter::repeat("?")
        .take(types.len())
        .collect::<Vec<_>>()
        .join(",");
    let class_filter = match filter {
        EndpointFilter::ContentOnly => " AND d.doc_class = 'content'",
        EndpointFilter::Any => "",
    };
    let sql = format!(
        r#"
        SELECT e.chunk_id, e.entity_type, COALESCE(e.normalized_value, e.value)
        FROM entities e
        JOIN chunks c ON c.id = e.chunk_id
        JOIN documents d ON d.id = c.doc_id
        WHERE c.doc_id = ?1
          AND e.entity_type IN ({placeholders})
          AND c.chunk_signal = 'content'
          {class_filter}
        "#
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&new_doc_id];
    for ty in types {
        params_vec.push(ty);
    }
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(params_vec.iter().copied()),
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn format_entity_reason(entity_type: &str, value: &str) -> String {
    match entity_type {
        "PROPER" => format!("proper:{value}"),
        "PHRASE" => format!("phrase:{value}"),
        "DATE" => format!("date:{value}"),
        other => format!("{}:{value}", other.to_ascii_lowercase()),
    }
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
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "doc-a", &["a1"]);
        seed(&conn, "doc-b", &["b1"]);

        insert_entities(
            &mut conn,
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
    fn low_signal_chunks_do_not_create_shared_entity_edges() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "doc-a", &["a1"]);
        seed(&conn, "doc-b", &["b1"]);
        conn.execute(
            "UPDATE chunks SET chunk_signal = 'anchor_list' WHERE id = 'a1'",
            [],
        )
        .unwrap();

        insert_entities(
            &mut conn,
            &[
                EntityHit {
                    chunk_id: "a1".to_string(),
                    entity_type: "ANCHOR".to_string(),
                    value: "INC-2026-ATLAS-014".to_string(),
                    confidence: 1.0,
                },
                EntityHit {
                    chunk_id: "b1".to_string(),
                    entity_type: "ANCHOR".to_string(),
                    value: "INC-2026-ATLAS-014".to_string(),
                    confidence: 1.0,
                },
            ],
        )
        .unwrap();

        assert!(build_shared_anchor_edges(&conn, "doc-a")
            .unwrap()
            .is_empty());
        assert!(build_shared_anchor_edges(&conn, "doc-b")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn stores_normalized_entity_values_and_terms() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "doc-a", &["a1"]);

        insert_entities(
            &mut conn,
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

    /// Regression test for the JSON-hang root cause. Bulk inserts MUST run
    /// inside a single transaction; without it, a 5000-hit batch takes many
    /// seconds even in-memory because of per-row implicit commits. With the
    /// transaction wrapper this completes in well under 200ms on any modern
    /// machine. We use a deliberately loose 5s ceiling so the test passes on
    /// slow CI while still failing catastrophically if someone accidentally
    /// reverts the transaction wrapper.
    #[test]
    fn bulk_insert_completes_under_loose_time_budget() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed(&conn, "doc-a", &["a1"]);

        let hits: Vec<EntityHit> = (0..5000)
            .map(|i| EntityHit {
                chunk_id: "a1".to_string(),
                entity_type: "PROPER".to_string(),
                value: format!("Entity{i:05}"),
                confidence: 0.7,
            })
            .collect();

        let start = std::time::Instant::now();
        insert_entities(&mut conn, &hits).unwrap();
        let elapsed = start.elapsed();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 5000);
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "insert_entities of 5K hits took {:?} — transaction wrapper likely regressed",
            elapsed
        );
    }
}
