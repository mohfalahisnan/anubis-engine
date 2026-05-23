//! Lightweight, dependency-free entity extraction.
//!
//! Pulls four kinds of signals out of chunk text:
//! - `ANCHOR`: structured all-caps IDs like `VID-APPROVAL-005`,
//!   `INC-2026-ATLAS-014`, `APPROVAL-Q-ATLAS`. These are the only entity
//!   type that produces high-confidence cross-doc relations (weight 0.9).
//! - `DATE`: numeric date patterns (DD/MM/YY[YY], DD-MM-YY[YY])
//! - `PROPER`: capitalized tokens that aren't sentence-starting common words
//! - `PHRASE`: capitalized or content bigrams
//! - `KEYWORD`: top-N most-frequent content tokens (rough TF) — never
//!   produces edges; kept for query-time exact match only.
//!
//! No NER model. Cheap, deterministic, good enough to seed `shared_entity`
//! edges across documents.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;

use crate::types::Chunk;

#[derive(Debug, Clone, PartialEq)]
pub struct EntityHit {
    pub chunk_id: String,
    pub entity_type: String,
    pub value: String,
    pub confidence: f32,
}

const KEYWORD_TOP_N: usize = 5;
const MIN_TOKEN_LEN: usize = 4;
const MAX_TOKEN_LEN: usize = 30;

pub fn extract_from_chunks(chunks: &[Chunk]) -> Vec<EntityHit> {
    let mut hits = Vec::new();
    for chunk in chunks {
        hits.extend(extract_anchors(&chunk.id, &chunk.content));
        hits.extend(extract_dates(&chunk.id, &chunk.content));
        hits.extend(extract_phrases(&chunk.id, &chunk.content));
        hits.extend(extract_proper_nouns(&chunk.id, &chunk.content));
        hits.extend(extract_keywords(&chunk.id, &chunk.content));
    }
    hits
}

/// Structural ID anchors: at least three ALL-CAPS-or-digit segments joined
/// by hyphens, total length ≤ 64. The first segment must start with a letter
/// (so we don't catch `2026-05-21` here — that's the DATE detector's job).
///
/// Accepts: `VID-APPROVAL-005`, `INC-2026-ATLAS-014`, `APPROVAL-Q-ATLAS`,
/// `SHIP-NODE-SURYA`.
/// Rejects: `Anubis` (single token), `INC-014` (only 2 segments),
/// `Q1-2026` (first segment too short / shape doesn't match).
///
/// Capped at `MAX_ANCHORS_PER_CHUNK` unique anchors per chunk: a manifest
/// row listing hundreds of IDs in one line shouldn't produce hundreds of
/// entity-table writes and a quadratic explosion in the cross-doc edge
/// builder. Twenty is well above the realistic density of anchors per
/// chunk of prose; a single chunk that needs more is almost certainly a
/// listing/index file (`doc_class='reference'`).
const MAX_ANCHORS_PER_CHUNK: usize = 20;

fn extract_anchors(chunk_id: &str, text: &str) -> Vec<EntityHit> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // \b[A-Z][A-Z0-9]+(?:-[A-Z0-9]+){2,}\b
        Regex::new(r"\b[A-Z][A-Z0-9]+(?:-[A-Z0-9]+){2,}\b").expect("anchor regex compiles")
    });

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in re.find_iter(text) {
        let value = m.as_str();
        if value.len() > 64 {
            continue;
        }
        if !seen.insert(value.to_string()) {
            continue;
        }
        out.push(EntityHit {
            chunk_id: chunk_id.to_string(),
            entity_type: "ANCHOR".to_string(),
            value: value.to_string(),
            confidence: 1.0,
        });
        if out.len() >= MAX_ANCHORS_PER_CHUNK {
            break;
        }
    }
    out
}

fn extract_dates(chunk_id: &str, text: &str) -> Vec<EntityHit> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if let Some((value, consumed)) = match_date(&bytes[i..]) {
            out.push(EntityHit {
                chunk_id: chunk_id.to_string(),
                entity_type: "DATE".to_string(),
                value,
                confidence: 1.0,
            });
            i += consumed;
        } else {
            i += 1;
        }
    }
    out
}

/// Match `D[D]<sep>D[D]<sep>YY[YY]` where sep is `/` or `-`.
fn match_date(window: &[u8]) -> Option<(String, usize)> {
    let mut idx = 0;
    let d1 = take_digits(&window[idx..], 1, 2)?;
    idx += d1;
    let s1 = window.get(idx).copied()?;
    if s1 != b'/' && s1 != b'-' {
        return None;
    }
    idx += 1;
    let d2 = take_digits(&window[idx..], 1, 2)?;
    idx += d2;
    let s2 = window.get(idx).copied()?;
    if s2 != s1 {
        return None;
    }
    idx += 1;
    let d3 = take_digits(&window[idx..], 2, 4)?;
    idx += d3;
    // Must not be followed by another digit.
    if window.get(idx).map(|b| b.is_ascii_digit()).unwrap_or(false) {
        return None;
    }
    let raw = &window[..idx];
    Some((String::from_utf8_lossy(raw).into_owned(), idx))
}

