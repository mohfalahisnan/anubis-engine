use crate::{store::graph_store::GraphEdge, types::Chunk};

pub fn build_edges(chunks: &[Chunk], embeddings: &[Vec<f32>]) -> Vec<GraphEdge> {
    let mut edges = Vec::new();

    for left_index in 0..chunks.len() {
        for right_index in (left_index + 1)..chunks.len() {
            let left = &chunks[left_index];
            let right = &chunks[right_index];

            if left.doc_id == right.doc_id {
                edges.push(GraphEdge {
                    src_chunk: left.id.clone(),
                    dst_chunk: right.id.clone(),
                    weight: 0.5,
                    edge_type: "same_doc".to_string(),
                });
            }

            if let (Some(left_embedding), Some(right_embedding)) =
                (embeddings.get(left_index), embeddings.get(right_index))
            {
                let similarity = crate::graph::scorer::cosine_sim(left_embedding, right_embedding);
                if similarity > 0.75 {
                    edges.push(GraphEdge {
                        src_chunk: left.id.clone(),
                        dst_chunk: right.id.clone(),
                        weight: similarity,
                        edge_type: "semantic".to_string(),
                    });
                }
            }
        }
    }

    edges
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
