use rusqlite::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::EngineError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub anchor: Option<String>,
    pub src_span: Option<String>,
    pub dst_span: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub src_chunk: String,
    pub dst_chunk: String,
    pub weight: f32,
    pub edge_type: String,
    /// Short, structured tag explaining why this edge exists. Examples:
    /// `anchor:VID-APPROVAL-005`, `proper:Atlas`, `cos:0.71`, `same_doc`,
    /// `manifest:INC-2026-ATLAS-014`. `None` for legacy edges written
    /// before the schema added the column.
    #[serde(default, rename = "edge_reason")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
}

impl GraphEdge {
    /// Canonicalize endpoints so (A, B) and (B, A) collapse to one PK.
    /// Kept for callers that don't supply a reason (legacy / tests).
    pub fn canonical(src: &str, dst: &str, weight: f32, edge_type: &str) -> Self {
        Self::canonical_with_reason(src, dst, weight, edge_type, None)
    }

    /// Same as [`canonical`] but records the human-readable reason this
    /// edge was created (e.g. the literal shared anchor) so consumers can
    /// cite concrete evidence.
    pub fn canonical_with_reason(
        src: &str,
        dst: &str,
        weight: f32,
        edge_type: &str,
        reason: Option<String>,
    ) -> Self {
        if src <= dst {
            Self {
                src_chunk: src.to_string(),
                dst_chunk: dst.to_string(),
                weight,
                edge_type: edge_type.to_string(),
                reason,
                evidence: None,
            }
        } else {
            Self {
                src_chunk: dst.to_string(),
                dst_chunk: src.to_string(),
                weight,
                edge_type: edge_type.to_string(),
                reason,
                evidence: None,
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNeighbor {
    pub chunk_id: String,
    pub doc_id: String,
    pub content: String,
    pub filename: String,
    pub page: Option<u32>,
    #[serde(default = "default_chunk_signal_str")]
    pub chunk_signal: String,
    pub score: f32,
    pub score_bm25: f32,
    pub score_vec: f32,
    pub score_graph: f32,
    pub score_entity: f32,
    pub score_centrality: f32,
    pub edge_type: String,
    pub edge_reason: Option<String>,
    pub evidence: Option<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewNode {
    pub chunk_id: String,
    pub doc_id: String,
    pub content: String,
    pub filename: String,
    pub page: Option<u32>,
    pub degree: u32,
    /// `content` for primary documents, `reference` for manifest/README/index
    /// files. Lets the UI render reference docs in a distinct style so users
    /// understand why a high-degree node isn't dominating Q&A (it's been
    /// down-ranked in the search blend and excluded from relation evidence).
    /// Defaults to `content` for legacy rows written before doc_class existed.
    #[serde(default = "default_doc_class_str")]
    pub doc_class: String,
    #[serde(default = "default_chunk_signal_str")]
    pub chunk_signal: String,
}

fn default_doc_class_str() -> String {
    "content".to_string()
}

fn default_chunk_signal_str() -> String {
    "content".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOverview {
    pub nodes: Vec<OverviewNode>,
    pub edges: Vec<GraphEdge>,
}

/// Upsert edges without wiping the entire table.
/// Cleanup of stale edges happens automatically via FK CASCADE when chunks
/// are deleted in `chunks::replace_doc_chunks`.
pub fn upsert_edges(conn: &mut Connection, edges: &[GraphEdge]) -> Result<(), EngineError> {
    let tx = conn.transaction()?;
    for edge in edges {
        tx.execute(
            r#"
            INSERT INTO graph_edges (src_chunk, dst_chunk, weight, edge_type, reason)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(src_chunk, dst_chunk) DO UPDATE SET
                weight = MAX(graph_edges.weight, excluded.weight),
                edge_type = CASE
                    WHEN excluded.weight > graph_edges.weight THEN excluded.edge_type
                    ELSE graph_edges.edge_type
                END,
                reason = CASE
                    WHEN excluded.weight >= graph_edges.weight THEN excluded.reason
                    ELSE graph_edges.reason
                END
            "#,
            params![
                edge.src_chunk,
                edge.dst_chunk,
                edge.weight,
                edge.edge_type,
                edge.reason,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn edge_count_by_type(conn: &Connection) -> Result<HashMap<String, i64>, EngineError> {
    let mut stmt =
        conn.prepare("SELECT edge_type, COUNT(*) FROM graph_edges GROUP BY edge_type")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (kind, count) = row?;
        out.insert(kind, count);
    }
    Ok(out)
}

pub fn chunk_neighbors(
    conn: &Connection,
    chunk_id: &str,
    limit: usize,
) -> Result<Vec<GraphNeighbor>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT c.id, c.doc_id, c.content, d.filename, c.page, c.chunk_signal, e.weight,
               e.edge_type, e.reason, origin.content
        FROM graph_edges e
        JOIN chunks origin ON origin.id = ?1
        JOIN chunks c ON c.id = CASE
            WHEN e.src_chunk = ?1 THEN e.dst_chunk
            ELSE e.src_chunk
        END
        JOIN documents d ON d.id = c.doc_id
        WHERE e.src_chunk = ?1 OR e.dst_chunk = ?1
        ORDER BY e.weight DESC
        LIMIT ?2
        "#,
    )?;
    let rows = stmt.query_map(params![chunk_id, limit as i64], |row| {
        let weight = row.get::<_, f64>(6)? as f32;
        let edge_type: String = row.get(7)?;
        let edge_reason: Option<String> = row.get(8)?;
        let origin_content: String = row.get(9)?;
        let neighbor_content: String = row.get(2)?;
        Ok(GraphNeighbor {
            chunk_id: row.get(0)?,
            doc_id: row.get(1)?,
            content: neighbor_content.clone(),
            filename: row.get(3)?,
            page: row.get::<_, Option<i64>>(4)?.map(|page| page as u32),
            chunk_signal: row.get(5)?,
            score: weight,
            score_bm25: 0.0,
            score_vec: 0.0,
            score_graph: weight,
            score_entity: 0.0,
            score_centrality: 0.0,
            edge_type: edge_type.clone(),
            edge_reason: edge_reason.clone(),
            evidence: evidence_for_edge(
                &edge_type,
                edge_reason.as_deref(),
                &origin_content,
                &neighbor_content,
            ),
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

/// Multi-seed BFS — explores the graph starting from EVERY chunk_id in
/// `seed_chunk_ids` simultaneously. Used by the search-result visualization
/// so all top-N hits and their shared neighborhood show up as one constellation.
pub fn graph_search_neighborhood(
    conn: &Connection,
    seed_chunk_ids: &[String],
    depth: usize,
    node_limit: usize,
) -> Result<GraphOverview, EngineError> {
    let max_depth = depth.clamp(0, 3);
    let max_nodes = node_limit.clamp(1, 500);
    let mut visited: HashMap<String, usize> = HashMap::new();
    let mut queue = VecDeque::new();

    for id in seed_chunk_ids {
        if visited.insert(id.clone(), 0).is_none() {
            queue.push_back((id.clone(), 0));
        }
    }

    if max_depth > 0 {
        while let Some((current, depth_here)) = queue.pop_front() {
            if depth_here >= max_depth || visited.len() >= max_nodes {
                continue;
            }
            let mut edge_stmt = conn.prepare(
                r#"
                SELECT src_chunk, dst_chunk
                FROM graph_edges
                WHERE src_chunk = ?1 OR dst_chunk = ?1
                ORDER BY weight DESC
                "#,
            )?;
            let rows = edge_stmt.query_map(params![current], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (src, dst) = row?;
                let next = if src == current { dst } else { src };
                if visited.contains_key(&next) {
                    continue;
                }
                visited.insert(next.clone(), depth_here + 1);
                queue.push_back((next, depth_here + 1));
                if visited.len() >= max_nodes {
                    break;
                }
            }
        }
    }

    let nodes = nodes_for_ids(conn, &visited)?;
    if nodes.is_empty() {
        return Ok(GraphOverview {
            nodes,
            edges: vec![],
        });
    }

    let visible_ids: HashSet<&str> = nodes.iter().map(|node| node.chunk_id.as_str()).collect();
    let edges = edges_between_ids(conn, &visible_ids)?;

    Ok(GraphOverview { nodes, edges })
}

pub fn graph_overview(conn: &Connection, node_limit: usize) -> Result<GraphOverview, EngineError> {
    let mut node_stmt = conn.prepare(
        r#"
        SELECT c.id, c.doc_id, c.content, d.filename, c.page,
               (SELECT COUNT(*) FROM graph_edges e
                WHERE e.src_chunk = c.id OR e.dst_chunk = c.id) AS degree,
               d.doc_class, c.chunk_signal
        FROM chunks c
        JOIN documents d ON d.id = c.doc_id
        WHERE c.chunk_signal = 'content'
        ORDER BY degree DESC, c.id
        LIMIT ?1
        "#,
    )?;
    let nodes: Vec<OverviewNode> = node_stmt
        .query_map(params![node_limit as i64], |row| {
            Ok(OverviewNode {
                chunk_id: row.get(0)?,
                doc_id: row.get(1)?,
                content: row.get(2)?,
                filename: row.get(3)?,
                page: row.get::<_, Option<i64>>(4)?.map(|page| page as u32),
                degree: row.get::<_, i64>(5)? as u32,
                doc_class: row.get(6)?,
                chunk_signal: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if nodes.is_empty() {
        return Ok(GraphOverview {
            nodes,
            edges: vec![],
        });
    }

    let ids: Vec<String> = nodes.iter().map(|n| n.chunk_id.clone()).collect();
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT e.src_chunk, e.dst_chunk, e.weight, e.edge_type, e.reason, sc.content, dc.content
         FROM graph_edges e
         JOIN chunks sc ON sc.id = e.src_chunk
         JOIN chunks dc ON dc.id = e.dst_chunk
         WHERE e.src_chunk IN ({0}) AND e.dst_chunk IN ({0})",
        placeholders
    );
    let mut edge_stmt = conn.prepare(&sql)?;
    let edges: Vec<GraphEdge> = edge_stmt
        .query_map(
            params_from_iter(ids.iter().chain(ids.iter()).map(|s| s.as_str())),
            |row| {
                let weight: f64 = row.get(2)?;
                Ok(GraphEdge {
                    src_chunk: row.get(0)?,
                    dst_chunk: row.get(1)?,
                    weight: weight as f32,
                    edge_type: row.get(3)?,
                    reason: row.get(4)?,
                    evidence: {
                        let edge_type: String = row.get(3)?;
                        let reason: Option<String> = row.get(4)?;
                        let src_content: String = row.get(5)?;
                        let dst_content: String = row.get(6)?;
                        evidence_for_edge(&edge_type, reason.as_deref(), &src_content, &dst_content)
                    },
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GraphOverview { nodes, edges })
}

pub fn graph_neighborhood(
    conn: &Connection,
    chunk_id: &str,
    depth: usize,
    node_limit: usize,
) -> Result<GraphOverview, EngineError> {
    let max_depth = depth.clamp(1, 4);
    let max_nodes = node_limit.clamp(1, 500);
    let mut visited: HashMap<String, usize> = HashMap::new();
    let mut queue = VecDeque::new();

    visited.insert(chunk_id.to_string(), 0);
    queue.push_back((chunk_id.to_string(), 0));

    while let Some((current_id, current_depth)) = queue.pop_front() {
        if current_depth >= max_depth || visited.len() >= max_nodes {
            continue;
        }

        let mut edge_stmt = conn.prepare(
            r#"
            SELECT src_chunk, dst_chunk
            FROM graph_edges
            WHERE src_chunk = ?1 OR dst_chunk = ?1
            ORDER BY weight DESC
            "#,
        )?;
        let rows = edge_stmt.query_map(params![current_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (src, dst) = row?;
            let next_id = if src == current_id { dst } else { src };
            if visited.contains_key(&next_id) {
                continue;
            }

            visited.insert(next_id.clone(), current_depth + 1);
            queue.push_back((next_id, current_depth + 1));

            if visited.len() >= max_nodes {
                break;
            }
        }
    }

    let nodes = nodes_for_ids(conn, &visited)?;
    if nodes.is_empty() {
        return Ok(GraphOverview {
            nodes,
            edges: vec![],
        });
    }

    let visible_ids: HashSet<&str> = nodes.iter().map(|node| node.chunk_id.as_str()).collect();
    let edges = edges_between_ids(conn, &visible_ids)?;

    Ok(GraphOverview { nodes, edges })
}

fn nodes_for_ids(
    conn: &Connection,
    visited: &HashMap<String, usize>,
) -> Result<Vec<OverviewNode>, EngineError> {
    let ids: Vec<&str> = visited.keys().map(String::as_str).collect();
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"
        SELECT c.id, c.doc_id, c.content, d.filename, c.page,
               (SELECT COUNT(*) FROM graph_edges e
                WHERE e.src_chunk = c.id OR e.dst_chunk = c.id) AS degree,
               d.doc_class, c.chunk_signal
        FROM chunks c
        JOIN documents d ON d.id = c.doc_id
        WHERE c.id IN ({})
        "#,
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut nodes = stmt
        .query_map(params_from_iter(ids), |row| {
            Ok(OverviewNode {
                chunk_id: row.get(0)?,
                doc_id: row.get(1)?,
                content: row.get(2)?,
                filename: row.get(3)?,
                page: row.get::<_, Option<i64>>(4)?.map(|page| page as u32),
                degree: row.get::<_, i64>(5)? as u32,
                doc_class: row.get(6)?,
                chunk_signal: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    nodes.sort_by(|left, right| {
        let left_depth = visited.get(&left.chunk_id).copied().unwrap_or(usize::MAX);
        let right_depth = visited.get(&right.chunk_id).copied().unwrap_or(usize::MAX);
        left_depth
            .cmp(&right_depth)
            .then_with(|| right.degree.cmp(&left.degree))
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    Ok(nodes)
}

fn edges_between_ids(
    conn: &Connection,
    ids: &HashSet<&str>,
) -> Result<Vec<GraphEdge>, EngineError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let ordered_ids: Vec<&str> = ids.iter().copied().collect();
    let placeholders = std::iter::repeat("?")
        .take(ordered_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT e.src_chunk, e.dst_chunk, e.weight, e.edge_type, e.reason, sc.content, dc.content
         FROM graph_edges e
         JOIN chunks sc ON sc.id = e.src_chunk
         JOIN chunks dc ON dc.id = e.dst_chunk
         WHERE e.src_chunk IN ({0}) AND e.dst_chunk IN ({0})
         ORDER BY e.weight DESC",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let edges = stmt
        .query_map(
            params_from_iter(ordered_ids.iter().chain(ordered_ids.iter()).map(|id| *id)),
            |row| {
                let weight: f64 = row.get(2)?;
                Ok(GraphEdge {
                    src_chunk: row.get(0)?,
                    dst_chunk: row.get(1)?,
                    weight: weight as f32,
                    edge_type: row.get(3)?,
                    reason: row.get(4)?,
                    evidence: {
                        let edge_type: String = row.get(3)?;
                        let reason: Option<String> = row.get(4)?;
                        let src_content: String = row.get(5)?;
                        let dst_content: String = row.get(6)?;
                        evidence_for_edge(&edge_type, reason.as_deref(), &src_content, &dst_content)
                    },
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(edges)
}

fn evidence_for_edge(
    edge_type: &str,
    reason: Option<&str>,
    src_content: &str,
    dst_content: &str,
) -> Option<Evidence> {
    if edge_type == "semantic" {
        return Some(Evidence {
            kind: "semantic".to_string(),
            anchor: None,
            src_span: None,
            dst_span: None,
        });
    }

    let (kind, prefix) = match edge_type {
        "shared_anchor" => ("shared_anchor", "anchor:"),
        "shared_entity" => {
            let reason = reason?;
            let split_at = reason.find(':')?;
            let literal = &reason[split_at + 1..];
            return Some(Evidence {
                kind: "shared_entity".to_string(),
                anchor: Some(literal.to_string()),
                src_span: snippet_around(src_content, literal),
                dst_span: snippet_around(dst_content, literal),
            });
        }
        "manifest_overlap" => ("manifest", "manifest:"),
        _ => return None,
    };

    let literal = reason?.strip_prefix(prefix)?;
    Some(Evidence {
        kind: kind.to_string(),
        anchor: Some(literal.to_string()),
        src_span: snippet_around(src_content, literal),
        dst_span: snippet_around(dst_content, literal),
    })
}

fn snippet_around(text: &str, literal: &str) -> Option<String> {
    let start = find_case_insensitive(text, literal)?;
    let end = start
        + text[start..]
            .chars()
            .take(literal.chars().count())
            .map(char::len_utf8)
            .sum::<usize>();
    let snippet_start = char_boundary_before(text, start.saturating_sub(40));
    let snippet_end = char_boundary_after(text, (end + 40).min(text.len()));
    Some(text[snippet_start..snippet_end].to_string())
}

fn find_case_insensitive(text: &str, literal: &str) -> Option<usize> {
    if literal.is_empty() {
        return None;
    }
    if let Some(idx) = text.find(literal) {
        return Some(idx);
    }
    let needle = literal.to_lowercase();
    text.char_indices()
        .find(|(idx, _)| text[*idx..].to_lowercase().starts_with(&needle))
        .map(|(idx, _)| idx)
}

fn char_boundary_before(text: &str, mut idx: usize) -> usize {
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn char_boundary_after(text: &str, mut idx: usize) -> usize {
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::migrate;

    fn seed_doc_with_chunks(conn: &rusqlite::Connection, doc_id: &str, chunk_ids: &[&str]) {
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
    fn upsert_does_not_wipe_other_docs_edges() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_doc_with_chunks(&conn, "doc-a", &["a1", "a2"]);
        seed_doc_with_chunks(&conn, "doc-b", &["b1", "b2"]);

        let a_edges = vec![GraphEdge::canonical("a1", "a2", 0.5, "same_doc")];
        upsert_edges(&mut conn, &a_edges).unwrap();
        let b_edges = vec![GraphEdge::canonical("b1", "b2", 0.5, "same_doc")];
        upsert_edges(&mut conn, &b_edges).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2, "both docs' edges must persist");
    }

    #[test]
    fn canonical_orders_endpoints() {
        let e = GraphEdge::canonical("z", "a", 0.5, "same_doc");
        assert_eq!(e.src_chunk, "a");
        assert_eq!(e.dst_chunk, "z");
    }

    #[test]
    fn upsert_keeps_max_weight_on_conflict() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_doc_with_chunks(&conn, "doc-a", &["a1", "a2"]);

        upsert_edges(
            &mut conn,
            &[GraphEdge::canonical("a1", "a2", 0.5, "same_doc")],
        )
        .unwrap();
        upsert_edges(
            &mut conn,
            &[GraphEdge::canonical("a1", "a2", 0.8, "semantic")],
        )
        .unwrap();

        let (weight, kind): (f64, String) = conn
            .query_row(
                "SELECT weight, edge_type FROM graph_edges WHERE src_chunk='a1' AND dst_chunk='a2'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!((weight - 0.8).abs() < 1e-6);
        assert_eq!(kind, "semantic");
    }

    #[test]
    fn upsert_round_trips_edge_reason() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_doc_with_chunks(&conn, "doc-a", &["a1", "a2"]);

        upsert_edges(
            &mut conn,
            &[GraphEdge::canonical_with_reason(
                "a1",
                "a2",
                0.9,
                "shared_anchor",
                Some("anchor:VID-APPROVAL-005".to_string()),
            )],
        )
        .unwrap();

        let edge = edges_between_ids(&conn, &HashSet::from(["a1", "a2"]))
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        assert_eq!(edge.reason.as_deref(), Some("anchor:VID-APPROVAL-005"));
    }

    #[test]
    fn graph_neighborhood_returns_anchor_evidence_from_reason() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status) VALUES ('doc-a', 'a.md', 'a.md', 'md', 1, 'h', '2026-05-21T00:00:00Z', 'indexed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status) VALUES ('doc-b', 'b.md', 'b.md', 'md', 1, 'h', '2026-05-21T00:00:00Z', 'indexed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, created_at) VALUES ('a1', 'doc-a', 0, 'Approval VID-APPROVAL-005 is ready.', 0, 35, '2026-05-21T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, created_at) VALUES ('b1', 'doc-b', 0, 'Follow up on VID-APPROVAL-005 tomorrow.', 0, 41, '2026-05-21T00:00:00Z')",
            [],
        )
        .unwrap();
        upsert_edges(
            &mut conn,
            &[GraphEdge::canonical_with_reason(
                "a1",
                "b1",
                0.9,
                "shared_anchor",
                Some("anchor:VID-APPROVAL-005".to_string()),
            )],
        )
        .unwrap();

        let overview = graph_neighborhood(&conn, "a1", 1, 10).unwrap();
        let edge = overview.edges.first().expect("edge");
        let evidence = edge.evidence.as_ref().expect("evidence");

        assert_eq!(edge.reason.as_deref(), Some("anchor:VID-APPROVAL-005"));
        assert_eq!(evidence.kind, "shared_anchor");
        assert_eq!(evidence.anchor.as_deref(), Some("VID-APPROVAL-005"));
        assert!(evidence
            .src_span
            .as_deref()
            .unwrap()
            .contains("VID-APPROVAL-005"));
        assert!(evidence
            .dst_span
            .as_deref()
            .unwrap()
            .contains("VID-APPROVAL-005"));
    }

    #[test]
    fn semantic_evidence_has_no_literal_spans() {
        let evidence =
            evidence_for_edge("semantic", Some("cos:0.71"), "alpha text", "similar text")
                .expect("semantic evidence");

        assert_eq!(evidence.kind, "semantic");
        assert!(evidence.anchor.is_none());
        assert!(evidence.src_span.is_none());
        assert!(evidence.dst_span.is_none());
    }

    #[test]
    fn graph_neighborhood_respects_relation_depth() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_doc_with_chunks(&conn, "doc-a", &["a1", "a2", "a3", "a4"]);

        upsert_edges(
            &mut conn,
            &[
                GraphEdge::canonical("a1", "a2", 0.9, "semantic"),
                GraphEdge::canonical("a2", "a3", 0.8, "semantic"),
                GraphEdge::canonical("a3", "a4", 0.7, "semantic"),
            ],
        )
        .unwrap();

        let one_hop = graph_neighborhood(&conn, "a1", 1, 10).unwrap();
        let one_hop_ids: Vec<String> = one_hop
            .nodes
            .iter()
            .map(|node| node.chunk_id.clone())
            .collect();
        assert_eq!(one_hop_ids, vec!["a1".to_string(), "a2".to_string()]);
        assert_eq!(one_hop.edges.len(), 1);

        let two_hops = graph_neighborhood(&conn, "a1", 2, 10).unwrap();
        let two_hop_ids: Vec<String> = two_hops
            .nodes
            .iter()
            .map(|node| node.chunk_id.clone())
            .collect();
        assert_eq!(
            two_hop_ids,
            vec!["a1".to_string(), "a2".to_string(), "a3".to_string()]
        );
        assert_eq!(two_hops.edges.len(), 2);
    }

    #[test]
    fn graph_neighborhood_returns_edges_between_visible_nodes() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_doc_with_chunks(&conn, "doc-a", &["a1", "a2", "a3"]);

        upsert_edges(
            &mut conn,
            &[
                GraphEdge::canonical("a1", "a2", 0.9, "semantic"),
                GraphEdge::canonical("a1", "a3", 0.8, "semantic"),
                GraphEdge::canonical("a2", "a3", 0.7, "shared_entity"),
            ],
        )
        .unwrap();

        let neighborhood = graph_neighborhood(&conn, "a1", 1, 10).unwrap();
        assert_eq!(neighborhood.nodes.len(), 3);
        assert_eq!(
            neighborhood.edges.len(),
            3,
            "focused graph should include every edge among visible nodes"
        );
    }

    #[test]
    fn graph_overview_excludes_low_signal_chunks() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_doc_with_chunks(&conn, "doc-a", &["a1", "a2"]);
        conn.execute(
            "UPDATE chunks SET chunk_signal = 'anchor_list' WHERE id = 'a1'",
            [],
        )
        .unwrap();

        upsert_edges(
            &mut conn,
            &[GraphEdge::canonical("a1", "a2", 0.9, "shared_anchor")],
        )
        .unwrap();

        let overview = graph_overview(&conn, 10).unwrap();
        let ids: Vec<String> = overview
            .nodes
            .into_iter()
            .map(|node| node.chunk_id)
            .collect();

        assert_eq!(ids, vec!["a2".to_string()]);
    }
}
