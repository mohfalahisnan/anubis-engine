use std::path::Path;

use lopdf::Document;
use uuid::Uuid;

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let document = Document::load(path).map_err(|error| EngineError::Parse {
        path: path.to_string_lossy().into_owned(),
        msg: error.to_string(),
    })?;
    let metadata = metadata_for_path(path)?;
    let mut pages = Vec::new();

    for page_num in document.get_pages().keys() {
        let text = document
            .extract_text(&[*page_num])
            .map_err(|error| EngineError::Parse {
                path: path.to_string_lossy().into_owned(),
                msg: error.to_string(),
            })?;
        pages.push(ParsedPage {
            page_num: Some(*page_num),
            text,
            images: Vec::new(),
        });
    }

    Ok(ParsedDoc {
        doc_id: Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Pdf,
        pages,
        metadata,
        doc_class: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::types::DocFormat;
    use std::path::PathBuf;

    #[test]
    fn parses_text_pdf_fixture() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.pdf");

        let doc = parse(&fixture).expect("parse pdf fixture");

        assert_eq!(doc.format, DocFormat::Pdf);
        assert_eq!(doc.pages.len(), 1);
        assert_eq!(doc.pages[0].page_num, Some(1));
        assert!(doc.pages[0].text.contains("Anubis sample PDF text"));
        assert!(doc.pages[0].images.is_empty());
    }
}
