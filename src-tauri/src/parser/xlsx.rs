use std::path::Path;

use calamine::{open_workbook_auto, Reader};

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let mut workbook = open_workbook_auto(path).map_err(|error| EngineError::Parse {
        path: path.to_string_lossy().into_owned(),
        msg: error.to_string(),
    })?;
    let metadata = metadata_for_path(path)?;
    let mut text = String::new();

    for sheet_name in workbook.sheet_names().to_owned() {
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|error| EngineError::Parse {
                path: path.to_string_lossy().into_owned(),
                msg: error.to_string(),
            })?;
        text.push_str(&format!("[{}]\n", sheet_name));
        for row in range.rows() {
            let line = row
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\t");
            text.push_str(&line);
            text.push('\n');
        }
    }

    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Xlsx,
        pages: vec![ParsedPage {
            page_num: None,
            text: text.trim().to_string(),
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
