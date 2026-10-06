//! Pointing Claude Code at the relay, and back.
//!
//! Linking owns exactly one value: `env.ANTHROPIC_BASE_URL` in the user's
//! `settings.json` (in `CLAUDE_CONFIG_DIR`, else `~/.claude`). The value it replaces, or the
//! fact that there was none, is kept in a link record, and the relay forwards to that address
//! (Anthropic's own when there was none). Unlinking puts it back only while the value still
//! holds what linking wrote. Nothing else in the file is touched: keys keep their order and
//! the file keeps its indentation.
//!
//! Prompt deploys (`claude-instruct.py`) carry other `env` keys through unchanged, so a deploy
//! or its undo leaves the link alone. A `restore` that copies a whole backup over the file
//! can drop it; that shows as bypassed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::paths::atomic_write;

pub const ENV_KEY: &str = "ANTHROPIC_BASE_URL";
pub const DEFAULT_UPSTREAM: &str = "https://api.anthropic.com";

/// Switches that send Claude Code somewhere other than `ANTHROPIC_BASE_URL`.
const CLOUD_SWITCHES: &[(&str, &str)] = &[
    ("CLAUDE_CODE_USE_BEDROCK", "bedrock"),
    ("CLAUDE_CODE_USE_VERTEX", "vertex"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeLinkRecord {
    pub claude_dir: PathBuf,
    /// `env.ANTHROPIC_BASE_URL` before linking; `None` when it was not set.
    pub previous: Option<String>,
    pub relay_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ClaudeLinkState {
    Linked,
    /// A link record exists but the settings no longer point at the relay.
    Bypassed,
    Unlinked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// The settings file is not a JSON object.
    BadSettings,
    Bedrock,
    Vertex,
    NotHttp,
}

impl Unsupported {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadSettings => "bad-settings",
            Self::Bedrock => "bedrock",
            Self::Vertex => "vertex",
            Self::NotHttp => "not-http",
        }
    }
}

/// Where Claude Code keeps its user settings.
pub fn claude_dir(home_override: Option<&Path>) -> Option<PathBuf> {
    if home_override.is_none() {
        if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
            if !dir.trim().is_empty() {
                return Some(PathBuf::from(dir.trim()));
            }
        }
    }
    let home = match home_override {
        Some(home) => home.to_path_buf(),
        None => dirs::home_dir()?,
    };
    Some(home.join(".claude"))
}

fn settings_path(claude_dir: &Path) -> PathBuf {
    claude_dir.join("settings.json")
}

/// The settings object, or an empty one when there is no file yet.
fn read_settings(claude_dir: &Path) -> std::result::Result<Map<String, Value>, Unsupported> {
    match std::fs::read(settings_path(claude_dir)) {
        Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Ok(Map::new()),
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(map)) => Ok(map),
            _ => Err(Unsupported::BadSettings),
        },
        Err(_) => Ok(Map::new()),
    }
}

fn env_value<'a>(settings: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    settings.get("env")?.as_object()?.get(key)?.as_str()
}

fn truthy(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim();
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

/// Where Claude Code sends requests now, or why the relay cannot stand in front of it.
pub fn current_upstream(claude_dir: &Path) -> std::result::Result<String, Unsupported> {
    let settings = read_settings(claude_dir)?;
    for (key, reason) in CLOUD_SWITCHES {
        let set = truthy(env_value(&settings, key))
            || (env_value(&settings, key).is_none() && truthy(std::env::var(key).ok().as_deref()));
        if set {
            return Err(if *reason == "bedrock" {
                Unsupported::Bedrock
            } else {
                Unsupported::Vertex
            });
        }
    }
    let base = env_value(&settings, ENV_KEY)
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_UPSTREAM.to_string());
    match url::Url::parse(&base) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(base),
        _ => Err(Unsupported::NotHttp),
    }
}

/// The host Claude Code reaches today, for the page. Never the full URL.
pub fn upstream_host(upstream: &str) -> Option<String> {
    url::Url::parse(upstream)
        .ok()?
        .host_str()
        .map(str::to_string)
}

pub fn state(record: Option<&ClaudeLinkRecord>, claude_dir: &Path) -> ClaudeLinkState {
    let Some(record) = record else {
        return ClaudeLinkState::Unlinked;
    };
    let linked = read_settings(claude_dir)
        .ok()
        .is_some_and(|settings| env_value(&settings, ENV_KEY) == Some(&record.relay_base_url));
    if linked {
        ClaudeLinkState::Linked
    } else {
        ClaudeLinkState::Bypassed
    }
}

