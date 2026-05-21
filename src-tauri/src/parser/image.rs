use std::path::Path;

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let metadata = metadata_for_path(path)?;
    let bytes = std::fs::read(path)?;
    let text = crate::ocr::engine::run(&bytes)?;
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
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
