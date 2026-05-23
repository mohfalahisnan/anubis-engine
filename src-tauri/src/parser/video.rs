use std::path::{Path, PathBuf};

use crate::{
    engine::settings::transcription_enabled,
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
        // Cheap path: a previous run already wrote `<stem>.anubis.txt`
        // next to the source. If it's at least as fresh as the source,
        // reuse it verbatim and skip ffmpeg + whisper entirely. An empty
        // sidecar is authoritative ("ASR found no usable speech") — the
        // user can delete it to force a re-transcribe.
        if let Some(cached) = read_fresh_sidecar(path) {
            tracing::info!(
                "reusing cached transcript {} for {}",
                cached.0.display(),
                path.display()
            );
            cached.1
        } else {
            // A media file with no audio stream is a *valid* corpus entry — it
            // just has no transcript. Surface it in the index as a regular
            // document with empty text so the user can still see the file in
            // their library (searchable by filename / metadata).
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

/// Return `Some((sidecar_path, contents))` when a `.anubis.txt` transcript
/// exists next to the source media (or in `ANUBIS_TRANSCRIPT_DIR`) AND its
/// mtime is `>=` the source mtime. Returns `None` to signal "transcribe
/// fresh" — either the sidecar is missing, older than the source, or the
/// filesystem can't tell us. mtime comparison only — hashing the video to
/// validate is expensive and unnecessary; the user can delete the sidecar
/// to force a re-run.
fn read_fresh_sidecar(source: &Path) -> Option<(PathBuf, String)> {
    let stem = source.file_stem().and_then(|s| s.to_str())?;
    let sidecar_dir = sidecar_dir_for(source);
    let sidecar = sidecar_dir.join(format!("{stem}.anubis.txt"));

    let sidecar_meta = std::fs::metadata(&sidecar).ok()?;
    let source_meta = std::fs::metadata(source).ok()?;

    let sidecar_mtime = sidecar_meta.modified().ok()?;
    let source_mtime = source_meta.modified().ok()?;
    if sidecar_mtime < source_mtime {
        return None;
    }

    let contents = std::fs::read_to_string(&sidecar).ok()?;
    Some((sidecar, contents))
}

/// Mirror of `transcription::engine::resolve_output_dir` — kept here as a
/// tiny helper rather than re-exported because that module is otherwise
/// private to the transcription pipeline.
fn sidecar_dir_for(source: &Path) -> PathBuf {
    if let Ok(env_dir) = std::env::var("ANUBIS_TRANSCRIPT_DIR") {
        if !env_dir.trim().is_empty() {
            return PathBuf::from(env_dir);
        }
    }
    source
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

#[cfg(test)]
mod tests {
    use super::read_fresh_sidecar;
    use std::time::{Duration, SystemTime};

    /// Write a file and force its modified time so the test doesn't depend on
    /// wall-clock granularity (Windows FAT/NTFS can be 2-second-coarse).
    fn write_with_mtime(path: &std::path::Path, contents: &str, mtime: SystemTime) {
        std::fs::write(path, contents).expect("write fixture");
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open for mtime");
        file.set_modified(mtime).expect("set mtime");
    }

    #[test]
    fn sidecar_reused_when_at_least_as_fresh_as_source() {
        let tmp =
            std::env::temp_dir().join(format!("anubis-sidecar-fresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("clip.mp4");
        let sidecar = tmp.join("clip.anubis.txt");

        let base = SystemTime::now() - Duration::from_secs(10);
        write_with_mtime(&source, "fake mp4", base);
        write_with_mtime(
            &sidecar,
            "hello from sidecar",
            base + Duration::from_secs(5),
        );

        let (cached_path, text) = read_fresh_sidecar(&source).expect("should hit");
        assert_eq!(cached_path, sidecar);
        assert_eq!(text, "hello from sidecar");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn sidecar_ignored_when_older_than_source() {
        let tmp =
            std::env::temp_dir().join(format!("anubis-sidecar-stale-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("clip.mp4");
        let sidecar = tmp.join("clip.anubis.txt");

        let base = SystemTime::now() - Duration::from_secs(10);
        write_with_mtime(&sidecar, "stale", base);
        write_with_mtime(&source, "fresh mp4", base + Duration::from_secs(5));

        assert!(read_fresh_sidecar(&source).is_none());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn sidecar_returns_none_when_missing() {
        let tmp =
            std::env::temp_dir().join(format!("anubis-sidecar-missing-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("clip.mp4");
        write_with_mtime(&source, "fake mp4", SystemTime::now());

        assert!(read_fresh_sidecar(&source).is_none());
        std::fs::remove_dir_all(&tmp).ok();
    }
}
