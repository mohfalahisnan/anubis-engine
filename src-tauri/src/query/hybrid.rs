use std::collections::HashMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{
    store::{chunks::query_result_for_chunk, vectors},
    types::QueryResult,
    EngineError,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreParts {
    pub chunk_id: String,
    pub score_bm25: f32,
    pub score_vec: f32,
    pub score_graph: f32,
}

pub fn final_score(score_vec: f32, score_bm25: f32, score_graph: f32) -> f32 {
    ((0.5 * score_vec) + (0.3 * score_bm25) + (0.2 * score_graph)).clamp(0.0, 1.0)
}

pub fn merge_scores(
    vector_scores: Vec<(String, f32)>,
    bm25_scores: Vec<(String, f32)>,
    graph_scores: Vec<(String, f32)>,
    limit: usize,
) -> Vec<ScoreParts> {
    let mut merged: HashMap<String, ScoreParts> = HashMap::new();

    for (chunk_id, score_vec) in vector_scores {
        merged
            .entry(chunk_id.clone())
            .or_insert_with(|| ScoreParts {
                chunk_id,
                score_bm25: 0.0,
                score_vec: 0.0,
                score_graph: 0.0,
            })
            .score_vec = score_vec;
    }

    for (chunk_id, score_bm25) in bm25_scores {
        merged
            .entry(chunk_id.clone())
            .or_insert_with(|| ScoreParts {
                chunk_id,
                score_bm25: 0.0,
                score_vec: 0.0,
                score_graph: 0.0,
            })
            .score_bm25 = score_bm25;
    }

    for (chunk_id, score_graph) in graph_scores {
        merged
            .entry(chunk_id.clone())
            .or_insert_with(|| ScoreParts {
                chunk_id,
                score_bm25: 0.0,
                score_vec: 0.0,
                score_graph: 0.0,
            })
            .score_graph = score_graph;
    }

    let mut scores: Vec<ScoreParts> = merged.into_values().collect();
    scores.sort_by(|left, right| {
        final_score(right.score_vec, right.score_bm25, right.score_graph)
            .partial_cmp(&final_score(
                left.score_vec,
                left.score_bm25,
                left.score_graph,
            ))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scores.truncate(limit);
    scores
}

pub fn query_with_embedding(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<QueryResult>, EngineError> {
    let vector_hits = vectors::search_vectors(conn, query_embedding, limit * 3)?;
    let vector_scores = normalize_scores(
        vector_hits
            .into_iter()
            .map(|hit| (hit.chunk_id, hit.score))
            .collect(),
    );
    let graph_scores = graph_boosts(conn, vector_scores.iter().map(|(id, _)| id.clone()).collect())?;
    let merged = merge_scores(vector_scores, Vec::new(), graph_scores, limit);
    let mut results = Vec::new();

    for score in merged {
        let final_score = final_score(score.score_vec, score.score_bm25, score.score_graph);
        if let Some(result) = query_result_for_chunk(
            conn,
            &score.chunk_id,
            final_score,
            score.score_bm25,
            score.score_vec,
            score.score_graph,
        )? {
            results.push(result);
        }
    }

    Ok(results)
}

fn normalize_scores(scores: Vec<(String, f32)>) -> Vec<(String, f32)> {
    let max_score = scores
        .iter()
        .map(|(_, score)| *score)
        .fold(0.0f32, f32::max);

    if max_score <= 0.0 {
        return scores.into_iter().map(|(id, _)| (id, 0.0)).collect();
    }

    scores
        .into_iter()
        .map(|(id, score)| (id, (score / max_score).clamp(0.0, 1.0)))
        .collect()
}

fn graph_boosts(
    conn: &Connection,
    chunk_ids: Vec<String>,
) -> Result<Vec<(String, f32)>, EngineError> {
    let mut boosts = Vec::new();
    let mut stmt = conn.prepare(
        r#"
        SELECT AVG(weight)
        FROM graph_edges
        WHERE src_chunk = ?1 OR dst_chunk = ?1
        "#,
    )?;

    for chunk_id in chunk_ids {
        let score: Option<f64> = stmt.query_row([&chunk_id], |row| row.get(0))?;
        boosts.push((chunk_id, score.unwrap_or(0.0) as f32));
    }

    Ok(boosts)
}

#[cfg(test)]
mod tests {
    use super::{final_score, merge_scores};

    #[test]
    fn merges_scores_with_spec_weights_and_sorts_descending() {
        let merged = merge_scores(
            vec![("chunk-a".to_string(), 1.0), ("chunk-b".to_string(), 0.2)],
            vec![("chunk-a".to_string(), 0.5), ("chunk-b".to_string(), 1.0)],
            vec![("chunk-a".to_string(), 0.5), ("chunk-b".to_string(), 0.0)],
            2,
        );

        assert_eq!(merged[0].chunk_id, "chunk-a");
        assert!((final_score(1.0, 0.5, 0.5) - 0.75).abs() < 0.00001);
        assert!((final_score(0.2, 1.0, 0.0) - 0.4).abs() < 0.00001);
    }
}
