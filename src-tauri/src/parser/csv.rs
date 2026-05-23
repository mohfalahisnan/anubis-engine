use std::path::Path;

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let metadata = metadata_for_path(path)?;
    let mut reader = ::csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|error| parse_error(path, error))?;
    let headers = reader
        .headers()
        .map_err(|error| parse_error(path, error))?
        .clone();
    let mut text = String::new();

    append_record(&mut text, 1, None, headers.iter());

    for (row_index, record) in reader.records().enumerate() {
        let record = record.map_err(|error| parse_error(path, error))?;
        append_record(&mut text, row_index + 2, Some(&headers), record.iter());
    }

    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Csv,
        pages: vec![ParsedPage {
            page_num: None,
            text: text.trim().to_string(),
            images: Vec::new(),
        }],
        metadata,
        doc_class: Default::default(),
    })
}

fn append_record<'a>(
    text: &mut String,
    row_number: usize,
    headers: Option<&::csv::StringRecord>,
    values: impl Iterator<Item = &'a str>,
) {
    let mut wrote_value = false;
    for (column_index, value) in values.enumerate() {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if !wrote_value {
            text.push_str(&format!("row {row_number}\n"));
            wrote_value = true;
        }
        let field_name = headers
            .and_then(|headers| headers.get(column_index))
            .filter(|header| !header.trim().is_empty())
            .map(str::trim)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("column_{}", column_index + 1));
        text.push_str(&field_name);
        text.push_str(": ");
        text.push_str(value);
        text.push('\n');
    }
    if wrote_value {
        text.push('\n');
    }
}

fn parse_error(path: &Path, error: ::csv::Error) -> EngineError {
    EngineError::Parse {
        path: path.to_string_lossy().into_owned(),
        msg: error.to_string(),
    }
}
