use std::path::Path;

use crate::{
    types::{DocFormat, DocMetadata},
    EngineError,
};

pub mod docx;
pub mod image;
pub mod markdown;
pub mod pdf;
pub mod xlsx;

pub fn parse(path: &Path) -> Result<crate::types::ParsedDoc, EngineError> {
    match format_from_path(path) {
        DocFormat::Markdown | DocFormat::Text => markdown::parse(path),
        DocFormat::Pdf => pdf::parse(path),
        DocFormat::Docx => docx::parse(path),
        DocFormat::Xlsx => xlsx::parse(path),
        DocFormat::Image => image::parse(path),
    }
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
        Some("png" | "jpg" | "jpeg" | "webp" | "tiff") => DocFormat::Image,
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
