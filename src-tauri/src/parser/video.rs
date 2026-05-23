use std::path::Path;

use crate::{
    engine::{settings::transcription_enabled, sidecar},
    parser::metadata_for_path,
    transcription::engine::transcribe_file,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    parse_with_format(path, DocFormat::Video)
}

pub(crate) fn parse_with_format(path: &Path, format: DocFormat) -> Result<ParsedDoc, EngineError> {
    let metadata = metadata_for_path(path)?;

    let text = if transcription_enabled() {
        // Cheap path: a previous run (or the preprocessing pre-pass) already
        // wrote `<stem>.anubis.txt` next to the source. `engine::sidecar`
        // owns the freshness contract — we just consume whatever it
        // returns. An empty sidecar is authoritative ("ASR found no usable
        // speech"); the user can delete the file to force a re-transcribe.
        if let Some(cached) = sidecar::read(path) {
            tracing::info!(
                "reusing cached transcript {} for {}",
                cached.path.display(),
                path.display()
            );
            cached.text
        } else {
            // A media file with no audio stream is a *valid* corpus entry —
            // it just has no transcript. Surface it in the index as a
            // regular document with empty text so the user can still see
            // the file in their library (searchable by filename / metadata).
            match transcribe_file(path) {
                Ok(text) => text,
                Err(EngineError::NoAudioTrack(_)) => {
                    tracing::info!(
                        "no audio track in {}; indexing with empty transcript",
                        path.display()
                    );
                    String::new()
                }
                Err(error) => return Err(error),
            }
        }
    } else {
        tracing::debug!(
            "transcription disabled; indexing {} as metadata-only",
            path.display()
        );
        String::new()
    };

    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format,
        pages: vec![ParsedPage {
            page_num: None,
            text,
            images: Vec::new(),
        }],
        metadata,
        doc_class: Default::default(),
    })
}

// All sidecar logic lives in `engine::sidecar`; this module no longer
// duplicates path resolution or freshness checks. Tests for those live
// alongside the implementation there.
