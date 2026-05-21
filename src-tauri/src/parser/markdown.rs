use std::path::Path;

use pulldown_cmark::{Event, Parser};
use uuid::Uuid;

use crate::{
    parser::{format_from_path, metadata_for_path},
    types::{DocFormat, DocMetadata, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let content = std::fs::read_to_string(path)?;
    let metadata = metadata_for_path(path)?;
    Ok(parse_markdown_str(
        Uuid::new_v4().to_string(),
        path.to_string_lossy().into_owned(),
        &content,
        metadata,
        format_from_path(path),
    ))
}

pub fn parse_markdown_str(
    doc_id: String,
    path: String,
    markdown: &str,
    metadata: DocMetadata,
    format: DocFormat,
) -> ParsedDoc {
    let mut text = String::new();

    for event in Parser::new(markdown) {
        match event {
            Event::Text(value) | Event::Code(value) => {
                if !text.is_empty() && !text.ends_with(char::is_whitespace) {
                    text.push(' ');
                }
                text.push_str(&value);
            }
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            Event::End(_) => {
                if !text.ends_with('\n') {
                    text.push('\n');
                }
            }
            _ => {}
        }
    }

    ParsedDoc {
        doc_id,
        path,
        format,
        pages: vec![ParsedPage {
            page_num: None,
            text: text.trim().to_string(),
            images: Vec::new(),
        }],
        metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_markdown_str;
    use crate::types::{DocFormat, DocMetadata};

    #[test]
    fn parses_markdown_string_into_plain_text_page() {
        let metadata = DocMetadata {
            filename: "sample.md".to_string(),
            size_bytes: 64,
            hash: "hash-a".to_string(),
        };

        let doc = parse_markdown_str(
            "doc-1".to_string(),
            "D:\\knowledge\\sample.md".to_string(),
            "# Title\n\nA **bold** paragraph with [link](https://example.com).",
            metadata,
            DocFormat::Markdown,
        );

        assert_eq!(doc.doc_id, "doc-1");
        assert_eq!(doc.format, DocFormat::Markdown);
        assert_eq!(doc.pages.len(), 1);
        assert!(doc.pages[0].text.contains("Title"));
        assert!(doc.pages[0].text.contains("bold"));
        assert!(doc.pages[0].text.contains("link"));
        assert!(doc.pages[0].images.is_empty());
    }
}
