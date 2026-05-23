use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::{
    store::graph_store::{self, Evidence},
    types::{ChunkSignal, QueryResult},
    EngineError,
};

const DEFAULT_BUDGET_TOKENS: usize = 6_000;
const DEFAULT_LIMIT: usize = 10;
const DEFAULT_DEPTH: usize = 1;
const ITEM_OVERHEAD_TOKENS: usize = 18;
const RELATION_OVERHEAD_TOKENS: usize = 14;
const METADATA_OVERHEAD_TOKENS: usize = 24;
const MIN_CONTENT_ITEMS_BEFORE_LOW_SIGNAL: usize = 3;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ContextPackOpts {
    pub budget_tokens: usize,
    pub limit: usize,
    pub depth: usize,
    pub include_graph: bool,
}

impl Default for ContextPackOpts {
    fn default() -> Self {
        Self {
            budget_tokens: DEFAULT_BUDGET_TOKENS,
            limit: DEFAULT_LIMIT,
            depth: DEFAULT_DEPTH,
            include_graph: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPack {
    pub query: String,
    pub budget: ContextPackBudget,
    pub sources: Vec<ContextPackSource>,
    pub items: Vec<ContextPackItem>,
    pub relations: Vec<ContextPackRelation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPackBudget {
    pub requested_tokens: usize,
    pub estimated_used_tokens: usize,
    pub omitted_items: usize,
    pub omitted_relations: usize,
    pub estimator: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPackSource {
    pub doc_id: String,
    pub filename: String,
    pub path: Option<String>,
    pub doc_class: String,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPackItem {
    pub chunk_id: String,
    pub doc_id: String,
    pub filename: String,
    pub path: Option<String>,
    pub page: Option<u32>,
    pub doc_class: String,
    pub chunk_signal: ChunkSignal,
    pub score: f32,
    pub score_bm25: f32,
    pub score_vec: f32,
    pub score_graph: f32,
    pub score_entity: f32,
    pub score_centrality: f32,
    pub excerpt: String,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPackRelation {
    pub source_chunk_id: String,
    pub target_chunk_id: String,
    pub target_doc_id: String,
    pub target_filename: String,
    pub target_page: Option<u32>,
    pub edge_type: String,
    #[serde(rename = "edge_reason")]
    pub edge_reason: Option<String>,
    pub weight: f32,
    pub evidence: Option<Evidence>,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone)]
struct DocMeta {
    path: Option<String>,
    doc_class: String,
}

pub fn build_context_pack(
    conn: &Connection,
    query: &str,
    results: &[QueryResult],
    opts: ContextPackOpts,
) -> Result<ContextPack, EngineError> {
    let budget_tokens = opts.budget_tokens.max(1);
    let mut used_tokens = estimate_tokens(query) + METADATA_OVERHEAD_TOKENS;
    let mut omitted_items = 0;
    let mut omitted_relations = 0;
    let mut warnings = Vec::new();
    let mut items = Vec::new();
    let mut seen_chunks = HashSet::new();
    let mut seen_excerpt_keys = HashSet::new();

    let direct_limit = opts.limit.max(1);
    let direct_results: Vec<&QueryResult> = results.iter().take(direct_limit).collect();
    let content_results: Vec<&QueryResult> = direct_results
        .iter()
        .copied()
        .filter(|result| !result.chunk_signal.is_low_signal())
        .collect();
    let low_signal_results: Vec<&QueryResult> = direct_results
        .iter()
        .copied()
        .filter(|result| result.chunk_signal.is_low_signal())
        .collect();

    let mut content_items_added = 0;
    for result in &content_results {
        match try_push_item(
            conn,
            result,
            budget_tokens,
            &mut used_tokens,
            &mut items,
            &mut seen_chunks,
            &mut seen_excerpt_keys,
        )? {
            PushOutcome::Added => content_items_added += 1,
            PushOutcome::Omitted => omitted_items += 1,
            PushOutcome::Duplicate => {}
        }
    }

    if content_items_added < MIN_CONTENT_ITEMS_BEFORE_LOW_SIGNAL {
        for result in &low_signal_results {
            match try_push_item(
                conn,
                result,
                budget_tokens,
                &mut used_tokens,
                &mut items,
                &mut seen_chunks,
                &mut seen_excerpt_keys,
            )? {
                PushOutcome::Added => {}
                PushOutcome::Omitted => omitted_items += 1,
                PushOutcome::Duplicate => {}
            }
        }
    } else {
        omitted_items += low_signal_results.len();
    }

    let mut relations = Vec::new();
    if opts.include_graph && !items.is_empty() && opts.depth > 0 {
        omitted_relations += push_graph_relations(
            conn,
            &items,
            opts.depth,
            budget_tokens,
            &mut used_tokens,
            &mut relations,
        )?;
    }

    if results.is_empty() {
        warnings.push("no retrieval results matched the query".to_string());
    } else if items.is_empty() {
        warnings.push(
            "retrieval results were omitted because the context budget was too small".to_string(),
        );
    }

    let sources = build_sources(&items);
    Ok(ContextPack {
        query: query.to_string(),
        budget: ContextPackBudget {
            requested_tokens: budget_tokens,
            estimated_used_tokens: used_tokens,
            omitted_items,
            omitted_relations,
            estimator: "ceil(chars/4)",
        },
        sources,
        items,
        relations,
        warnings,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushOutcome {
    Added,
    Omitted,
    Duplicate,
}

fn try_push_item(
    conn: &Connection,
    result: &QueryResult,
    budget_tokens: usize,
    used_tokens: &mut usize,
    items: &mut Vec<ContextPackItem>,
    seen_chunks: &mut HashSet<String>,
    seen_excerpt_keys: &mut HashSet<String>,
) -> Result<PushOutcome, EngineError> {
    if !seen_chunks.insert(result.chunk_id.clone()) {
        return Ok(PushOutcome::Duplicate);
    }

    let remaining = budget_tokens.saturating_sub(*used_tokens + ITEM_OVERHEAD_TOKENS);
    if remaining == 0 && !items.is_empty() {
        return Ok(PushOutcome::Omitted);
    }

    let excerpt_budget = remaining.max(8);
    let excerpt = trim_to_token_budget(&result.content, excerpt_budget);
    let dedupe_key = excerpt_dedupe_key(&excerpt);
    if !dedupe_key.is_empty() && !seen_excerpt_keys.insert(dedupe_key) {
        return Ok(PushOutcome::Duplicate);
    }

    let excerpt_tokens = estimate_tokens(&excerpt);
    let item_tokens = excerpt_tokens + ITEM_OVERHEAD_TOKENS;
    if *used_tokens + item_tokens > budget_tokens && !items.is_empty() {
        return Ok(PushOutcome::Omitted);
    }

    let meta = doc_meta_for_chunk(conn, &result.chunk_id)?.unwrap_or_else(|| DocMeta {
        path: None,
        doc_class: "content".to_string(),
    });

    *used_tokens += item_tokens;
    items.push(ContextPackItem {
        chunk_id: result.chunk_id.clone(),
        doc_id: result.doc_id.clone(),
        filename: result.filename.clone(),
        path: meta.path,
        page: result.page,
        doc_class: meta.doc_class,
        chunk_signal: result.chunk_signal,
        score: result.score,
        score_bm25: result.score_bm25,
        score_vec: result.score_vec,
        score_graph: result.score_graph,
        score_entity: result.score_entity,
        score_centrality: result.score_centrality,
        excerpt,
        estimated_tokens: excerpt_tokens,
    });
    Ok(PushOutcome::Added)
}

fn push_graph_relations(
    conn: &Connection,
    items: &[ContextPackItem],
    depth: usize,
    budget_tokens: usize,
    used_tokens: &mut usize,
    relations: &mut Vec<ContextPackRelation>,
) -> Result<usize, EngineError> {
    let max_depth = depth.clamp(1, 3);
    let max_relations = (items.len() * 3).max(3);
    let relation_budget = budget_tokens / 5;
    let mut relation_tokens_used = 0;
    let mut omitted = 0;
    let mut seen_edges = HashSet::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    for item in items {
        if visited.insert(item.chunk_id.clone()) {
            queue.push_back((item.chunk_id.clone(), 0usize));
        }
    }

    while let Some((chunk_id, depth_here)) = queue.pop_front() {
        if depth_here >= max_depth || relations.len() >= max_relations {
            continue;
        }

        let neighbors = graph_store::chunk_neighbors(conn, &chunk_id, 25)?;
        for neighbor in neighbors {
            if neighbor.chunk_signal != "content" {
                omitted += 1;
                continue;
            }
            let target_meta =
                doc_meta_for_chunk(conn, &neighbor.chunk_id)?.unwrap_or_else(|| DocMeta {
                    path: None,
                    doc_class: "content".to_string(),
                });
            if target_meta.doc_class == "reference" {
                omitted += 1;
                continue;
            }

            let edge_key = canonical_edge_key(&chunk_id, &neighbor.chunk_id);
            if !seen_edges.insert(edge_key) {
                continue;
            }

            let relation_text = format!(
                "{} {} {} {}",
                neighbor.edge_type,
                neighbor.edge_reason.as_deref().unwrap_or_default(),
                neighbor.filename,
                neighbor
                    .evidence
                    .as_ref()
                    .and_then(|evidence| evidence.anchor.as_deref())
                    .unwrap_or_default()
            );
            let relation_tokens = estimate_tokens(&relation_text) + RELATION_OVERHEAD_TOKENS;
            if relation_tokens_used + relation_tokens > relation_budget
                || *used_tokens + relation_tokens > budget_tokens
                || relations.len() >= max_relations
            {
                omitted += 1;
                continue;
            }

            relation_tokens_used += relation_tokens;
            *used_tokens += relation_tokens;
            relations.push(ContextPackRelation {
                source_chunk_id: chunk_id.clone(),
                target_chunk_id: neighbor.chunk_id.clone(),
                target_doc_id: neighbor.doc_id.clone(),
                target_filename: neighbor.filename.clone(),
                target_page: neighbor.page,
                edge_type: neighbor.edge_type.clone(),
                edge_reason: neighbor.edge_reason.clone(),
                weight: neighbor.score,
                evidence: neighbor.evidence.clone(),
                estimated_tokens: relation_tokens,
            });

            if visited.insert(neighbor.chunk_id.clone()) {
                queue.push_back((neighbor.chunk_id, depth_here + 1));
            }
        }
    }

    Ok(omitted)
}

fn build_sources(items: &[ContextPackItem]) -> Vec<ContextPackSource> {
    let mut ordered_sources: Vec<ContextPackSource> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();

    for item in items {
        if let Some(index) = positions.get(&item.doc_id).copied() {
            ordered_sources[index].item_count += 1;
        } else {
            positions.insert(item.doc_id.clone(), ordered_sources.len());
            ordered_sources.push(ContextPackSource {
                doc_id: item.doc_id.clone(),
                filename: item.filename.clone(),
                path: item.path.clone(),
                doc_class: item.doc_class.clone(),
                item_count: 1,
            });
        }
    }

    ordered_sources
}

fn doc_meta_for_chunk(conn: &Connection, chunk_id: &str) -> Result<Option<DocMeta>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT d.path, d.doc_class
        FROM chunks c
        JOIN documents d ON d.id = c.doc_id
        WHERE c.id = ?1
        "#,
    )?;
    let mut rows = stmt.query(params![chunk_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(DocMeta {
            path: row.get::<_, Option<String>>(0)?,
            doc_class: row.get::<_, String>(1)?,
        })),
        None => Ok(None),
    }
}

fn canonical_edge_key(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}\0{b}")
    } else {
        format!("{b}\0{a}")
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4).max(1)
}

fn trim_to_token_budget(text: &str, token_budget: usize) -> String {
    let compact = compact_text(text);
    let max_chars = token_budget.saturating_mul(4).max(1);
    if compact.chars().count() <= max_chars {
        return compact;
    }

    let mut truncated: String = compact.chars().take(max_chars).collect();
    let min_boundary = max_chars / 2;
    let boundary = truncated
        .match_indices(['.', '\n', ';'])
        .map(|(index, marker)| index + marker.len())
        .filter(|index| *index >= min_boundary)
        .last();
    if let Some(index) = boundary {
        truncated.truncate(index);
    }
    truncated.trim_end().to_string() + "..."
}

fn compact_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn excerpt_dedupe_key(excerpt: &str) -> String {
    excerpt
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_whitespace() {
                Some(' ')
            } else {
                None
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(220)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{build_context_pack, ContextPackOpts};
    use crate::{
        store::db::migrate,
        types::{ChunkSignal, QueryResult},
    };
    use rusqlite::{params, Connection};

    fn conn_with_docs() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        migrate(&conn).expect("migration");
        conn
    }

    fn insert_chunk(conn: &Connection, doc_id: &str, chunk_id: &str, content: &str, signal: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO documents (id, path, filename, format, size_bytes, hash, indexed_at, status, doc_class) VALUES (?1, ?2, ?3, 'md', 1, 'hash', '2026-05-21T00:00:00Z', 'indexed', 'content')",
            params![doc_id, format!("/kb/{doc_id}.md"), format!("{doc_id}.md")],
        )
        .expect("insert document");
        conn.execute(
            "INSERT INTO chunks (id, doc_id, chunk_index, content, char_start, char_end, page, chunk_signal, created_at) VALUES (?1, ?2, 0, ?3, 0, ?4, NULL, ?5, '2026-05-21T00:00:00Z')",
            params![chunk_id, doc_id, content, content.len() as i64, signal],
        )
        .expect("insert chunk");
    }

    fn result(
        chunk_id: &str,
        doc_id: &str,
        content: &str,
        signal: ChunkSignal,
        score: f32,
    ) -> QueryResult {
        QueryResult {
            chunk_id: chunk_id.to_string(),
            doc_id: doc_id.to_string(),
            content: content.to_string(),
            filename: format!("{doc_id}.md"),
            page: None,
            chunk_signal: signal,
            score,
            score_bm25: score,
            score_vec: 0.0,
            score_graph: 0.0,
            score_entity: 0.0,
            score_centrality: 0.0,
        }
    }

    #[test]
    fn enforces_budget_by_trimming_first_item() {
        let conn = conn_with_docs();
        let content = "Atlas incident evidence. ".repeat(80);
        insert_chunk(&conn, "doc-1", "chunk-1", &content, "content");
        let results = vec![result(
            "chunk-1",
            "doc-1",
            &content,
            ChunkSignal::Content,
            0.9,
        )];

        let pack = build_context_pack(
            &conn,
            "Atlas",
            &results,
            ContextPackOpts {
                budget_tokens: 80,
                include_graph: false,
                ..ContextPackOpts::default()
            },
        )
        .expect("pack");

        assert_eq!(pack.items.len(), 1);
        assert!(pack.budget.estimated_used_tokens <= 80);
        assert!(pack.items[0].excerpt.len() < content.len());
    }

    #[test]
    fn prefers_content_before_anchor_list_results() {
        let conn = conn_with_docs();
        let anchor = "Document anchors: INC-2026-ATLAS-014 API-ATLAS-014";
        let prose =
            "Incident INC-2026-ATLAS-014 describes delayed Atlas approvals with receipt evidence.";
        insert_chunk(&conn, "doc-a", "anchor", anchor, "anchor_list");
        insert_chunk(&conn, "doc-b", "content", prose, "content");
        let results = vec![
            result("anchor", "doc-a", anchor, ChunkSignal::AnchorList, 0.99),
            result("content", "doc-b", prose, ChunkSignal::Content, 0.7),
        ];

        let pack = build_context_pack(
            &conn,
            "INC-2026-ATLAS-014",
            &results,
            ContextPackOpts::default(),
        )
        .expect("pack");

        assert_eq!(pack.items[0].chunk_id, "content");
    }

    #[test]
    fn falls_back_to_anchor_list_when_no_content_matches() {
        let conn = conn_with_docs();
        let anchor = "Document anchors: ONLY-ANCHOR-001";
        insert_chunk(&conn, "doc-a", "anchor", anchor, "anchor_list");
        let results = vec![result(
            "anchor",
            "doc-a",
            anchor,
            ChunkSignal::AnchorList,
            0.9,
        )];

        let pack = build_context_pack(
            &conn,
            "ONLY-ANCHOR-001",
            &results,
            ContextPackOpts::default(),
        )
        .expect("pack");

        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].chunk_signal, ChunkSignal::AnchorList);
    }

    #[test]
    fn suppresses_duplicate_excerpts() {
        let conn = conn_with_docs();
        let content = "Receipt OCR shows Atlas approval INC-2026-ATLAS-014 at register lane seven.";
        insert_chunk(&conn, "doc-a", "chunk-a", content, "content");
        insert_chunk(&conn, "doc-b", "chunk-b", content, "content");
        let results = vec![
            result("chunk-a", "doc-a", content, ChunkSignal::Content, 0.9),
            result("chunk-b", "doc-b", content, ChunkSignal::Content, 0.8),
        ];

        let pack = build_context_pack(
            &conn,
            "Atlas approval",
            &results,
            ContextPackOpts::default(),
        )
        .expect("pack");

        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.budget.omitted_items, 0);
    }

    #[test]
    fn graph_relations_keep_content_and_skip_low_signal_neighbors() {
        let conn = conn_with_docs();
        let seed = "Incident INC-2026-ATLAS-014 cites ATLAS-EDGE-1 in prose.";
        let neighbor = "CSV row confirms ATLAS-EDGE-1 for approval route.";
        let hub = "Document anchors: ATLAS-EDGE-1 INC-2026-ATLAS-014";
        insert_chunk(&conn, "doc-a", "seed", seed, "content");
        insert_chunk(&conn, "doc-b", "neighbor", neighbor, "content");
        insert_chunk(&conn, "doc-c", "hub", hub, "anchor_list");
        insert_chunk(
            &conn,
            "doc-d",
            "readme",
            "README content also mentions ATLAS-EDGE-1.",
            "content",
        );
        conn.execute(
            "UPDATE documents SET doc_class = 'reference' WHERE id = 'doc-d'",
            [],
        )
        .expect("mark reference");
        conn.execute(
            "INSERT INTO graph_edges (src_chunk, dst_chunk, weight, edge_type, reason) VALUES ('seed', 'neighbor', 0.9, 'shared_anchor', 'anchor:ATLAS-EDGE-1')",
            [],
        )
        .expect("insert content edge");
        conn.execute(
            "INSERT INTO graph_edges (src_chunk, dst_chunk, weight, edge_type, reason) VALUES ('seed', 'hub', 0.95, 'shared_anchor', 'anchor:ATLAS-EDGE-1')",
            [],
        )
        .expect("insert hub edge");
        conn.execute(
            "INSERT INTO graph_edges (src_chunk, dst_chunk, weight, edge_type, reason) VALUES ('seed', 'readme', 0.92, 'shared_anchor', 'anchor:ATLAS-EDGE-1')",
            [],
        )
        .expect("insert reference edge");
        let results = vec![result("seed", "doc-a", seed, ChunkSignal::Content, 0.9)];

        let pack = build_context_pack(&conn, "ATLAS-EDGE-1", &results, ContextPackOpts::default())
            .expect("pack");

        assert_eq!(pack.relations.len(), 1);
        assert_eq!(pack.relations[0].target_chunk_id, "neighbor");
        assert_eq!(
            pack.relations[0]
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.anchor.as_deref()),
            Some("ATLAS-EDGE-1")
        );
    }
}
