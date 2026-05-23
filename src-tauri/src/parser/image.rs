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
        searchable_image_text(&metadata.filename, &cached.text)
    } else {
        // No sidecar — fall back to inline OCR. This keeps the single-file
        // reindex button working (it calls `parser::parse` directly without
        // going through the pre-pass) and gives a sensible default for
        // callers that bypass the engine.
        let text = crate::ocr::engine::run(&bytes)?;
        searchable_image_text(&metadata.filename, &text)
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

fn searchable_image_text(filename: &str, text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("Image OCR text for {filename}:\n{trimmed}")
}

#[cfg(test)]
mod tests {
    use super::searchable_image_text;

    #[test]
    fn image_ocr_text_carries_filename_context() {
        let text = searchable_image_text(
            "img_invoice_02.png",
            "VID-APPROVAL-005 invoice approval confirms dock payment evidence.",
        );

        assert!(text.contains("img_invoice_02.png"));
        assert!(text.contains("VID-APPROVAL-005 invoice approval"));
    }

    #[test]
    fn empty_image_ocr_text_stays_empty() {
        assert_eq!(searchable_image_text("blank.png", " \n\t"), "");
    }
}
