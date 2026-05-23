use std::path::Path;

use serde_json::Value;

use crate::{
    parser::metadata_for_path,
    types::{DocFormat, ParsedDoc, ParsedPage},
    EngineError,
};

/// Soft cap on a single JSON page's flattened text. Bounds the per-page
/// chunk count so a 5 MB JSON doesn't produce one ten-thousand-chunk page
/// that pegs the indexer for minutes. Large sub-trees are split into
/// sibling pages along the next-deepest structural boundary.
///
/// 16 KiB is generous — well above what fits in a screenful of context but
/// small enough that any single page produces at most ~32 sliding chunks.
pub const JSON_PAGE_MAX_CHARS: usize = 16 * 1024;

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let content = std::fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&content).map_err(|error| EngineError::Parse {
        path: path.to_string_lossy().into_owned(),
        msg: error.to_string(),
    })?;
    let metadata = metadata_for_path(path)?;

    let pages = paginate(&value);

    Ok(ParsedDoc {
        doc_id: uuid::Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        format: DocFormat::Json,
        pages,
        metadata,
        doc_class: Default::default(),
    })
}

/// Split a JSON document into multiple pages, each ≤ `JSON_PAGE_MAX_CHARS`.
///
/// Strategy: walk top-level keys (for an object) or top-level array
/// elements. Each yields a starting "page text" that gets greedily packed
/// with siblings until the cap is hit, at which point a new page opens.
/// Sub-trees that on their own exceed the cap are split recursively along
/// their next-deepest structural boundary, so we never emit one giant page
/// for a deeply nested object.
fn paginate(root: &Value) -> Vec<ParsedPage> {
    let mut pages = Vec::new();
    let mut current = String::new();
    let mut page_num: u32 = 1;

    let emit = |buf: &mut String, page_num: &mut u32, pages: &mut Vec<ParsedPage>| {
        if buf.trim().is_empty() {
            return;
        }
        pages.push(ParsedPage {
            page_num: Some(*page_num),
            text: buf.trim_end().to_string(),
            images: Vec::new(),
        });
        *page_num += 1;
        buf.clear();
    };

    // Top-level scalars / single small values just become one page.
    match root {
        Value::Object(map) if !map.is_empty() => {
            for (key, child) in map {
                let segments = segments_for(key, child);
                for segment in segments {
                    if !current.is_empty() && current.len() + segment.len() > JSON_PAGE_MAX_CHARS {
                        emit(&mut current, &mut page_num, &mut pages);
                    }
                    current.push_str(&segment);
                }
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (index, child) in items.iter().enumerate() {
                let segments = segments_for(&format!("[{index}]"), child);
                for segment in segments {
                    if !current.is_empty() && current.len() + segment.len() > JSON_PAGE_MAX_CHARS {
                        emit(&mut current, &mut page_num, &mut pages);
                    }
                    current.push_str(&segment);
                }
            }
        }
        other => {
            current.push_str(&flatten_scalar_or_empty("", other));
        }
    }

    emit(&mut current, &mut page_num, &mut pages);
    if pages.is_empty() {
        // Always return at least one page so the document is discoverable
        // by filename even when it's structurally empty.
        pages.push(ParsedPage {
            page_num: Some(1),
            text: String::new(),
            images: Vec::new(),
        });
    }
    pages
}

/// Return one-or-more text fragments representing this `(path, value)`
/// pair. A single fragment for small sub-trees; if `value` itself is large
/// and structurally splittable, multiple fragments — each safe to drop
/// onto a page on its own.
fn segments_for(path: &str, value: &Value) -> Vec<String> {
    let mut whole = String::new();
    flatten_value(path, value, &mut whole);
    if whole.len() <= JSON_PAGE_MAX_CHARS {
        return vec![whole];
    }

    // Too big to live on one page. Split along the next-deepest boundary.
    match value {
        Value::Object(map) if !map.is_empty() => {
            let mut out = Vec::with_capacity(map.len());
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                out.extend(segments_for(&child_path, child));
            }
            out
        }
        Value::Array(items) if !items.is_empty() => {
            let mut out = Vec::with_capacity(items.len());
            for (index, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                out.extend(segments_for(&child_path, child));
            }
            out
        }
        // A scalar bigger than JSON_PAGE_MAX_CHARS (a giant base64 blob, say).
        // We can't structurally split it; just chunk the rendered text.
        // Returning it as-is lets the sliding chunker handle it; an oversized
        // page is far better than a stuck process.
        _ => vec![whole],
    }
}

