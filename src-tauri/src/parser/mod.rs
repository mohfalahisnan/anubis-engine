use std::path::Path;

use crate::{
    types::{DocClass, DocFormat, DocMetadata},
    EngineError,
};

pub mod audio;
pub mod csv;
pub mod docx;
pub mod image;
pub mod json;
pub mod markdown;
pub mod pdf;
pub mod video;
pub mod xlsx;

pub fn parse(path: &Path) -> Result<crate::types::ParsedDoc, EngineError> {
    let mut parsed = match format_from_path(path) {
        DocFormat::Markdown | DocFormat::Text => markdown::parse(path),
        DocFormat::Pdf => pdf::parse(path),
        DocFormat::Docx => docx::parse(path),
        DocFormat::Xlsx => xlsx::parse(path),
        DocFormat::Csv => csv::parse(path),
        DocFormat::Json => json::parse(path),
        DocFormat::Image => image::parse(path),
        DocFormat::Video => video::parse(path),
        DocFormat::Audio => audio::parse(path),
    }?;
    parsed.doc_class = doc_class_from_path(path);
    Ok(parsed)
}

/// Classify a document as primary content vs reference/index/manifest based
/// on its filename. Reference docs (manifest.txt, README.md, index.json,
/// file_list, relation_map, TOC) get a search down-rank and are excluded
/// from being used as relation evidence — see the relations-rework spec.
///
/// Filename-only by design: simple, fast, predictable. The user opts a file
/// in or out by renaming.
pub fn doc_class_from_path(path: &Path) -> DocClass {
    let filename_lower = match path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase)
    {
        Some(name) => name,
        None => return DocClass::Content,
    };

    let stem_lower = filename_lower
        .rsplit_once('.')
        .map(|(stem, _ext)| stem.to_string())
        .unwrap_or_else(|| filename_lower.clone());

    let ext_lower: Option<&str> = filename_lower.rsplit_once('.').map(|(_, ext)| ext);

    // `index.html` is a real document (web page), not a reference index.
    if filename_lower == "index.html" {
        return DocClass::Content;
    }

    if stem_lower == "manifest" && matches!(ext_lower, Some("txt" | "md" | "json" | "csv")) {
        return DocClass::Reference;
    }

    if stem_lower == "index" && matches!(ext_lower, Some("txt" | "md" | "json")) {
        return DocClass::Reference;
    }

    // Exact-name reference files where the spec allows any extension.
    let any_ext_reference_stems: &[&str] = &[
        "file_list",
        "filelist",
        "files",
        "relation_map",
        "relations",
        "toc",
        "table_of_contents",
    ];
    if any_ext_reference_stems.contains(&stem_lower.as_str()) {
        return DocClass::Reference;
    }

    // README and README-anything (README-internal.md, README_old.txt, etc.).
    if stem_lower == "readme"
        || stem_lower.starts_with("readme-")
        || stem_lower.starts_with("readme_")
    {
        return DocClass::Reference;
    }

    // *_index suffix (e.g. `chunks_index.json`, `doc_index.md`) — but only
    // for text-like extensions so we don't pick up `something_index.html`.
    if stem_lower.ends_with("_index") {
        match ext_lower {
            Some("txt") | Some("md") | Some("json") => return DocClass::Reference,
            _ => {}
        }
    }

    DocClass::Content
}

pub fn format_from_path(path: &Path) -> DocFormat {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("md") => DocFormat::Markdown,
        Some("txt") => DocFormat::Text,
        Some("pdf") => DocFormat::Pdf,
        Some("docx") => DocFormat::Docx,
        Some("xlsx") => DocFormat::Xlsx,
        Some("csv") => DocFormat::Csv,
        Some("json") => DocFormat::Json,
        Some("png" | "jpg" | "jpeg" | "webp" | "tiff") => DocFormat::Image,
        Some("mp4" | "mov" | "avi" | "mkv" | "webm" | "wmv") => DocFormat::Video,
        Some("mp3" | "wav" | "m4a" | "flac" | "ogg" | "opus") => DocFormat::Audio,
        _ => DocFormat::Text,
    }
}

