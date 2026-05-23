use std::path::{Path, PathBuf};

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let metadata = metadata_for_path(path)?;
    let bytes = std::fs::read(path)?;

    // Cheap path: the preprocessing pre-pass has already OCR'd this image
    // and written `<stem>.anubis.txt`. If that sidecar is at least as
    // fresh as the source, reuse it verbatim. This mirrors the video
    // parser's sidecar policy and is what makes the two-stage pipeline
    // work: the heavy OCR runs once during pre-pass, the indexer never
    // re-runs it.
    let text = if let Some(cached) = read_fresh_sidecar(path) {
        tracing::info!(
            "reusing OCR sidecar {} for {}",
            cached.0.display(),
            path.display()
        );
        cached.1
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

/// Mirror of [`crate::parser::video::read_fresh_sidecar`] for images. Same
/// `<stem>.anubis.txt` filename convention so the engine treats text and
/// transcript sidecars uniformly.
pub(crate) fn read_fresh_sidecar(source: &Path) -> Option<(PathBuf, String)> {
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
            std::env::temp_dir().join(format!("anubis-img-sidecar-fresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("photo.png");
        let sidecar = tmp.join("photo.anubis.txt");

        let base = SystemTime::now() - Duration::from_secs(10);
        write_with_mtime(&source, "fake png", base);
        write_with_mtime(&sidecar, "extracted text", base + Duration::from_secs(5));

        let (cached_path, text) = read_fresh_sidecar(&source).expect("should hit");
        assert_eq!(cached_path, sidecar);
        assert_eq!(text, "extracted text");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn sidecar_ignored_when_older_than_source() {
        let tmp =
            std::env::temp_dir().join(format!("anubis-img-sidecar-stale-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("photo.png");
        let sidecar = tmp.join("photo.anubis.txt");

        let base = SystemTime::now() - Duration::from_secs(10);
        write_with_mtime(&sidecar, "stale", base);
        write_with_mtime(&source, "fresh png", base + Duration::from_secs(5));

        assert!(read_fresh_sidecar(&source).is_none());
        std::fs::remove_dir_all(&tmp).ok();
    }
}