fn flatten_value(path: &str, value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            if map.is_empty() && !path.is_empty() {
                write_scalar(path, "{}", out);
            }
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                flatten_value(&child_path, child, out);
            }
        }
        Value::Array(items) => {
            if items.is_empty() && !path.is_empty() {
                write_scalar(path, "[]", out);
            }
            for (index, child) in items.iter().enumerate() {
                let child_path = if path.is_empty() {
                    format!("[{index}]")
                } else {
                    format!("{path}[{index}]")
                };
                flatten_value(&child_path, child, out);
            }
        }
        Value::String(value) => write_scalar(path, value, out),
        Value::Number(value) => write_scalar(path, &value.to_string(), out),
        Value::Bool(value) => write_scalar(path, if *value { "true" } else { "false" }, out),
        Value::Null => write_scalar(path, "null", out),
    }
}

fn flatten_scalar_or_empty(path: &str, value: &Value) -> String {
    let mut buf = String::new();
    flatten_value(path, value, &mut buf);
    buf
}

fn write_scalar(path: &str, value: &str, out: &mut String) {
    if path.is_empty() {
        out.push_str("value");
    } else {
        out.push_str(path);
    }
    out.push_str(": ");
    out.push_str(value);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{paginate, JSON_PAGE_MAX_CHARS};
    use serde_json::json;

    #[test]
    fn small_json_returns_single_page() {
        let value = json!({
            "incident": "INC-2026-ATLAS-014",
            "nodes": [{ "id": "BOT-PACK-7", "role": "packing robot" }]
        });
        let pages = paginate(&value);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].text.contains("incident: INC-2026-ATLAS-014"));
        assert!(pages[0].text.contains("nodes[0].id: BOT-PACK-7"));
    }

    #[test]
    fn large_json_splits_into_multiple_pages_each_bounded() {
        // Build a JSON object large enough to exceed several page budgets.
        // 2000 entries × ~50 chars per flattened line ≈ 100 KB > 16 KB.
        let mut map = serde_json::Map::new();
        for i in 0..2000 {
            map.insert(
                format!("entry_{i:05}"),
                json!({ "id": format!("INC-2026-ATLAS-{i:05}"), "note": "alpha bravo charlie" }),
            );
        }
        let pages = paginate(&serde_json::Value::Object(map));

        assert!(
            pages.len() > 1,
            "expected pagination to fire; got 1 page of {} chars",
            pages[0].text.len()
        );
        for (i, page) in pages.iter().enumerate() {
            assert!(
                page.text.len() <= JSON_PAGE_MAX_CHARS + 256, // small slack for last segment
                "page {i} text length {} exceeds budget {}",
                page.text.len(),
                JSON_PAGE_MAX_CHARS
            );
            assert!(!page.text.trim().is_empty());
        }
    }

    #[test]
    fn deeply_nested_subtree_still_paginates() {
        // One top-level key whose value alone exceeds the budget.
        let inner: Vec<_> = (0..500)
            .map(|i| json!({ "key": format!("PROJ-{i:04}-INC-001"), "v": "lorem ipsum dolor" }))
            .collect();
        let root = json!({ "big": inner });

        let pages = paginate(&root);
        assert!(pages.len() > 1, "deeply-nested big subtree must split");
    }

    #[test]
    fn empty_root_returns_one_empty_page() {
        let pages = paginate(&serde_json::Value::Object(serde_json::Map::new()));
        assert_eq!(pages.len(), 1);
    }
}
