use std::path::Path;

use crate::{
    parser::metadata_for_path,
    transcription::engine::transcribe_file,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let metadata = metadata_for_path(path)?;
    let text = transcribe_file(path)?;
    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Video,
        pages: vec![ParsedPage {
            page_num: None,
            text,
            images: Vec::new(),
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
