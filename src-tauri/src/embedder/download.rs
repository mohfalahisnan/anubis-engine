//! Self-hosted downloader for the multilingual-e5-small ONNX model.
//!
//! Bypasses fastembed's built-in hf-hub fetch because it cannot be configured
//! with a longer timeout — and we kept hitting `request error: timeout: global`
//! on slow links. Files are placed in a stable directory next to the SQLite
//! database; once present, fastembed loads them locally via
//! `try_new_from_user_defined`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use fastembed::{
    InitOptionsUserDefined, Pooling, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel,
};

use crate::engine::download::{ensure_file, ensure_file_quiet};
use crate::EngineError;

const HF_BASE: &str = "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main";
const MODEL_DIR_NAME: &str = "embedding-multilingual-e5-small";
const EVENT_ID: &str = "embedding";
const LABEL: &str = "Embedding model (multilingual-e5-small)";

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_models_dir(path: PathBuf) {
    let _ = MODELS_DIR.set(path);
}

fn model_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("ANUBIS_EMBED_MODELS_DIR") {
        return PathBuf::from(env_dir).join(MODEL_DIR_NAME);
    }
    if let Some(dir) = MODELS_DIR.get() {
        return dir.join(MODEL_DIR_NAME);
    }
    std::env::temp_dir().join(MODEL_DIR_NAME)
}

/// Download model artifacts (if missing) and instantiate the fastembed
/// embedder from the local bytes.
pub fn load_or_download() -> Result<TextEmbedding, EngineError> {
    let dir = model_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|error| EngineError::Embed(format!("create model dir: {error}")))?;

    let onnx_path = dir.join("model.onnx");
    let tokenizer_path = dir.join("tokenizer.json");
    let config_path = dir.join("config.json");
    let special_tokens_path = dir.join("special_tokens_map.json");
    let tokenizer_config_path = dir.join("tokenizer_config.json");

    // The model file is ~118 MB — that's the one worth a visible progress bar.
    // Companion tokenizer/config files are tiny; download them silently.
    ensure_file(
        &onnx_path,
        &format!("{HF_BASE}/onnx/model.onnx"),
        EVENT_ID,
        LABEL,
    )
    .map_err(EngineError::Embed)?;

    download_companion(&tokenizer_path, "tokenizer.json")?;
    download_companion(&config_path, "config.json")?;
    download_companion(&special_tokens_path, "special_tokens_map.json")?;
    download_companion(&tokenizer_config_path, "tokenizer_config.json")?;

    let onnx_bytes =
        std::fs::read(&onnx_path).map_err(|error| EngineError::Embed(error.to_string()))?;
    let tokenizer_files = TokenizerFiles {
        tokenizer_file: std::fs::read(&tokenizer_path)
            .map_err(|error| EngineError::Embed(error.to_string()))?,
        config_file: std::fs::read(&config_path)
            .map_err(|error| EngineError::Embed(error.to_string()))?,
        special_tokens_map_file: std::fs::read(&special_tokens_path)
            .map_err(|error| EngineError::Embed(error.to_string()))?,
        tokenizer_config_file: std::fs::read(&tokenizer_config_path)
            .map_err(|error| EngineError::Embed(error.to_string()))?,
    };

    let model =
        UserDefinedEmbeddingModel::new(onnx_bytes, tokenizer_files).with_pooling(Pooling::Mean);

    TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::default())
        .map_err(|error| EngineError::Embed(error.to_string()))
}

fn download_companion(path: &Path, filename: &str) -> Result<(), EngineError> {
    let url = format!("{HF_BASE}/{filename}");
    ensure_file_quiet(path, &url).map_err(EngineError::Embed)
}
