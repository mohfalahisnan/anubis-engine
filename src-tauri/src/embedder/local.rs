use fastembed::TextEmbedding;

use crate::EngineError;

pub const EMBEDDING_DIM: usize = 384;

pub fn embed_batch(
    model: &mut TextEmbedding,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, EngineError> {
    model
        .embed(texts, None)
        .map_err(|error| EngineError::Embed(error.to_string()))
}

pub fn embed_query(model: &mut TextEmbedding, text: &str) -> Result<Vec<f32>, EngineError> {
    let embeddings = embed_batch(model, &[text.to_string()])?;
    embeddings
        .into_iter()
        .next()
        .ok_or_else(|| EngineError::Embed("empty embedding result".to_string()))
}

pub fn deterministic_embedding(text: &str) -> Vec<f32> {
    let mut embedding = vec![0.0f32; EMBEDDING_DIM];

    for token in text.split_whitespace() {
        let mut hash = 1469598103934665603u64;
        for byte in token.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(1099511628211);
        }
        let index = (hash as usize) % EMBEDDING_DIM;
        embedding[index] += 1.0;
    }

    let norm = embedding
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm > 0.0 {
        for value in &mut embedding {
            *value /= norm;
        }
    }

    embedding
}

pub fn deterministic_embed_batch(texts: &[String]) -> Vec<Vec<f32>> {
    texts
        .iter()
        .map(|text| deterministic_embedding(text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{deterministic_embedding, EMBEDDING_DIM};

    #[test]
    fn deterministic_embedding_has_expected_dimension_and_norm() {
        let embedding = deterministic_embedding("promo printer thermal");
        let norm = embedding
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();

        assert_eq!(embedding.len(), EMBEDDING_DIM);
        assert!((norm - 1.0).abs() < 0.00001);
    }
}
