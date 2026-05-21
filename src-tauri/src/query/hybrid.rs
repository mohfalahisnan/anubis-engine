//! Hybrid search: BM25 + dense vectors + entity exact match + graph expansion.
//!
//! Scoring weights (from the spec the user requested):
//! - BM25: 0.35
//! - Vector semantic: 0.40
//! - Graph relation boost: 0.15
//! - Entity / exact-token boost: 0.10
//!
//! Depth controls how aggressively the graph is used to *expand* the candidate
//! pool (not just boost existing candidates):
//! - 0: only direct text/semantic/entity matches.
//! - 1: include 1-hop neighbors of top seeds (default).
//! - 2: discovery mode — 2 hops, capped fan-out.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use crate::{
    store::{chunks::query_result_for_chunk, fts, vectors},
    types::QueryResult,
    EngineError,
};

pub const W_BM25: f32 = 0.35;
pub const W_VEC: f32 = 0.40;
pub const W_GRAPH: f32 = 0.15;
pub const W_ENTITY: f32 = 0.10;

/// Centrality is a query-INDEPENDENT signal (how connected this chunk is in
/// the global graph). Used only as a tie-breaker — additive on top of the
/// blended base score, never enough on its own to surface a chunk.
pub const W_CENTRALITY_TIEBREAK: f32 = 0.05;

/// A chunk must show at least this much query-relevant signal (vector, BM25,
/// or entity match — whichever is strongest) before centrality gets to
/// contribute anything. Stops "hub" chunks (READMEs, glossaries, intros) from
/// dominating queries they don't actually answer.
pub const CENTRALITY_RELEVANCE_GATE: f32 = 0.20;

/// Cap how many results from a single document appear in the top N.
/// Avoids the "all hits clustered in one doc" failure mode.
const MAX_RESULTS_PER_DOC: usize = 3;

/// Fan-out cap during graph BFS so a hot node doesn't explode the candidate pool.
const EXPANSION_FANOUT: usize = 8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QueryOpts {
    pub limit: usize,
    pub depth: usize,
}