fn take_digits(window: &[u8], min: usize, max: usize) -> Option<usize> {
    let mut n = 0;
    while n < max && window.get(n).map(|b| b.is_ascii_digit()).unwrap_or(false) {
        n += 1;
    }
    if n >= min {
        Some(n)
    } else {
        None
    }
}

fn extract_proper_nouns(chunk_id: &str, text: &str) -> Vec<EntityHit> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut at_sentence_start = true;

    for token in tokens(text) {
        let is_proper = token
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false)
            && token.len() >= MIN_TOKEN_LEN
            && token.len() <= MAX_TOKEN_LEN
            && token.chars().all(|c| c.is_alphabetic());

        if is_proper && !at_sentence_start && !STOPWORDS.contains(&token.to_lowercase().as_str()) {
            let value = token.to_string();
            if seen.insert(value.clone()) {
                out.push(EntityHit {
                    chunk_id: chunk_id.to_string(),
                    entity_type: "PROPER".to_string(),
                    value,
                    confidence: 0.7,
                });
            }
        }
        at_sentence_start = token.ends_with('.') || token.ends_with('!') || token.ends_with('?');
    }

    out
}

fn extract_keywords(chunk_id: &str, text: &str) -> Vec<EntityHit> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for token in tokens(text) {
        let lower = token.to_lowercase();
        if lower.len() < MIN_TOKEN_LEN || lower.len() > MAX_TOKEN_LEN {
            continue;
        }
        if STOPWORDS.contains(&lower.as_str()) {
            continue;
        }
        if !lower.chars().all(|c| c.is_alphabetic()) {
            continue;
        }
        *counts.entry(lower).or_insert(0) += 1;
    }
    let mut ranked: Vec<(String, u32)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(KEYWORD_TOP_N)
        .filter(|(_, count)| *count >= 2)
        .map(|(value, count)| EntityHit {
            chunk_id: chunk_id.to_string(),
            entity_type: "KEYWORD".to_string(),
            value,
            confidence: (count as f32 / 10.0).min(1.0),
        })
        .collect()
}

fn extract_phrases(chunk_id: &str, text: &str) -> Vec<EntityHit> {
    let mut seen = HashSet::new();
    let raw_tokens: Vec<&str> = tokens(text).collect();
    let mut out = Vec::new();

    for window in raw_tokens.windows(2) {
        let left = clean_token(window[0]);
        let right = clean_token(window[1]);
        if left.is_empty() || right.is_empty() {
            continue;
        }

        let capitalized_phrase = starts_upper(left) && starts_upper(right);
        let content_phrase = is_content_term(left) && is_content_term(right);
        if !capitalized_phrase && !content_phrase {
            continue;
        }

        let phrase = format!("{} {}", left, right);
        let key = phrase.to_lowercase();
        if seen.insert(key) {
            out.push(EntityHit {
                chunk_id: chunk_id.to_string(),
                entity_type: "PHRASE".to_string(),
                value: phrase,
                confidence: if capitalized_phrase { 0.85 } else { 0.65 },
            });
        }
        if out.len() >= 8 {
            break;
        }
    }

    out
}

fn clean_token(token: &str) -> &str {
    token.trim_matches(|ch: char| !ch.is_alphanumeric())
}

fn starts_upper(token: &str) -> bool {
    token
        .chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
}

fn is_content_term(token: &str) -> bool {
    let lower = token.to_lowercase();
    lower.len() >= 4
        && lower.len() <= MAX_TOKEN_LEN
        && lower.chars().all(|ch| ch.is_alphabetic())
        && !STOPWORDS.contains(&lower.as_str())
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric() && c != '.' && c != '\'')
        .filter(|t| !t.is_empty())
}

