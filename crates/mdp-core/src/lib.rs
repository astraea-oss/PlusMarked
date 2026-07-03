use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFrontmatter {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, deserialize_with = "deserialize_tags", serialize_with = "serialize_tags")]
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

    let normalized_yaml = normalize_hashtag_tag_line(yaml);
    let frontmatter = noyalib::from_str::<NoteFrontmatter>(&normalized_yaml)
        .map_err(|error| MarkdownPlusError::InvalidYaml(error.to_string()))?;

    Ok(NoteDocument { frontmatter, body })
}

pub fn serialize_document(document: &NoteDocument) -> Result<String> {
    let yaml = noyalib::to_string(&document.frontmatter)
        .map_err(|error| MarkdownPlusError::SerializeYaml(error.to_string()))?;
    let yaml = serialize_hashtag_tag_line(yaml.trim_end(), &document.frontmatter.tags);

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

fn normalize_hashtag_tag_line(yaml: &str) -> String {
    yaml.lines()
        .map(|line| {
            let Some((key, value)) = line.split_once(':') else {
                return line.to_string();
            };

            if key.trim() != "tags" || !value.trim_start().starts_with('#') {
                return line.to_string();
            }

            format!("{key}: {}", serde_json::to_string(value.trim()).unwrap_or_else(|_| "\"\"".to_string()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_hashtag_tag_line(yaml: &str, tags: &[String]) -> String {
    yaml.lines()
        .map(|line| {
            let Some((key, _value)) = line.split_once(':') else {
                return line.to_string();
            };

            if key.trim() != "tags" || tags.is_empty() {
                return line.to_string();
            }

            format!("{key}: {}", format_hashtag_tags(tags))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_hashtag_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!("#{}", normalize_tag_token(tag)))
        .filter(|tag| tag.len() > 1)
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_tag_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| character == ',' || character.is_whitespace())
        .map(normalize_tag_token)
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn normalize_tag_token(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_string()
}

fn serialize_tags<S>(tags: &[String], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if tags.is_empty() {
        tags.serialize(serializer)
    } else {
        serializer.serialize_str(&format_hashtag_tags(tags))
    }
}

fn deserialize_tags<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(TagListVisitor)
}

struct TagListVisitor;

impl<'de> Visitor<'de> for TagListVisitor {
    type Value = Vec<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a hashtag string or a sequence of tag strings")
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(parse_tag_tokens(value))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(parse_tag_tokens(&value))
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Vec::new())
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Vec::new())
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut tags = Vec::new();
        while let Some(value) = sequence.next_element::<String>()? {
            tags.extend(parse_tag_tokens(&value));
        }
        Ok(tags)
    }
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
    fn parses_hashtag_frontmatter_tags() {
        let input = r#"---
id: "018ff6e2-9f4b-7a64-b101-0e2fd6e32f20"
title: "Example"
created_at: "2026-06-28T23:30:00Z"
updated_at: "2026-06-28T23:30:00Z"
tags: #Project, #ClientWork
aliases: []
type: "note"
---

Body text
"#;

        let document = parse_document(input).unwrap();

        assert_eq!(document.frontmatter.tags, vec!["Project", "ClientWork"]);
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

    #[test]
    fn serializes_tags_as_hashtag_tokens() {
        let document = new_note("Example".to_string(), None);
        let document = update_note(
            document,
            "Example".to_string(),
            "note".to_string(),
            vec!["Project".to_string(), "ClientWork".to_string()],
            Vec::new(),
            String::new(),
        );

        let serialized = serialize_document(&document).unwrap();

        assert!(serialized.contains("tags: #Project, #ClientWork"));
        assert!(!serialized.contains("tags: ["));
    }
}