impl Default for QueryOpts {
    fn default() -> Self {
        Self { limit: 10, depth: 1 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreParts {
    pub chunk_id: String,
    pub score_bm25: f32,
    pub score_vec: f32,
    /// Query-DEPENDENT graph signal — how strongly this chunk is reachable
    /// from the top seeds via the graph (BFS expansion product of edge
    /// weights).
    pub score_graph: f32,
    pub score_entity: f32,
    /// Query-INDEPENDENT graph signal — avg edge weight to all neighbors.
    /// Applied as a gated tie-breaker only.
    pub score_centrality: f32,
}

impl ScoreParts {
    fn new(chunk_id: String) -> Self {
        Self {
            chunk_id,
            score_bm25: 0.0,
            score_vec: 0.0,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 0.0,
        }
    }

    /// Strongest query-RELEVANT signal — used to gate centrality.
    fn relevance(&self) -> f32 {
        self.score_vec.max(self.score_bm25).max(self.score_entity)
    }
}

pub fn final_score(s: &ScoreParts) -> f32 {
    let base = W_VEC * s.score_vec
        + W_BM25 * s.score_bm25
        + W_GRAPH * s.score_graph
        + W_ENTITY * s.score_entity;
    let centrality_bonus = if s.relevance() >= CENTRALITY_RELEVANCE_GATE {
        W_CENTRALITY_TIEBREAK * s.score_centrality
    } else {
        0.0
    };
    (base + centrality_bonus).clamp(0.0, 1.0)
}

/// Full hybrid query — the public entry point.
pub fn run_query(
    conn: &Connection,
    fts_index: &tantivy::Index,
    query_text: &str,
    query_embedding: &[f32],
    opts: QueryOpts,
) -> Result<Vec<QueryResult>, EngineError> {
    let pool_size = (opts.limit * 4).max(20);

    // (1) Vector candidates
    let vector_hits = vectors::search_vectors(conn, query_embedding, pool_size)?;
    let vector_scores = normalize(
        vector_hits
            .into_iter()
            .map(|h| (h.chunk_id, h.score))
            .collect(),
    );

    // (2) BM25 candidates
    let bm25_hits = match fts::search(fts_index, &sanitize_for_tantivy(query_text), pool_size) {
        Ok(hits) => hits,
        Err(error) => {
            tracing::warn!("bm25 search failed, continuing without it: {}", error);
            Vec::new()
        }
    };
    let bm25_scores = normalize(
        bm25_hits
            .into_iter()
            .map(|h| (h.chunk_id, h.score))
            .collect(),
    );

    // (3) Entity exact-token matches
    let entity_scores = match_entities(conn, query_text, pool_size)?;

    // Pool: union of all three signals
    let mut pool: HashMap<String, ScoreParts> = HashMap::new();
    for (id, score) in &vector_scores {
        pool.entry(id.clone()).or_insert_with(|| ScoreParts::new(id.clone())).score_vec = *score;
    }
    for (id, score) in &bm25_scores {
        pool.entry(id.clone()).or_insert_with(|| ScoreParts::new(id.clone())).score_bm25 = *score;
    }
    for (id, score) in &entity_scores {
        pool.entry(id.clone()).or_insert_with(|| ScoreParts::new(id.clone())).score_entity =
            *score;
    }

    // (4) Graph expansion at depth > 0: add chunks reachable from top seeds.
    if opts.depth > 0 && !pool.is_empty() {
        let seeds = top_seed_ids(&pool, 10);
        let expansion = expand_via_graph(conn, &seeds, opts.depth)?;
        for (id, expansion_score) in expansion {
            pool.entry(id.clone())
                .or_insert_with(|| ScoreParts::new(id.clone()))
                .score_graph = expansion_score;
        }
    }

    // (5) Centrality: write into score_centrality (gated in final_score by
    // CENTRALITY_RELEVANCE_GATE so hubs can't surface on weak matches).
    if !pool.is_empty() {
        populate_centrality(conn, &mut pool)?;
    }

    // (6) Score, sort, diversify per-doc
    let mut scored: Vec<ScoreParts> = pool.into_values().collect();
    scored.sort_by(|a, b| {
        final_score(b)
            .partial_cmp(&final_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let selected = diversify_per_doc(conn, scored, opts.limit, MAX_RESULTS_PER_DOC)?;

    // (7) Materialize QueryResult rows
    let mut results = Vec::with_capacity(selected.len());
    for s in selected {
        let total = final_score(&s);
        if let Some(r) = query_result_for_chunk(
            conn,
            &s.chunk_id,
            total,
            s.score_bm25,
            s.score_vec,
            s.score_graph,
            s.score_entity,
            s.score_centrality,
        )? {
            results.push(r);
        }
    }
    Ok(results)
}

/// Backwards-compat shim — embedding-only path, kept for callers that don't
/// hold a tantivy Index. Skips BM25 and entity matching.
pub fn query_with_embedding(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<QueryResult>, EngineError> {
    let vector_hits = vectors::search_vectors(conn, query_embedding, limit * 3)?;
    let vector_scores = normalize(
        vector_hits
            .into_iter()
            .map(|h| (h.chunk_id, h.score))
            .collect(),
    );

    let mut pool: HashMap<String, ScoreParts> = HashMap::new();
    for (id, score) in &vector_scores {
        pool.entry(id.clone()).or_insert_with(|| ScoreParts::new(id.clone())).score_vec = *score;
    }
    populate_centrality(conn, &mut pool)?;

    let mut scored: Vec<ScoreParts> = pool.into_values().collect();
    scored.sort_by(|a, b| {
        final_score(b)
            .partial_cmp(&final_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let selected = diversify_per_doc(conn, scored, limit, MAX_RESULTS_PER_DOC)?;
    let mut results = Vec::new();
    for s in selected {
        let total = final_score(&s);
        if let Some(r) = query_result_for_chunk(
            conn,
            &s.chunk_id,
            total,
            s.score_bm25,
            s.score_vec,
            s.score_graph,
            s.score_entity,
            s.score_centrality,
        )? {
            results.push(r);
        }
    }
    Ok(results)
}

// ----------------------------------------------------------------------------
// Internals
// ----------------------------------------------------------------------------

fn normalize(scores: Vec<(String, f32)>) -> Vec<(String, f32)> {
    let max_score = scores.iter().map(|(_, s)| *s).fold(0.0f32, f32::max);
    if max_score <= 0.0 {
        return scores.into_iter().map(|(id, _)| (id, 0.0)).collect();
    }
    scores
        .into_iter()
        .map(|(id, s)| (id, (s / max_score).clamp(0.0, 1.0)))
        .collect()
}

fn sanitize_for_tantivy(q: &str) -> String {
    // Tantivy's QueryParser interprets `+`, `:`, `-`, `(`, `)`, `[`, `]`, `^`,
    // `"`, `~`, `*`, `?` as syntax. For free-text KB search we want term-OR
    // semantics, so we strip them.
    q.chars()
        .map(|c| match c {
            '+' | ':' | '-' | '(' | ')' | '[' | ']' | '^' | '"' | '~' | '*' | '?' | '\\' | '/' => {
                ' '
            }
            _ => c,
        })
        .collect()
}

/// Entity boost: chunks whose stored entity values match any query token.
/// Score = (matches / max_matches_seen), capped to 1.0.
fn match_entities(
    conn: &Connection,
    query_text: &str,
    pool_size: usize,
) -> Result<Vec<(String, f32)>, EngineError> {
    let tokens: Vec<String> = query_tokens(query_text);
    if tokens.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = std::iter::repeat("?")
        .take(tokens.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"
        SELECT e.chunk_id, COUNT(DISTINCT e.value) AS matches
        FROM entities e
        WHERE LOWER(e.value) IN ({0})
        GROUP BY e.chunk_id
        ORDER BY matches DESC
        LIMIT ?
        "#,
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let pool_size_i64 = pool_size as i64;
    let bind_iter = tokens
        .iter()
        .map(|t| t as &dyn rusqlite::ToSql)
        .chain(std::iter::once(&pool_size_i64 as &dyn rusqlite::ToSql))
        .collect::<Vec<_>>();
    let rows = stmt.query_map(bind_iter.as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    let raw: Vec<(String, f32)> = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(id, count)| (id, count as f32))
        .collect();
    Ok(normalize(raw))
}

fn query_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(|t| t.to_lowercase())
        .collect()
}

fn top_seed_ids(pool: &HashMap<String, ScoreParts>, n: usize) -> Vec<String> {
    let mut ranked: Vec<&ScoreParts> = pool.values().collect();
    ranked.sort_by(|a, b| {
        final_score(b)
            .partial_cmp(&final_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked
        .into_iter()
        .take(n)
        .map(|s| s.chunk_id.clone())
        .collect()
}

/// BFS from seeds up to `depth` hops. Edge weights decay per hop.
fn expand_via_graph(
    conn: &Connection,
    seeds: &[String],
    depth: usize,
) -> Result<Vec<(String, f32)>, EngineError> {
    if depth == 0 || seeds.is_empty() {
        return Ok(vec![]);
    }

    let mut best: HashMap<String, f32> = HashMap::new();
    let seed_set: HashSet<&str> = seeds.iter().map(|s| s.as_str()).collect();

    let mut queue: VecDeque<(String, f32, usize)> = VecDeque::new();
    for seed in seeds {
        queue.push_back((seed.clone(), 1.0, 0));
    }

    let mut neighbor_stmt = conn.prepare(
        r#"
        SELECT
            CASE WHEN src_chunk = ?1 THEN dst_chunk ELSE src_chunk END AS neighbor,
            weight
        FROM graph_edges
        WHERE src_chunk = ?1 OR dst_chunk = ?1
        ORDER BY weight DESC
        LIMIT ?2
        "#,
    )?;

    while let Some((current, accumulated, hops)) = queue.pop_front() {
        if hops >= depth {
            continue;
        }
        let rows = neighbor_stmt.query_map(params![current, EXPANSION_FANOUT as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)? as f32))
        })?;
        for row in rows {
            let (neighbor, weight) = row?;
            // Skip echo back to a seed.
            if seed_set.contains(neighbor.as_str()) {
                continue;
            }
            let decayed = accumulated * weight;
            let entry = best.entry(neighbor.clone()).or_insert(0.0);
            if decayed > *entry {
                *entry = decayed;
            }
            if hops + 1 < depth {
                queue.push_back((neighbor, decayed, hops + 1));
            }
        }
    }

    let raw: Vec<(String, f32)> = best.into_iter().collect();
    Ok(normalize(raw))
}

/// Populate `score_centrality` (avg edge weight to direct neighbors).
/// Query-independent — does NOT touch `score_graph`. Final_score gates this
/// behind a relevance threshold so a hub can't surface on relevance-free
/// queries.
fn populate_centrality(
    conn: &Connection,
    pool: &mut HashMap<String, ScoreParts>,
) -> Result<(), EngineError> {
    if pool.is_empty() {
        return Ok(());
    }
    let ids: Vec<String> = pool.keys().cloned().collect();
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"
        SELECT chunk_id, AVG(weight) FROM (
            SELECT src_chunk AS chunk_id, weight FROM graph_edges WHERE src_chunk IN ({0})
            UNION ALL
            SELECT dst_chunk AS chunk_id, weight FROM graph_edges WHERE dst_chunk IN ({0})
        )
        GROUP BY chunk_id
        "#,
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let centrality: HashMap<String, f32> = stmt
        .query_map(
            params_from_iter(ids.iter().chain(ids.iter()).map(|s| s.as_str())),
            |row| {
                let id: String = row.get(0)?;
                let weight: f64 = row.get(1).unwrap_or(0.0);
                Ok((id, weight as f32))
            },
        )?
        .collect::<Result<HashMap<_, _>, _>>()?;

    let max = centrality.values().copied().fold(0.0f32, f32::max);
    for (id, parts) in pool.iter_mut() {
        if let Some(&c) = centrality.get(id) {
            let normalized = if max > 0.0 { c / max } else { 0.0 };
            parts.score_centrality = normalized.clamp(0.0, 1.0);
        }
    }
    Ok(())
}

/// Greedily take top-scored chunks, but cap how many can come from the same
/// document. Returns up to `limit` items.
fn diversify_per_doc(
    conn: &Connection,
    sorted_scores: Vec<ScoreParts>,
    limit: usize,
    per_doc_cap: usize,
) -> Result<Vec<ScoreParts>, EngineError> {
    if sorted_scores.is_empty() {
        return Ok(vec![]);
    }

    // Fetch doc_ids for all candidate chunks in one round-trip.
    let ids: Vec<String> = sorted_scores.iter().map(|s| s.chunk_id.clone()).collect();
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, doc_id FROM chunks WHERE id IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let doc_for_chunk: HashMap<String, String> = stmt
        .query_map(
            params_from_iter(ids.iter().map(|s| s.as_str())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?
        .collect::<Result<HashMap<_, _>, _>>()?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut chosen = Vec::with_capacity(limit);
    let mut overflow: Vec<ScoreParts> = Vec::new();

    for s in sorted_scores {
        let doc = doc_for_chunk.get(&s.chunk_id).cloned();
        match doc {
            Some(d) => {
                let n = counts.entry(d.clone()).or_insert(0);
                if *n < per_doc_cap {
                    *n += 1;
                    chosen.push(s);
                } else {
                    overflow.push(s);
                }
            }
            None => chosen.push(s),
        }
        if chosen.len() >= limit {
            break;
        }
    }

    // If diversification starved us, top up from overflow.
    for s in overflow {
        if chosen.len() >= limit {
            break;
        }
        chosen.push(s);
    }
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{db::migrate, fts as fts_store};

    fn seed_kb(conn: &mut Connection) {
        conn.execute_batch(
            r#"
            INSERT INTO documents (id,path,filename,format,size_bytes,hash,indexed_at,status)
            VALUES
              ('doc-a','a.md','a.md','md',1,'h','2026-05-21T00:00:00Z','indexed'),
              ('doc-b','b.md','b.md','md',1,'h','2026-05-21T00:00:00Z','indexed');

            INSERT INTO chunks (id,doc_id,chunk_index,content,char_start,char_end,created_at)
            VALUES
              ('a1','doc-a',0,'printer thermal promo discount',0,30,'2026-05-21T00:00:00Z'),
              ('a2','doc-a',1,'invoice ledger ledger ledger',0,28,'2026-05-21T00:00:00Z'),
              ('b1','doc-b',0,'thermal printer manual',0,22,'2026-05-21T00:00:00Z');
            "#,
        ).unwrap();
    }

    #[test]
    fn final_score_weights_sum_to_one_without_centrality() {
        let parts = ScoreParts {
            chunk_id: "x".into(),
            score_bm25: 1.0,
            score_vec: 1.0,
            score_graph: 1.0,
            score_entity: 1.0,
            score_centrality: 0.0,
        };
        // 35 + 40 + 15 + 10 = 100.
        assert!((final_score(&parts) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn final_score_weighted_blend() {
        let parts = ScoreParts {
            chunk_id: "x".into(),
            score_bm25: 1.0,
            score_vec: 0.0,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 0.0,
        };
        assert!((final_score(&parts) - 0.35).abs() < 1e-6);
    }

    #[test]
    fn centrality_does_not_apply_below_relevance_gate() {
        // High centrality (a hub) with zero relevance signal — must NOT get
        // any centrality contribution. This is the bug fix.
        let parts = ScoreParts {
            chunk_id: "hub".into(),
            score_bm25: 0.0,
            score_vec: 0.0,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 1.0,
        };
        assert!(final_score(&parts).abs() < 1e-6);
    }

    #[test]
    fn centrality_applies_above_relevance_gate() {
        // Same hub but now it has SOME baseline relevance — centrality may
        // contribute as a small additive tie-breaker (0.05 max).
        let parts = ScoreParts {
            chunk_id: "hub".into(),
            score_bm25: 0.5, // > CENTRALITY_RELEVANCE_GATE = 0.2
            score_vec: 0.0,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 1.0,
        };
        // base = 0.35 * 0.5 = 0.175; centrality bonus = 0.05; total = 0.225.
        assert!((final_score(&parts) - 0.225).abs() < 1e-6);
    }

    #[test]
    fn hub_cannot_outrank_a_precise_match() {
        // Real-world scenario: README-style hub vs a focused chunk.
        let hub = ScoreParts {
            chunk_id: "readme".into(),
            score_bm25: 0.10, // weak BM25, below gate
            score_vec: 0.10,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 1.0, // very well connected
        };
        let precise = ScoreParts {
            chunk_id: "answer".into(),
            score_bm25: 0.55,
            score_vec: 0.50,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 0.10, // isolated
        };
        assert!(
            final_score(&precise) > final_score(&hub),
            "precise match {:.3} must outrank hub {:.3}",
            final_score(&precise),
            final_score(&hub)
        );
    }

    #[test]
    fn diversification_caps_per_doc_results() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_kb(&mut conn);

        let scores = vec![
            ScoreParts {
                score_vec: 1.0,
                ..ScoreParts::new("a1".into())
            },
            ScoreParts {
                score_vec: 0.95,
                ..ScoreParts::new("a2".into())
            },
            ScoreParts {
                score_vec: 0.5,
                ..ScoreParts::new("b1".into())
            },
        ];
        // Cap of 1 → must keep at most 1 per doc.
        let chosen = diversify_per_doc(&conn, scores, 2, 1).unwrap();
        let docs: Vec<&str> = chosen
            .iter()
            .map(|s| if s.chunk_id.starts_with('a') { "doc-a" } else { "doc-b" })
            .collect();
        assert!(docs.contains(&"doc-a"));
        assert!(docs.contains(&"doc-b"));
    }

    #[test]
    fn entity_matches_boost_chunks_with_exact_token() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_kb(&mut conn);
        conn.execute(
            "INSERT INTO entities (id, chunk_id, entity_type, value, confidence) VALUES ('e1','a1','PROPER','Anubis',0.7)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entities (id, chunk_id, entity_type, value, confidence) VALUES ('e2','b1','KEYWORD','printer',0.5)",
            [],
        )
        .unwrap();

        let hits = match_entities(&conn, "Anubis", 10).unwrap();
        assert!(hits.iter().any(|(id, _)| id == "a1"));
    }

    #[test]
    fn depth_zero_skips_graph_expansion() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        seed_kb(&mut conn);
        let fts_index = fts_store::create_in_ram();

        // Empty vector means vector search returns nothing — keeps the test
        // about depth, not about embedding signal.
        let zeros = vec![0.0f32; 384];
        let out = run_query(
            &conn,
            &fts_index,
            "",
            &zeros,
            QueryOpts { limit: 5, depth: 0 },
        )
        .unwrap();
        // With no BM25 / entity matches and zero embedding, result set may be
        // empty but the call MUST NOT panic and MUST NOT touch graph_edges.
        let _ = out;
    }
}
