use std::path::Path;

use crate::error::{Error, Result};
use crate::models::{sha256_hex, ToolKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontMatter {
    pub id: String,
    pub tool: ToolKind,
    pub title: String,
    pub tags: Vec<String>,
    pub version: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownPrompt {
    pub front: FrontMatter,
    pub content: String,
}

pub fn parse_markdown(text: &str) -> Result<MarkdownPrompt> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or_else(|| Error::invalid("prompt markdown is missing front matter"))?;
    let end = rest
        .find("\n---\n")
        .or_else(|| rest.find("\n---\r\n"))
        .ok_or_else(|| Error::invalid("prompt markdown front matter is not closed"))?;
    let header = &rest[..end];
    let body = rest[end + 5..].trim_start_matches('\n').to_string();

    let mut id = None;
    let mut tool = None;
    let mut title = None;
    let mut tags = Vec::new();
    let mut version = 1_i64;
    let mut deleted = false;

    for raw_line in header.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = split_kv(line)?;
        match key {
            "id" => id = Some(value.to_string()),
            "tool" => tool = Some(value.parse()?),
            "title" => title = Some(value.to_string()),
            "tags" => tags = parse_tags(value),
            "version" => {
                version = value
                    .parse()
                    .map_err(|_| Error::invalid(format!("invalid version: {value}")))?;
            }
            "deleted" => deleted = parse_bool(value),
            _ => {}
        }
    }

    let id = id.ok_or_else(|| Error::invalid("front matter is missing id"))?;
    let tool = tool.ok_or_else(|| Error::invalid("front matter is missing tool"))?;
    let title = title.unwrap_or_else(|| id.clone());
    Ok(MarkdownPrompt {
        front: FrontMatter {
            id,
            tool,
            title,
            tags,
            version,
            deleted,
        },
        content: body,
    })
}

pub fn render_markdown(front: &FrontMatter, content: &str) -> String {
    let tags = if front.tags.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            front
                .tags
                .iter()
                .map(|tag| tag.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("id: {}\n", front.id));
    out.push_str(&format!("tool: {}\n", front.tool.as_str()));
    out.push_str(&format!("title: {}\n", front.title));
    out.push_str(&format!("tags: {tags}\n"));
    out.push_str(&format!("version: {}\n", front.version));
    if front.deleted {
        out.push_str("deleted: true\n");
    }
    out.push_str("---\n\n");
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn parse_file(path: &Path) -> Result<MarkdownPrompt> {
    let text = std::fs::read_to_string(path)?;
    parse_markdown(&text)
}

pub fn content_sha(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

fn split_kv(line: &str) -> Result<(&str, &str)> {
    let (key, value) = line
        .split_once(':')
        .ok_or_else(|| Error::invalid(format!("invalid front matter line: {line}")))?;
    Ok((key.trim(), value.trim()))
}

fn parse_tags(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_bool(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "1"
    )
}
