use std::path::Path;

use docx_rs::{read_docx, DocumentChild, ParagraphChild, RunChild};

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let bytes = std::fs::read(path)?;
    let docx = read_docx(&bytes).map_err(|error| EngineError::Parse {
        path: path.to_string_lossy().into_owned(),
        msg: error.to_string(),
    })?;
    let metadata = metadata_for_path(path)?;
    let mut text = String::new();

    for child in docx.document.children {
        if let DocumentChild::Paragraph(paragraph) = child {
            for paragraph_child in paragraph.children {
                if let ParagraphChild::Run(run) = paragraph_child {
                    for run_child in run.children {
                        match run_child {
                            RunChild::Text(value) => text.push_str(&value.text),
                            RunChild::InstrTextString(value) => text.push_str(&value),
                            RunChild::Tab(_) => text.push('\t'),
                            RunChild::Break(_) => text.push('\n'),
                            _ => {}
                        }
                    }
                }
            }
            text.push('\n');
        }
    }

    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Docx,
        pages: vec![ParsedPage {
            page_num: None,
            text: text.trim().to_string(),
            images: Vec::new(),
        }],
        metadata,
        doc_class: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
