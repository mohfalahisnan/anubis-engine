use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDoc {
    pub doc_id: String,
    pub path: String,
    pub format: DocFormat,
    pub pages: Vec<ParsedPage>,
    pub metadata: DocMetadata,
    #[serde(default = "default_doc_class")]
    pub doc_class: DocClass,
}

fn default_doc_class() -> DocClass {
    DocClass::Content
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPage {
    pub page_num: Option<u32>,
    pub text: String,
    pub images: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChunkSignal {
    #[default]
    Content,
    AnchorList,
    Metadata,
}

impl ChunkSignal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::AnchorList => "anchor_list",
            Self::Metadata => "metadata",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "anchor_list" => Self::AnchorList,
            "metadata" => Self::Metadata,
            _ => Self::Content,
        }
    }

    pub fn is_low_signal(&self) -> bool {
        matches!(self, Self::AnchorList | Self::Metadata)
    }
}

/// Classifies a document as primary content or as a reference/index/manifest
/// (README, file listings, relation maps). Reference docs get a search
/// down-rank and are excluded from being used as relation evidence.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DocClass {
    #[default]
    Content,
    Reference,
}

impl DocClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Reference => "reference",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "reference" => Self::Reference,
            _ => Self::Content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    Markdown,
    Pdf,
    Docx,
    Xlsx,
    Csv,
    Json,
    Image,
    Video,
    Audio,
    Text,
}

impl DocFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Text => "text",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "md" | "markdown" => Self::Markdown,
            "pdf" => Self::Pdf,
            "docx" => Self::Docx,
            "xlsx" => Self::Xlsx,
            "csv" => Self::Csv,
            "json" => Self::Json,
            "image" => Self::Image,
            "video" => Self::Video,
            "audio" => Self::Audio,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMetadata {
    pub filename: String,
    pub size_bytes: u64,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chunk {
    pub id: String,
    pub doc_id: String,
    pub chunk_index: usize,
    pub content: String,
    pub char_start: usize,
    pub char_end: usize,
    pub page: Option<u32>,
    #[serde(default)]
    pub signal: ChunkSignal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub chunk_id: String,
    pub doc_id: String,
    pub content: String,
    pub filename: String,
    pub page: Option<u32>,
    #[serde(default)]
    pub chunk_signal: ChunkSignal,
    pub score: f32,
    pub score_bm25: f32,
    pub score_vec: f32,
    pub score_graph: f32,
    pub score_entity: f32,
    pub score_centrality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub status: IndexStatus,
    pub errors: Vec<String>,
    /// Optional sub-stage label so the UI can show "parsing big.json" /
    /// "embedding 3.4K chunks" / "writing" / "linking" instead of just
    /// the filename. `None` for between-files emissions to keep the old
    /// shape valid for legacy listeners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<IndexStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexStatus {
    Idle,
    Running,
    Done,
    Error,
    /// User invoked `cancel_indexing` while a stage was in flight. The
    /// indexer halts at the next safe checkpoint (between files / between
    /// sub-stages) and emits this status with the counters at the moment
    /// of cancellation. Completed files stay indexed; partial work is
    /// either rolled back (each file's writes are wrapped in transactions)
    /// or finishes for the in-flight file before the loop exits.
    Cancelled,
}

/// Sub-stage of work happening inside a single `index_one` call. Drives
/// per-file labels in the UI so the user can tell the difference between
/// "this is still parsing the JSON" and "this is now embedding 4K chunks".
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexStage {
    Parsing,
    Embedding,
    Writing,
    Linking,
}

/// Sub-stage of work happening inside the preprocessing pre-pass. Lets
/// the UI distinguish "running Whisper" from "running OCR" from "skipped
/// because the sidecar is fresh".
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreprocessStage {
    Transcribing,
    Ocr,
    CachedSkipped,
}

/// What kind of preprocessing a file needs. `Pdf` is reserved for future
/// scanned-PDF page-image OCR; the pre-pass does not produce it today.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PreprocessKind {
    Video,
    Audio,
    Image,
    Pdf,
}

/// Progress payload for the new `preprocess-progress` Tauri event. Mirrors
/// `IndexProgress` so the UI can render both with the same component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessProgress {
    pub total: usize,
    pub done: usize,
    pub current: String,
    pub kind: Option<PreprocessKind>,
    pub stage: Option<PreprocessStage>,
    pub status: IndexStatus,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{DocFormat, IndexStatus};

    #[test]
    fn serializes_frontend_enums_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&DocFormat::Markdown).expect("json"),
            "\"markdown\""
        );
        assert_eq!(
            serde_json::to_string(&IndexStatus::Running).expect("json"),
            "\"running\""
        );
    }
}
