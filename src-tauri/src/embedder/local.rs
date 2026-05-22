use fastembed::TextEmbedding;

use crate::EngineError;

pub const EMBEDDING_DIM: usize = 384;

/// E5 family was trained with two distinct task prefixes. Skipping these
/// silently degrades recall by ~5-15 percentage points.
const PASSAGE_PREFIX: &str = "passage: ";
const QUERY_PREFIX: &str = "query: ";

fn embed_raw(
    model: &mut TextEmbedding,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, EngineError> {
    model
        .embed(texts, None)
        .map_err(|error| EngineError::Embed(error.to_string()))
}

/// Embed a batch of document chunks (passages) — applies the E5 `passage:` prefix.
pub fn embed_batch(
    model: &mut TextEmbedding,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, EngineError> {
    let prefixed: Vec<String> = texts
        .iter()
        .map(|text| format!("{PASSAGE_PREFIX}{text}"))
        .collect();
    embed_raw(model, &prefixed)
}

/// Embed a user query — applies the E5 `query:` prefix.
pub fn embed_query(model: &mut TextEmbedding, text: &str) -> Result<Vec<f32>, EngineError> {
    let prefixed = format!("{QUERY_PREFIX}{text}");
    let embeddings = embed_raw(model, &[prefixed])?;
    embeddings
        .into_iter()
        .next()
        .ok_or_else(|| EngineError::Embed("empty embedding result".to_string()))
}

pub fn embed_batch_with_retry(
    model: &mut TextEmbedding,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, EngineError> {
    match embed_batch(model, texts) {
        Ok(embeddings) => Ok(embeddings),
        Err(batch_error) => {
            tracing::warn!(
                "fastembed batch failed: {}; retrying chunks one by one",
                batch_error
            );
            let mut embeddings = Vec::with_capacity(texts.len());
            for text in texts {
                let mut result = embed_batch(model, std::slice::from_ref(text))?;
                let embedding = result
                    .pop()
                    .ok_or_else(|| EngineError::Embed("empty embedding result".to_string()))?;
                embeddings.push(embedding);
            }
            Ok(embeddings)
        }
    }
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
