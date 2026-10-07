//! Pointing Grok Build at the relay, and back.
//!
//! Grok chooses the address per model, so linking writes one `[model."<id>"]` table with a
//! `base_url` for every model in Grok's catalog, all inside one marked region at the end of
//! `~/.grok/config.toml`:
//!
//! ```toml
//! # === keysmith-switch input rewrite begin ===
//! [model."grok-4.6"]
//! base_url = "http://127.0.0.1:…/t/<token>/grok/<upstream>"
//! # === keysmith-switch input rewrite end ===
//! ```
//!
//! The region is this module's alone: nothing outside it is written, and unlinking removes it
//! whole. grok-keysmith 0.7.0 and later leave it out of their config fingerprint and keep it
//! through deploys, uninstalls and recovery. A model that already has a `[model.<id>]` table
//! of the person's own is left alone (two tables with one name would not parse).
//!
//! Where each model really goes comes from Grok's catalog cache (`models_cache.json`); a model
//! added to the catalog later bypasses the relay until reconnecting picks it up.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::paths::atomic_write;

pub const BEGIN: &str = "# === keysmith-switch input rewrite begin ===";
pub const END: &str = "# === keysmith-switch input rewrite end ===";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokLinkRecord {
    pub grok_dir: PathBuf,
    /// Model id → the relay address written for it.
    pub models: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum GrokLinkState {
    /// The region is in place. `unrouted` counts catalog models it does not cover yet.
    Linked {
        unrouted: usize,
    },
    /// A link record exists but the region is gone or was edited.
    Bypassed,
    Unlinked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    NoConfig,
    /// Grok has not fetched its model list yet: run it once.
    NoCatalog,
    /// The region's markers are damaged (one without the other, or twice).
    BadRegion,
}

impl Unsupported {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoConfig => "no-config",
            Self::NoCatalog => "no-catalog",
            Self::BadRegion => "bad-region",
        }
    }
}

/// One catalog model the relay can stand in front of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub base_url: String,
}

pub fn grok_dir(home_override: Option<&Path>) -> Option<PathBuf> {
    if home_override.is_none() {
        if let Ok(dir) = std::env::var("GROK_HOME") {
            if !dir.trim().is_empty() {
                return Some(PathBuf::from(dir.trim()));
            }
        }
    }
    let home = match home_override {
        Some(home) => home.to_path_buf(),
        None => dirs::home_dir()?,
    };
    Some(home.join(".grok"))
}

fn config_path(grok_dir: &Path) -> PathBuf {
    grok_dir.join("config.toml")
}

/// A model id that can stand as one relay path segment and one quoted TOML key as it is.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// The catalog models with http(s) addresses, from Grok's own cache. Keys and tokens in the
/// cache are never read into anything that leaves this function.
pub fn catalog(grok_dir: &Path) -> std::result::Result<Vec<Model>, Unsupported> {
    let bytes =
        std::fs::read(grok_dir.join("models_cache.json")).map_err(|_| Unsupported::NoCatalog)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| Unsupported::NoCatalog)?;
    let models = value
        .get("models")
        .and_then(Value::as_object)
        .ok_or(Unsupported::NoCatalog)?;
    let found: Vec<Model> = models
        .iter()
        .filter_map(|(id, entry)| {
            let base_url = entry.get("info")?.get("base_url")?.as_str()?;
            let http =
                url::Url::parse(base_url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"));
            (safe_id(id) && http && !is_relay(base_url)).then(|| Model {
                id: id.clone(),
                base_url: base_url.to_string(),
            })
        })
        .collect();
    if found.is_empty() {
        return Err(Unsupported::NoCatalog);
    }
    Ok(found)
}

fn is_relay(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:") && url.contains("/t/")
}

/// (text outside the region, the region's text), or `BadRegion` when the markers are damaged.
fn split(text: &str) -> std::result::Result<(String, Option<String>), Unsupported> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let begins: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim() == BEGIN)
        .collect();
    let ends: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim() == END)
        .collect();
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok((text.to_string(), None)),
        ([b], [e]) if b < e => {
            let outside: String = lines[..*b].concat() + &lines[e + 1..].concat();
            let mut region = lines[*b..=*e].concat();
            if !region.ends_with('\n') {
                region.push('\n');
            }
            Ok((outside, Some(region)))
        }
        _ => Err(Unsupported::BadRegion),
    }
}

