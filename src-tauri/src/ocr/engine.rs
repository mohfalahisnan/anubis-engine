//! Real OCR pipeline backed by the `ocrs` crate (rten ONNX runtime).
//!
//! Two model files are required (detection + recognition). They live next to
//! the SQLite database in the app data dir. First call to [`run`] will
//! download the upstream `ocrs-models` artifacts if they are not already
//! present.

use std::path::PathBuf;
use std::sync::OnceLock;

use ocrs::{ImageSource, OcrEngine, OcrEngineParams};

use crate::engine::download::ensure_file;
use crate::EngineError;

const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

const DETECTION_MODEL_FILE: &str = "text-detection.rten";
const RECOGNITION_MODEL_FILE: &str = "text-recognition.rten";

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();
static ENGINE: OnceLock<Result<OcrEngine, String>> = OnceLock::new();

/// Wire the directory where OCR models live (next to the SQLite DB). Called
/// once from `AppState::new`. Safe to call repeatedly; only the first wins.
pub fn set_models_dir(path: PathBuf) {
    let _ = MODELS_DIR.set(path);
}

fn models_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("ANUBIS_OCR_MODELS_DIR") {
        return PathBuf::from(env_dir);
    }
    if let Some(dir) = MODELS_DIR.get() {
        return dir.clone();
    }
    std::env::temp_dir().join("anubis-ocr-models")
}

fn engine() -> Result<&'static OcrEngine, EngineError> {
    ENGINE
        .get_or_init(init_engine)
        .as_ref()
        .map_err(|message| EngineError::Ocr(message.clone()))
}

fn init_engine() -> Result<OcrEngine, String> {
    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create OCR models dir {:?}: {error}", dir))?;

    let detection_path = dir.join(DETECTION_MODEL_FILE);
    let recognition_path = dir.join(RECOGNITION_MODEL_FILE);

    ensure_file(
        &detection_path,
        DETECTION_MODEL_URL,
        "ocr-detection",
        "OCR text detection model",
    )?;
    ensure_file(
        &recognition_path,
        RECOGNITION_MODEL_URL,
        "ocr-recognition",
        "OCR text recognition model",
    )?;

    let detection_model = rten::Model::load_file(&detection_path)
        .map_err(|error| format!("failed to load detection model: {error}"))?;
    let recognition_model = rten::Model::load_file(&recognition_path)
        .map_err(|error| format!("failed to load recognition model: {error}"))?;

    OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|error| format!("failed to construct ocrs engine: {error}"))
}

pub fn run(image_bytes: &[u8]) -> Result<String, EngineError> {
    if image_bytes.is_empty() {
        return Ok(String::new());
    }

    let engine = engine()?;

    let decoded = image::load_from_memory(image_bytes)
        .map_err(|error| EngineError::Ocr(format!("image decode failed: {error}")))?;
    let rgb = decoded.to_rgb8();
    let (width, height) = rgb.dimensions();
    let raw = rgb.into_raw();

    let source = ImageSource::from_bytes(&raw, (width, height))
        .map_err(|error| EngineError::Ocr(format!("ocr image source: {error:?}")))?;
    let input = engine
        .prepare_input(source)
        .map_err(|error| EngineError::Ocr(format!("ocr prepare_input: {error}")))?;
    let text = engine
        .get_text(&input)
        .map_err(|error| EngineError::Ocr(format!("ocr get_text: {error}")))?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::models_dir;

    #[test]
    fn models_dir_respects_env_override() {
        // Don't depend on what AppState set; just check env precedence.
        std::env::set_var("ANUBIS_OCR_MODELS_DIR", "X:\\nope");
        assert_eq!(
            models_dir(),
            std::path::PathBuf::from("X:\\nope"),
            "env var should win"
        );
        std::env::remove_var("ANUBIS_OCR_MODELS_DIR");
    }
}