pub fn metadata_for_path(path: &Path) -> Result<DocMetadata, EngineError> {
    let bytes = std::fs::read(path)?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    Ok(DocMetadata {
        filename,
        size_bytes: bytes.len() as u64,
        hash: blake3_hex(&bytes),
    })
}

pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::{doc_class_from_path, format_from_path, parse};
    use crate::types::{DocClass, DocFormat};

    #[test]
    fn classifies_manifest_and_readme_filenames_as_reference() {
        let cases = [
            "manifest.txt",
            "manifest.md",
            "manifest.json",
            "MANIFEST.TXT",
            "README",
            "README.md",
            "README-internal.md",
            "Readme_old.txt",
            "index.md",
            "index.json",
            "chunks_index.json",
            "doc_index.md",
            "file_list.txt",
            "filelist.md",
            "files.md",
            "relation_map.json",
            "relations.md",
            "toc.md",
            "table_of_contents.txt",
        ];
        for name in cases {
            assert_eq!(
                doc_class_from_path(std::path::Path::new(name)),
                DocClass::Reference,
                "expected {name} to be classified as reference",
            );
        }
    }

    #[test]
    fn classifies_normal_content_filenames_as_content() {
        let cases = [
            "notes.md",
            "report-2026.pdf",
            "incident_log.json",
            "video.mp4",
            "index.html",           // landing page, not a reference
            "index.csv",            // spec only treats index txt/md/json as reference
            "something_index.html", // also a real page
            "something_index.csv",  // spec only treats *_index txt/md/json as reference
            "manifest.pdf",         // spec only treats manifest txt/md/json/csv as reference
            "interview.docx",
        ];
        for name in cases {
            assert_eq!(
                doc_class_from_path(std::path::Path::new(name)),
                DocClass::Content,
                "expected {name} to be classified as content",
            );
        }
    }

    fn write_temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("anubis-parser-{}-{}", uuid::Uuid::new_v4(), name));
        std::fs::write(&path, content).expect("write fixture");
        path
    }

    #[test]
    fn detects_csv_and_json_formats_from_extensions() {
        assert_eq!(
            format_from_path(std::path::Path::new("route_risk_scores.csv")),
            DocFormat::Csv
        );
        assert_eq!(
            format_from_path(std::path::Path::new("relation_map.json")),
            DocFormat::Json
        );
    }

    #[test]
    fn parses_csv_rows_into_searchable_field_text() {
        let path = write_temp_file(
            "route_risk_scores.csv",
            "route,risk_score,reason\nSHIP-NODE-SURYA,0.79,\"barcode confidence low\"\n",
        );

        let doc = parse(&path).expect("parse csv");

        assert_eq!(doc.format, DocFormat::Csv);
        assert_eq!(doc.pages.len(), 1);
        assert!(doc.pages[0].text.contains("route: SHIP-NODE-SURYA"));
        assert!(doc.pages[0].text.contains("risk_score: 0.79"));
        assert!(doc.pages[0].text.contains("reason: barcode confidence low"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parses_headerless_csv_first_row_into_searchable_text() {
        let path = write_temp_file(
            "route_risk_scores.csv",
            "SHIP-NODE-SURYA,0.79,\"barcode confidence low\"\n",
        );

        let doc = parse(&path).expect("parse headerless csv");

        assert_eq!(doc.format, DocFormat::Csv);
        assert!(doc.pages[0].text.contains("column_1: SHIP-NODE-SURYA"));
        assert!(doc.pages[0].text.contains("column_2: 0.79"));
        assert!(doc.pages[0]
            .text
            .contains("column_3: barcode confidence low"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parses_json_into_searchable_paths_and_values() {
        let path = write_temp_file(
            "relation_map.json",
            r#"{"incident":"INC-2026-ATLAS-014","nodes":[{"id":"BOT-PACK-7","role":"packing robot"}]}"#,
        );

        let doc = parse(&path).expect("parse json");

        assert_eq!(doc.format, DocFormat::Json);
        assert_eq!(doc.pages.len(), 1);
        assert!(doc.pages[0].text.contains("incident: INC-2026-ATLAS-014"));
        assert!(doc.pages[0].text.contains("nodes[0].id: BOT-PACK-7"));
        assert!(doc.pages[0].text.contains("nodes[0].role: packing robot"));
        let _ = std::fs::remove_file(path);
    }
}
