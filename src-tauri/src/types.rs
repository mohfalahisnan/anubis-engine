use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDoc {
    pub doc_id: String,
    pub path: String,
    pub format: DocFormat,
    pub pages: Vec<ParsedPage>,
    pub metadata: DocMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPage {
    pub page_num: Option<u32>,
    pub text: String,
    pub images: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    Markdown,
    Pdf,
    Docx,
    Xlsx,
    Image,
    Text,
}

impl DocFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Image => "image",
            Self::Text => "text",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "md" | "markdown" => Self::Markdown,
            "pdf" => Self::Pdf,
            "docx" => Self::Docx,
            "xlsx" => Self::Xlsx,
            "image" => Self::Image,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub chunk_id: String,
    pub doc_id: String,
    pub content: String,
    pub filename: String,
    pub page: Option<u32>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexStatus {
    Idle,
    Running,
    Done,
    Error,
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
