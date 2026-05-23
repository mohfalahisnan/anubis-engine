use std::sync::OnceLock;

use regex::Regex;

use crate::types::ChunkSignal;

pub fn classify_chunk_signal(text: &str) -> ChunkSignal {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return ChunkSignal::Metadata;
    }

    let lower = compact.to_ascii_lowercase();
    let anchors = anchors_in(&compact);
    let anchor_count = anchors.len();
    let anchor_chars: usize = anchors.iter().map(|value| value.len()).sum();
    let non_anchor_words = non_anchor_word_count(&compact);
    let anchor_ratio = anchor_chars as f32 / compact.len().max(1) as f32;

    let has_metadata_label =
        lower.contains("title:") || lower.contains("tags:") || lower.contains("company:");
    if has_metadata_label
        && anchor_count >= 1
        && !has_sentence_like_prose(&compact)
        && (non_anchor_words <= 18 || anchor_ratio >= 0.25)
    {
        return ChunkSignal::Metadata;
    }

    let has_anchor_list_label = lower.contains("document anchors")
        || lower.contains("related anchors")
        || lower.contains("anchors:");
    if anchor_count >= 2
        && has_anchor_list_label
        && (non_anchor_words <= 14 || anchor_ratio >= 0.35)
    {
        return ChunkSignal::AnchorList;
    }

    ChunkSignal::Content
}

fn anchors_in(text: &str) -> Vec<&str> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\b[A-Z][A-Z0-9]+(?:-[A-Z0-9]+){2,}\b").expect("anchor regex compiles")
    });
    re.find_iter(text)
        .map(|m| m.as_str())
        .filter(|value| value.len() <= 64)
        .collect()
}

fn non_anchor_word_count(text: &str) -> usize {
    let without_anchors = anchor_regex().replace_all(text, " ");
    without_anchors
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|word| word.len() >= 4)
        .filter(|word| !LOW_SIGNAL_WORDS.contains(&word.to_ascii_lowercase().as_str()))
        .count()
}

fn anchor_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[A-Z][A-Z0-9]+(?:-[A-Z0-9]+){2,}\b").expect("anchor regex compiles")
    })
}

fn has_sentence_like_prose(text: &str) -> bool {
    text.split(['.', '!', '?'])
        .any(|sentence| non_anchor_word_count(sentence) >= 8)
}

const LOW_SIGNAL_WORDS: &[&str] = &[
    "anchor",
    "anchors",
    "document",
    "related",
    "title",
    "tags",
    "company",
    "cloud",
    "retail",
    "nusantara",
];

#[cfg(test)]
mod tests {
    use super::classify_chunk_signal;
    use crate::types::ChunkSignal;

    #[test]
    fn document_anchors_are_anchor_list() {
        let text =
            "Document anchors: NRC-COMPANY-001 ATLAS-POS-2026 INC-2026-ATLAS-014 APPROVAL-Q-ATLAS";

        assert_eq!(classify_chunk_signal(text), ChunkSignal::AnchorList);
    }

    #[test]
    fn related_anchors_section_is_anchor_list() {
        let text = "Atlas POS Product Brief > Related anchors\n\nATLAS-POS-2026\nINC-2026-ATLAS-014\nSKU-PRN-8842\nAPPROVAL-Q-ATLAS";

        assert_eq!(classify_chunk_signal(text), ChunkSignal::AnchorList);
    }

    #[test]
    fn prose_with_anchors_remains_content() {
        let text = "During INC-2026-ATLAS-014, several stores saw duplicate receipt printer callbacks related to SKU-PRN-8842. The mitigation was to hold approval in APPROVAL-Q-ATLAS until the warehouse count from WH-JKT-03 matched the register count.";

        assert_eq!(classify_chunk_signal(text), ChunkSignal::Content);
    }

    #[test]
    fn csv_or_ocr_rows_with_labels_remain_content() {
        let text = "row 2 route_id: SHIP-NODE-SURYA warehouse: WH-JKT-03 system: SURYA-FULFILL-2026 avg_delay_min: 11 linked_sku: SKU-PRN-8842 dashboard_card: DASHBOARD-CARD-KH";

        assert_eq!(classify_chunk_signal(text), ChunkSignal::Content);
    }

    #[test]
    fn frontmatter_style_anchor_metadata_is_metadata() {
        let text = "title: Incident Postmortem company: Nusantara Retail Cloud tags: INC-2026-ATLAS-014, ATLAS-POS-2026 anchors: APPROVAL-Q-ATLAS, SKU-PRN-8842";

        assert_eq!(classify_chunk_signal(text), ChunkSignal::Metadata);
    }
}