// A tiny stopword list — keep small; this is not an NLP product.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "of", "for", "to", "in", "on", "at", "by", "with",
    "from", "as", "is", "are", "was", "were", "be", "been", "this", "that", "these", "those", "it",
    "its", "we", "you", "they", "them", "i", "he", "she", "his", "her", "their", "our", "yours",
    "have", "has", "had", "will", "would", "could", "should", "may", "might", "do", "does", "did",
    "not", "no", "yes", "so", "than", "then", "there", "here", "when", "where", "what", "who",
    "which", "how", "why", "all", "any", "each", "more", "most", "some", "such", "only", "own",
    "same", "very", "just", "into", "about", "after", "before", "between", "during", "while",
    "also", "must", "your", "yours", "mine", "ours",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &str, content: &str) -> Chunk {
        Chunk {
            id: id.to_string(),
            doc_id: "doc".to_string(),
            chunk_index: 0,
            content: content.to_string(),
            char_start: 0,
            char_end: content.len(),
            page: None,
            signal: crate::types::ChunkSignal::Content,
        }
    }

    #[test]
    fn extracts_dates_in_common_formats() {
        let hits =
            extract_from_chunks(&[chunk("c1", "Meeting on 21/05/2026 then 1-1-2027 outcomes.")]);
        let dates: Vec<&str> = hits
            .iter()
            .filter(|h| h.entity_type == "DATE")
            .map(|h| h.value.as_str())
            .collect();
        assert!(dates.contains(&"21/05/2026"));
        assert!(dates.contains(&"1-1-2027"));
    }

    #[test]
    fn skips_sentence_starting_capitals() {
        let hits = extract_from_chunks(&[chunk("c1", "Apple ships fast. Beans cost more.")]);
        let proper: Vec<&str> = hits
            .iter()
            .filter(|h| h.entity_type == "PROPER")
            .map(|h| h.value.as_str())
            .collect();
        assert!(
            proper.is_empty(),
            "sentence-initial caps should not become PROPER: {proper:?}"
        );
    }

    #[test]
    fn detects_mid_sentence_proper_nouns() {
        let hits = extract_from_chunks(&[chunk(
            "c1",
            "We ship the Anubis engine for Indonesia today.",
        )]);
        let proper: Vec<&str> = hits
            .iter()
            .filter(|h| h.entity_type == "PROPER")
            .map(|h| h.value.as_str())
            .collect();
        assert!(proper.contains(&"Anubis"));
        assert!(proper.contains(&"Indonesia"));
    }

    #[test]
    fn extracts_known_anchor_id_shapes() {
        let hits = extract_from_chunks(&[chunk(
            "c1",
            "Linked tickets: VID-APPROVAL-005 and INC-2026-ATLAS-014 plus APPROVAL-Q-ATLAS \
             and the SHIP-NODE-SURYA route.",
        )]);
        let anchors: Vec<&str> = hits
            .iter()
            .filter(|h| h.entity_type == "ANCHOR")
            .map(|h| h.value.as_str())
            .collect();
        for want in [
            "VID-APPROVAL-005",
            "INC-2026-ATLAS-014",
            "APPROVAL-Q-ATLAS",
            "SHIP-NODE-SURYA",
        ] {
            assert!(
                anchors.contains(&want),
                "expected anchor {want} in {anchors:?}",
            );
        }
    }

    #[test]
    fn caps_anchors_per_chunk_to_defend_against_listing_files() {
        // 50 unique anchors in one line — must cap at 20 so a manifest row
        // listing every ticket in the project can't blow up the entity
        // table.
        let mut text = String::with_capacity(50 * 20);
        for i in 0..50 {
            text.push_str(&format!("INC-2026-ATLAS-{i:03} "));
        }
        let hits = extract_from_chunks(&[chunk("c1", &text)]);
        let anchors: Vec<_> = hits.iter().filter(|h| h.entity_type == "ANCHOR").collect();
        assert!(
            anchors.len() <= super::MAX_ANCHORS_PER_CHUNK,
            "expected ≤ {} anchors, got {}",
            super::MAX_ANCHORS_PER_CHUNK,
            anchors.len()
        );
    }

    #[test]
    fn rejects_non_anchor_shapes() {
        // `Anubis` is mixed case (single token), `INC-014` only has 2 segments,
        // `Q1-2026` doesn't satisfy the leading letter+letter/digit shape with
        // 3 segments, lowercase IDs must not match.
        let hits = extract_from_chunks(&[chunk(
            "c1",
            "Anubis briefing for INC-014 on Q1-2026 see also vid-approval-005.",
        )]);
        let anchors: Vec<&str> = hits
            .iter()
            .filter(|h| h.entity_type == "ANCHOR")
            .map(|h| h.value.as_str())
            .collect();
        assert!(anchors.is_empty(), "no false-positive anchors: {anchors:?}");
    }

    #[test]
    fn extracts_capitalized_and_content_phrases() {
        let hits = extract_from_chunks(&[chunk(
            "c1",
            "Anubis OS indexes thermal printer manuals for support.",
        )]);
        let phrases: Vec<&str> = hits
            .iter()
            .filter(|h| h.entity_type == "PHRASE")
            .map(|h| h.value.as_str())
            .collect();

        assert!(phrases.contains(&"Anubis OS"));
        assert!(phrases.contains(&"thermal printer"));
    }
}
