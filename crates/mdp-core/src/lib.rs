use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFrontmatter {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "default_note_type", rename = "type")]
    pub note_type: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDocument {
    pub frontmatter: NoteFrontmatter,
    pub body: String,
}

#[derive(Debug, Error)]
pub enum MarkdownPlusError {
    #[error("MarkdownPlus document must start with YAML frontmatter delimited by ---")]
    MissingFrontmatter,
    #[error("MarkdownPlus document is missing a closing frontmatter delimiter")]
    UnclosedFrontmatter,
    #[error("frontmatter YAML is invalid: {0}")]
    InvalidYaml(String),
    #[error("frontmatter YAML could not be serialized: {0}")]
    SerializeYaml(String),
}

pub type Result<T> = std::result::Result<T, MarkdownPlusError>;

pub fn parse_document(input: &str) -> Result<NoteDocument> {
    let body_start = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))
        .ok_or(MarkdownPlusError::MissingFrontmatter)?;

    let delimiter = body_start
        .find("\n---")
        .ok_or(MarkdownPlusError::UnclosedFrontmatter)?;

    let yaml = &body_start[..delimiter];
    let after_delimiter = &body_start[delimiter + "\n---".len()..];
    let body = strip_frontmatter_body_gap(after_delimiter).to_string();

    let frontmatter = noyalib::from_str::<NoteFrontmatter>(yaml)
        .map_err(|error| MarkdownPlusError::InvalidYaml(error.to_string()))?;

    Ok(NoteDocument { frontmatter, body })
}

pub fn serialize_document(document: &NoteDocument) -> Result<String> {
    let yaml = noyalib::to_string(&document.frontmatter)
        .map_err(|error| MarkdownPlusError::SerializeYaml(error.to_string()))?;
    let yaml = yaml.trim_end();

    if document.body.is_empty() {
        Ok(format!("---\n{yaml}\n---\n"))
    } else {
        Ok(format!("---\n{yaml}\n---\n{}", document.body))
    }
}

pub fn new_note(title: String, note_type: Option<String>) -> NoteDocument {
    let now = Utc::now();
    NoteDocument {
        frontmatter: NoteFrontmatter {
            id: Uuid::now_v7(),
            title,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
            aliases: Vec::new(),
            note_type: note_type.unwrap_or_else(default_note_type),
            extra: BTreeMap::new(),
        },
        body: String::new(),
    }
}

pub fn update_note(
    mut document: NoteDocument,
    title: String,
    note_type: String,
    tags: Vec<String>,
    aliases: Vec<String>,
    body: String,
) -> NoteDocument {
    document.frontmatter.title = title;
    document.frontmatter.note_type = note_type;
    document.frontmatter.tags = tags;
    document.frontmatter.aliases = aliases;
    document.frontmatter.updated_at = Utc::now();
    document.body = body;
    document
}

fn default_note_type() -> String {
    "note".to_string()
}

fn strip_frontmatter_body_gap(input: &str) -> &str {
    input.trim_start_matches(['\r', '\n'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let input = r#"---
id: "018ff6e2-9f4b-7a64-b101-0e2fd6e32f20"
title: "Example"
created_at: "2026-06-28T23:30:00Z"
updated_at: "2026-06-28T23:30:00Z"
tags: ["project"]
aliases: []
type: "note"
---

Body text
"#;

        let document = parse_document(input).unwrap();

        assert_eq!(document.frontmatter.title, "Example");
        assert_eq!(document.frontmatter.tags, vec!["project"]);
        assert_eq!(document.body, "Body text\n");
    }

    #[test]
    fn serialize_then_parse_does_not_add_leading_blank_lines() {
        let document = new_note("Example".to_string(), None);
        let document = update_note(
            document,
            "Example".to_string(),
            "note".to_string(),
            Vec::new(),
            Vec::new(),
            "# Heading\n\nBody".to_string(),
        );

        let serialized = serialize_document(&document).unwrap();
        let parsed = parse_document(&serialized).unwrap();

        assert_eq!(parsed.body, "# Heading\n\nBody");
    }
}
