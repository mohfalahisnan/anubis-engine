use uuid::Uuid;

use crate::types::{Chunk, ParsedDoc};

pub const DEFAULT_WINDOW_SIZE: usize = 512;
pub const DEFAULT_OVERLAP: usize = 64;
pub const DEFAULT_MIN_CHUNK: usize = 50;

pub fn chunk_document(doc: &ParsedDoc) -> Vec<Chunk> {
    let mut full_text = String::new();
    let mut page_offsets = Vec::new();

    for page in &doc.pages {
        let start = full_text.chars().count();
        if !full_text.is_empty() {
            full_text.push('\n');
        }
        full_text.push_str(&page.text);
        let end = full_text.chars().count();
        page_offsets.push((start, end, page.page_num));
    }

    let mut chunks = chunk_text(
        &full_text,
        &doc.doc_id,
        DEFAULT_WINDOW_SIZE,
        DEFAULT_OVERLAP,
        DEFAULT_MIN_CHUNK,
    );

    for chunk in &mut chunks {
        chunk.page = page_offsets
            .iter()
            .find(|(start, end, _)| chunk.char_start >= *start && chunk.char_start < *end)
            .and_then(|(_, _, page)| *page);
    }

    chunks
}

pub fn chunk_text(
    text: &str,
    doc_id: &str,
    window_size: usize,
    overlap: usize,
    min_chunk: usize,
) -> Vec<Chunk> {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    if total < min_chunk || window_size == 0 {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < total {
        let hard_end = total.min(start + window_size);
        let end = sentence_boundary(&chars, start, hard_end, min_chunk).unwrap_or(hard_end);

        if end <= start {
            break;
        }

        if end - start >= min_chunk {
            let content: String = chars[start..end].iter().collect();
            chunks.push(Chunk {
                id: Uuid::new_v4().to_string(),
                doc_id: doc_id.to_string(),
                chunk_index: chunks.len(),
                content,
                char_start: start,
                char_end: end,
                page: None,
            });
        }

        if end == total {
            break;
        }

        let next_start = end.saturating_sub(overlap);
        if next_start <= start {
            start = end;
        } else {
            start = next_start;
        }
    }

    chunks
}

fn sentence_boundary(
    chars: &[char],
    start: usize,
    hard_end: usize,
    min_chunk: usize,
) -> Option<usize> {
    let min_end = start + min_chunk;
    let mut best = None;

    for index in start..hard_end {
        let current = chars[index];
        let next = chars.get(index + 1).copied();
        if index + 1 >= min_end
            && matches!(current, '.' | '!' | '?')
            && next.map(char::is_whitespace).unwrap_or(true)
        {
            best = Some(index + 1);
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::chunk_text;

    #[test]
    fn drops_chunks_smaller_than_minimum() {
        let chunks = chunk_text("Short sentence.", "doc-1", 512, 64, 50);

        assert!(chunks.is_empty());
    }

    #[test]
    fn records_overlap_boundaries_between_chunks() {
        let text = format!(
            "{}. {}. {}.",
            "a".repeat(240),
            "b".repeat(240),
            "c".repeat(240)
        );

        let chunks = chunk_text(&text, "doc-1", 512, 64, 50);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].char_start, 0);
        assert_eq!(chunks[1].char_start, chunks[0].char_end.saturating_sub(64));
        assert!(chunks[0].char_end <= 512);
        assert!(chunks[1].content.len() >= 50);
    }
}