/// Set `env.ANTHROPIC_BASE_URL` to the relay. Re-linking while linked keeps the value
/// recorded the first time.
pub fn link(
    claude_dir: &Path,
    previous: Option<&ClaudeLinkRecord>,
    relay_base_url: &str,
) -> Result<ClaudeLinkRecord> {
    let mut settings =
        read_settings(claude_dir).map_err(|reason| Error::unavailable(reason.code()))?;
    let now = env_value(&settings, ENV_KEY).map(str::to_string);
    let was = match previous {
        Some(record) if now.as_deref() == Some(record.relay_base_url.as_str()) => {
            record.previous.clone()
        }
        _ if now.as_deref() == Some(relay_base_url) => {
            return Err(Error::drift(
                "Claude Code already points at the relay but this app has no record of it",
            ))
        }
        _ => now,
    };
    let env = settings
        .entry("env")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| Error::invalid("env in settings.json is not an object"))?;
    env.insert(ENV_KEY.into(), Value::String(relay_base_url.into()));
    write_settings(claude_dir, &settings)?;
    Ok(ClaudeLinkRecord {
        claude_dir: claude_dir.to_path_buf(),
        previous: was,
        relay_base_url: relay_base_url.into(),
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUnlinkReport {
    pub restored: bool,
    /// True when another tool changed the value after linking, so it was left as it is.
    pub left_alone: bool,
}

/// Put back the value linking replaced, only while the relay address is still there.
pub fn unlink(record: &ClaudeLinkRecord) -> Result<ClaudeUnlinkReport> {
    let Ok(mut settings) = read_settings(&record.claude_dir) else {
        return Ok(ClaudeUnlinkReport {
            restored: false,
            left_alone: true,
        });
    };
    if env_value(&settings, ENV_KEY) != Some(record.relay_base_url.as_str()) {
        return Ok(ClaudeUnlinkReport {
            restored: false,
            left_alone: true,
        });
    }
    if let Some(env) = settings.get_mut("env").and_then(Value::as_object_mut) {
        match &record.previous {
            Some(previous) => {
                env.insert(ENV_KEY.into(), Value::String(previous.clone()));
            }
            None => {
                env.shift_remove(ENV_KEY);
            }
        }
        if env.is_empty() {
            settings.shift_remove("env");
        }
    }
    write_settings(&record.claude_dir, &settings)?;
    Ok(ClaudeUnlinkReport {
        restored: true,
        left_alone: false,
    })
}

/// The indentation the file already uses, for a file this module has to write whole.
fn indent_of(text: &str) -> usize {
    text.lines()
        .skip(1)
        .find(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .filter(|width| (1..=8).contains(width))
        .unwrap_or(2)
}

/// Write `settings`, which differs from the file only in `env.ANTHROPIC_BASE_URL` (and maybe
/// an `env` object added or emptied). When the file already holds that key, only its string
/// changes on disk, so the person's formatting survives. Otherwise the file is written whole
/// in its own indentation.
fn write_settings(claude_dir: &Path, settings: &Map<String, Value>) -> Result<()> {
    let path = settings_path(claude_dir);
    let existing = std::fs::read_to_string(&path).ok();
    let text = existing
        .as_deref()
        .and_then(|text| splice_base_url(text, settings))
        .unwrap_or_else(|| whole(existing.as_deref().unwrap_or(""), settings));
    if existing.is_some() {
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        std::fs::copy(
            &path,
            claude_dir.join(format!("settings.json.keysmith-relay-{stamp}.bak")),
        )?;
    } else {
        std::fs::create_dir_all(claude_dir)?;
    }
    atomic_write(&path, &text)
}

fn whole(existing: &str, settings: &Map<String, Value>) -> String {
    let indent = " ".repeat(indent_of(existing));
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
    if Value::Object(settings.clone())
        .serialize(&mut serializer)
        .is_err()
    {
        return "{}\n".into();
    }
    out.push(b'\n');
    String::from_utf8_lossy(&out).into_owned()
}

/// Replace the string value of the one `"ANTHROPIC_BASE_URL"` key in `text`, when the file
/// has exactly one such key inside `env` and `settings` still has it. The result is checked:
/// it must parse back to exactly `settings`.
fn splice_base_url(text: &str, settings: &Map<String, Value>) -> Option<String> {
    let new = env_value(settings, ENV_KEY)?;
    let needle = format!("\"{ENV_KEY}\"");
    let mut found = text.match_indices(&needle);
    let (at, _) = found.next()?;
    if found.next().is_some() {
        return None;
    }
    let rest = &text[at + needle.len()..];
    let colon = rest.find(':')?;
    if !rest[..colon].trim().is_empty() {
        return None;
    }
    let after_colon = &rest[colon + 1..];
    let quote = after_colon.find('"')?;
    if !after_colon[..quote].trim().is_empty() {
        return None;
    }
    let value_start = at + needle.len() + colon + 1 + quote;
    let mut end = None;
    let mut escaped = false;
    for (i, c) in text[value_start + 1..].char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => {
                end = Some(value_start + 1 + i + 1);
                break;
            }
            _ => {}
        }
    }
    let end = end?;
    let encoded = serde_json::to_string(new).ok()?;
    let out = format!("{}{}{}", &text[..value_start], encoded, &text[end..]);
    let parsed: Value = serde_json::from_str(&out).ok()?;
    (parsed == Value::Object(settings.clone())).then_some(out)
}
