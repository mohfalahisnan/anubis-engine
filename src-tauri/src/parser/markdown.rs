use std::path::Path;

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use uuid::Uuid;

use crate::{
    parser::{format_from_path, metadata_for_path},
    types::{DocFormat, DocMetadata, ParsedDoc, ParsedPage},
    EngineError,
};

/// Headings up to this level become hard section boundaries — chunks won't
/// cross them. Deeper headings (H3+) are folded into the surrounding section
/// so we don't fragment too aggressively.
const SECTION_HEADING_MAX_LEVEL: HeadingLevel = HeadingLevel::H2;

pub fn parse(path: &Path) -> Result<ParsedDoc, EngineError> {
    let content = std::fs::read_to_string(path)?;
    let metadata = metadata_for_path(path)?;
    Ok(parse_markdown_str(
        Uuid::new_v4().to_string(),
        path.to_string_lossy().into_owned(),
        &content,
        metadata,
        format_from_path(path),
    ))
}

pub fn parse_markdown_str(
    doc_id: String,
    path: String,
    markdown: &str,
    metadata: DocMetadata,
    format: DocFormat,
) -> ParsedDoc {
    let sections = split_sections(markdown);

    let pages: Vec<ParsedPage> = sections
        .into_iter()
        .filter(|section| !section.body.trim().is_empty() || !section.heading_path.is_empty())
        .map(|section| {
            let mut text = String::new();
            if !section.heading_path.is_empty() {
                // Repeat the heading path at the top of every section so each
                // chunk carries its hierarchical context — critical for AI
                // retrieval that otherwise loses structural cues.
                text.push_str(&section.heading_path.join(" > "));
                text.push('\n');
                text.push('\n');
            }
            text.push_str(section.body.trim());
            ParsedPage {
                page_num: None,
                text,
                images: Vec::new(),
            }
        })
        .collect();

    let pages = if pages.is_empty() {
        vec![ParsedPage {
            page_num: None,
            text: String::new(),
            images: Vec::new(),
        }]
    } else {
        pages
    };

    ParsedDoc {
        doc_id,
        path,
        format,
        pages,
        metadata,
        doc_class: Default::default(),
    }
}

#[derive(Debug)]
struct Section {
    heading_path: Vec<String>,
    body: String,
}

fn split_sections(markdown: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut heading_stack: Vec<(HeadingLevel, String)> = Vec::new();

    // The "in-progress" body buffer and its heading path snapshot.
    let mut current_body = String::new();
    let mut current_path: Vec<String> = Vec::new();

    let mut in_heading: Option<HeadingLevel> = None;
    let mut heading_buf = String::new();

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                // Only section-level headings (H1/H2) create a hard break.
                // Deeper headings are folded into the surrounding body so a
                // dense doc full of H3/H4 doesn't fragment.
                if (level as u8) <= (SECTION_HEADING_MAX_LEVEL as u8)
                    && (!current_body.trim().is_empty() || !current_path.is_empty())
                {
                    sections.push(Section {
                        heading_path: current_path.clone(),
                        body: std::mem::take(&mut current_body),
                    });
                }
                in_heading = Some(level);
                heading_buf.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let level = in_heading.take().unwrap_or(HeadingLevel::H6);
                let heading_text = heading_buf.trim().to_string();
                heading_buf.clear();

                // Maintain a hierarchical heading stack so the path reflects
                // ancestor headings (H1 > H2). Pop until we reach a strictly
                // smaller level, then push.
                while let Some((top_level, _)) = heading_stack.last() {
                    if *top_level as u8 >= level as u8 {
                        heading_stack.pop();
                    } else {
                        break;
                    }
                }
                heading_stack.push((level, heading_text));

                if (level as u8) <= (SECTION_HEADING_MAX_LEVEL as u8) {
                    current_path = heading_stack.iter().map(|(_, text)| text.clone()).collect();
                } else {
                    // Deeper heading: keep as inline body so the section
                    // doesn't fragment.
                    if !current_body.is_empty() && !current_body.ends_with('\n') {
                        current_body.push('\n');
                    }
                    let last = heading_stack.last().map(|(_, t)| t.as_str()).unwrap_or("");
                    current_body.push_str(last);
                    current_body.push('\n');
                }
            }
            Event::Text(value) | Event::Code(value) => {
                if in_heading.is_some() {
                    heading_buf.push_str(&value);
                } else {
                    if !current_body.is_empty() && !current_body.ends_with(char::is_whitespace) {
                        current_body.push(' ');
                    }
                    current_body.push_str(&value);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_heading.is_some() {
                    heading_buf.push(' ');
                } else {
                    current_body.push('\n');
                }
            }
            Event::End(_) => {
                if in_heading.is_none() && !current_body.ends_with('\n') {
                    current_body.push('\n');
                }
            }
            _ => {}
        }
    }

    if !current_body.trim().is_empty() || !current_path.is_empty() {
        sections.push(Section {
            heading_path: current_path,
            body: current_body,
        });
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::{parse_markdown_str, split_sections};
    use crate::types::{DocFormat, DocMetadata};

    fn meta() -> DocMetadata {
        DocMetadata {
            filename: "sample.md".to_string(),
            size_bytes: 64,
            hash: "hash-a".to_string(),
        }
    }

    #[test]
    fn parses_markdown_string_into_plain_text_page() {
        let doc = parse_markdown_str(
            "doc-1".to_string(),
            "D:\\knowledge\\sample.md".to_string(),
            "# Title\n\nA **bold** paragraph with [link](https://example.com).",
            meta(),
            DocFormat::Markdown,
        );

        assert_eq!(doc.doc_id, "doc-1");
        assert_eq!(doc.format, DocFormat::Markdown);
        assert_eq!(doc.pages.len(), 1);
        assert!(doc.pages[0].text.contains("Title"));
        assert!(doc.pages[0].text.contains("bold"));
        assert!(doc.pages[0].text.contains("link"));
        assert!(doc.pages[0].images.is_empty());
    }

    #[test]
    fn splits_each_h2_into_its_own_page_with_heading_context() {
        let markdown = "\
# Project

Intro paragraph.

## Setup

Install deps.

## Usage

Run the CLI.
";
        let doc = parse_markdown_str(
            "doc-1".to_string(),
            "x.md".to_string(),
            markdown,
            meta(),
            DocFormat::Markdown,
        );

        // 3 sections: Project (intro), Setup, Usage.
        assert_eq!(doc.pages.len(), 3);
        assert!(doc.pages[1].text.starts_with("Project > Setup"));
        assert!(doc.pages[1].text.contains("Install deps"));
        assert!(doc.pages[2].text.starts_with("Project > Usage"));
    }

    #[test]
    fn deep_headings_do_not_create_extra_pages() {
        let markdown = "\
# Top

Body.

### Inline subheading

More body.
";
        let sections = split_sections(markdown);
        assert_eq!(sections.len(), 1);
        assert!(sections[0].body.contains("Body."));
        assert!(sections[0].body.contains("Inline subheading"));
        assert!(sections[0].body.contains("More body."));
    }
}
