use std::collections::HashMap;

use crate::{store::graph_store::GraphEdge, types::Chunk};

/// Minimum cosine for a *named* semantic edge.
pub const SEMANTIC_THRESHOLD: f32 = 0.62;
/// Floor for top-K cross-doc inclusion (weaker than SEMANTIC_THRESHOLD but
/// still meaningful) — ensures every chunk has some cross-doc connections.
pub const SEMANTIC_TOPK_FLOOR: f32 = 0.45;
/// How many cross-doc neighbors to keep per current chunk.
pub const SEMANTIC_TOPK: usize = 5;
pub const SAME_DOC_WEIGHT: f32 = 0.5;

/// (chunk_id, doc_id, embedding) of chunks already in the store from
/// previously-indexed documents.
pub type ExistingVector = (String, String, Vec<f32>);

pub fn build_edges(
    current_chunks: &[Chunk],
    current_embeddings: &[Vec<f32>],
    existing_vectors: &[ExistingVector],
) -> Vec<GraphEdge> {
    let mut raw: Vec<GraphEdge> = Vec::new();

    // Intra-document: same_doc + within-doc semantic above threshold.
    for left in 0..current_chunks.len() {
        for right in (left + 1)..current_chunks.len() {
            let l = &current_chunks[left];
            let r = &current_chunks[right];

            raw.push(GraphEdge::canonical(
                &l.id,
                &r.id,
                SAME_DOC_WEIGHT,
                "same_doc",
            ));

            if let (Some(le), Some(re)) =
                (current_embeddings.get(left), current_embeddings.get(right))
            {
                let sim = cosine_sim(le, re);
                if sim >= SEMANTIC_THRESHOLD {
                    raw.push(GraphEdge::canonical(&l.id, &r.id, sim, "semantic"));
                }
            }
        }
    }

    // Cross-document: top-K semantic neighbors per current chunk + threshold.
    for (idx, chunk) in current_chunks.iter().enumerate() {
        let Some(current_embedding) = current_embeddings.get(idx) else {
            continue;
        };

        let mut candidates: Vec<(String, f32)> = existing_vectors
            .iter()
            .map(|(other_id, _other_doc, other_emb)| {
                (other_id.clone(), cosine_sim(current_embedding, other_emb))
            })
            .filter(|(_, sim)| *sim >= SEMANTIC_TOPK_FLOOR)
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (other_id, sim) in candidates.into_iter().take(SEMANTIC_TOPK) {
            let edge_type = if sim >= SEMANTIC_THRESHOLD {
                "semantic"
            } else {
                "semantic_topk"
            };
            raw.push(GraphEdge::canonical(&chunk.id, &other_id, sim, edge_type));
        }
    }

    dedupe_by_max_weight(raw)
}

fn dedupe_by_max_weight(edges: Vec<GraphEdge>) -> Vec<GraphEdge> {
    let mut map: HashMap<(String, String), GraphEdge> = HashMap::new();
    for edge in edges {
        let key = (edge.src_chunk.clone(), edge.dst_chunk.clone());
        match map.get_mut(&key) {
            Some(existing) if edge.weight > existing.weight => *existing = edge,
            Some(_) => {}
            None => {
                map.insert(key, edge);
            }
        }
    }
    map.into_values().collect()
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    crate::store::vectors::cosine_sim(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Chunk;

    fn chunk(id: &str, doc_id: &str) -> Chunk {
        Chunk {
            id: id.to_string(),
            doc_id: doc_id.to_string(),
            chunk_index: 0,
            content: String::new(),
            char_start: 0,
            char_end: 0,
            page: None,
        }
    }

    #[test]
    fn same_doc_chunks_get_same_doc_edge() {
        let chunks = vec![chunk("a1", "doc-a"), chunk("a2", "doc-a")];
        let embs = vec![vec![1.0, 0.0], vec![0.0, 1.0]]; // orthogonal
        let edges = build_edges(&chunks, &embs, &[]);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_type, "same_doc");
    }

    #[test]
    fn cross_doc_topk_uses_existing_vectors() {
        let chunks = vec![chunk("a1", "doc-a")];
        let embs = vec![vec![1.0, 0.0]];
        let existing = vec![
            ("b1".to_string(), "doc-b".to_string(), vec![1.0, 0.0]),
            ("c1".to_string(), "doc-c".to_string(), vec![0.95, 0.05]),
            ("d1".to_string(), "doc-d".to_string(), vec![0.0, 1.0]),
        ];
        let edges = build_edges(&chunks, &embs, &existing);
        let kinds: Vec<&str> = edges.iter().map(|e| e.edge_type.as_str()).collect();
        assert!(kinds.contains(&"semantic"));
        // d1 is orthogonal (sim=0) → must NOT appear
        assert!(!edges
            .iter()
            .any(|e| e.src_chunk == "d1" || e.dst_chunk == "d1"));
    }
}
