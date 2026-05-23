use std::path::Path;

use crate::{
    engine::sidecar,
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let metadata = metadata_for_path(path)?;
    let bytes = std::fs::read(path)?;

    // Cheap path: the preprocessing pre-pass has already OCR'd this image
    // and written `<stem>.anubis.txt`. `engine::sidecar` owns freshness;
    // if it returns text, we use it verbatim and skip the OCR model
    // entirely.
    let text = if let Some(cached) = sidecar::read(path) {
        tracing::info!(
            "reusing OCR sidecar {} for {}",
            cached.path.display(),
            path.display()
        );
        cached.text
    } else {
        // No sidecar — fall back to inline OCR. This keeps the single-file
        // reindex button working (it calls `parser::parse` directly without
        // going through the pre-pass) and gives a sensible default for
        // callers that bypass the engine.
        crate::ocr::engine::run(&bytes)?
    };

    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Image,
        pages: vec![ParsedPage {
            page_num: Some(1),
            text,
            images: vec![bytes],
        }],
        metadata,
        doc_class: Default::default(),
    })
}

// All sidecar logic lives in `engine::sidecar`; this module no longer
// duplicates path resolution or freshness checks. Tests for those live
// alongside the implementation there.