/// Model ids that already have a `[model.<id>]` (or quoted) table outside the region.
fn own_tables(outside: &str) -> Vec<String> {
    outside
        .lines()
        .filter_map(|line| {
            let header = line.trim().strip_prefix("[model.")?.strip_suffix(']')?;
            Some(header.trim_matches('"').to_string())
        })
        .collect()
}

fn region_text(models: &BTreeMap<String, String>) -> String {
    let mut out = format!("{BEGIN}\n");
    for (id, url) in models {
        out.push_str(&format!("[model.\"{id}\"]\nbase_url = \"{url}\"\n"));
    }
    out.push_str(END);
    out.push('\n');
    out
}

fn read_config(grok_dir: &Path) -> std::result::Result<String, Unsupported> {
    std::fs::read_to_string(config_path(grok_dir)).map_err(|_| Unsupported::NoConfig)
}

pub fn state(record: Option<&GrokLinkRecord>, grok_dir: &Path) -> GrokLinkState {
    let Some(record) = record else {
        return GrokLinkState::Unlinked;
    };
    let intact = read_config(grok_dir)
        .ok()
        .and_then(|text| split(&text).ok())
        .and_then(|(_, region)| region)
        .is_some_and(|region| region == region_text(&record.models));
    if !intact {
        return GrokLinkState::Bypassed;
    }
    let unrouted = catalog(grok_dir)
        .unwrap_or_default()
        .iter()
        .filter(|model| !record.models.contains_key(&model.id))
        .count();
    GrokLinkState::Linked { unrouted }
}

/// Models to route now: every catalog model without a table of the person's own.
pub fn routable(grok_dir: &Path) -> std::result::Result<Vec<Model>, Unsupported> {
    let text = read_config(grok_dir).unwrap_or_default();
    let (outside, _) = split(&text)?;
    let own = own_tables(&outside);
    Ok(catalog(grok_dir)?
        .into_iter()
        .filter(|model| !own.contains(&model.id))
        .collect())
}

/// Write the region for `models` (id → relay address), replacing any earlier one.
pub fn link(grok_dir: &Path, models: BTreeMap<String, String>) -> Result<GrokLinkRecord> {
    if models.is_empty() {
        return Err(Error::unavailable(Unsupported::NoCatalog.code()));
    }
    let text = match read_config(grok_dir) {
        Ok(text) => text,
        Err(_) if grok_dir.is_dir() => String::new(),
        Err(reason) => return Err(Error::unavailable(reason.code())),
    };
    let (outside, _) = split(&text).map_err(|reason| Error::unavailable(reason.code()))?;
    let mut out = outside;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&region_text(&models));
    write_config(grok_dir, &text, &out)?;
    Ok(GrokLinkRecord {
        grok_dir: grok_dir.to_path_buf(),
        models,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokUnlinkReport {
    pub removed: bool,
    /// The region was edited by someone else, so it was left as it is.
    pub left_alone: bool,
}

/// Remove the region, only while it is still exactly what linking wrote.
pub fn unlink(record: &GrokLinkRecord) -> Result<GrokUnlinkReport> {
    let Ok(text) = read_config(&record.grok_dir) else {
        return Ok(GrokUnlinkReport::default());
    };
    let Ok((outside, region)) = split(&text) else {
        return Ok(GrokUnlinkReport {
            removed: false,
            left_alone: true,
        });
    };
    match region {
        None => Ok(GrokUnlinkReport::default()),
        Some(region) if region == region_text(&record.models) => {
            write_config(&record.grok_dir, &text, &outside)?;
            Ok(GrokUnlinkReport {
                removed: true,
                left_alone: false,
            })
        }
        Some(_) => Ok(GrokUnlinkReport {
            removed: false,
            left_alone: true,
        }),
    }
}

fn write_config(grok_dir: &Path, before: &str, after: &str) -> Result<()> {
    if before == after {
        return Ok(());
    }
    let path = config_path(grok_dir);
    if path.exists() {
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        std::fs::copy(
            &path,
            grok_dir.join(format!("config.toml.keysmith-relay-{stamp}.bak")),
        )?;
    }
    if after.parse::<toml_edit::DocumentMut>().is_err() {
        return Err(Error::message("config.toml would not parse after linking"));
    }
    atomic_write(&path, after)
}
