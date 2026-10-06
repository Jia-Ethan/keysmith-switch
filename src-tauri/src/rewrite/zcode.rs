//! Pointing ZCode at the relay, and back.
//!
//! ZCode reads its providers from `~/.zcode/v2/provider_config.json`. Models are chosen by
//! provider id, so linking cannot add a provider of its own the way Codex linking does: it
//! changes `config.api.baseUrl` of each personal provider in place, and nothing else. The
//! addresses it replaced are kept in a link record; unlinking puts each one back only while
//! that provider still holds the relay address.
//!
//! ZCode treats a file it cannot read as empty and re-reads the file about once a second, so
//! a write only ever swaps string values, is checked to parse back to the expected document,
//! and lands atomically. Providers that come with a ZCode account (Coding Plan) are left
//! alone, as are addresses that are not http(s).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::paths::atomic_write;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedProvider {
    pub provider_id: String,
    /// `config.api.baseUrl` before linking.
    pub previous: String,
    pub relay_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeLinkRecord {
    pub zcode_dir: PathBuf,
    pub providers: Vec<LinkedProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ZcodeLinkState {
    /// Every linked provider sends through the relay. `unrouted` counts personal providers
    /// added since, which do not.
    Linked {
        unrouted: usize,
    },
    /// A link record exists but some linked provider no longer points at the relay.
    Bypassed,
    Unlinked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    NoConfig,
    BadConfig,
    /// No personal provider with an http(s) address.
    NoProvider,
}

impl Unsupported {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoConfig => "no-config",
            Self::BadConfig => "bad-config",
            Self::NoProvider => "no-provider",
        }
    }
}

/// A personal provider the relay can stand in front of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
}

pub fn zcode_dir(home_override: Option<&Path>) -> Option<PathBuf> {
    let home = match home_override {
        Some(home) => home.to_path_buf(),
        None => dirs::home_dir()?,
    };
    Some(home.join(".zcode").join("v2"))
}

fn config_path(zcode_dir: &Path) -> PathBuf {
    zcode_dir.join("provider_config.json")
}

fn read_config(zcode_dir: &Path) -> std::result::Result<Value, Unsupported> {
    let bytes = std::fs::read(config_path(zcode_dir)).map_err(|_| Unsupported::NoConfig)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| Unsupported::BadConfig)?;
    if rules(&value).is_none() {
        return Err(Unsupported::BadConfig);
    }
    Ok(value)
}

fn rules(config: &Value) -> Option<&Vec<Value>> {
    config
        .get("config")?
        .get("providerConfigRules")?
        .get("providerRules")?
        .as_array()
}

fn rules_mut(config: &mut Value) -> Option<&mut Vec<Value>> {
    config
        .get_mut("config")?
        .get_mut("providerConfigRules")?
        .get_mut("providerRules")?
        .as_array_mut()
}

fn base_url(rule: &Value) -> Option<&str> {
    rule.get("config")?.get("api")?.get("baseUrl")?.as_str()
}

fn set_base_url(rule: &mut Value, url: &str) -> bool {
    match rule
        .get_mut("config")
        .and_then(|config| config.get_mut("api"))
        .and_then(|api| api.get_mut("baseUrl"))
    {
        Some(slot @ Value::String(_)) => {
            *slot = Value::String(url.into());
            true
        }
        _ => false,
    }
}

/// An id that can stand as one segment of a relay path as it is.
fn route_safe(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn is_personal(rule: &Value) -> bool {
    let id = rule.get("providerId").and_then(Value::as_str).unwrap_or("");
    let group = rule
        .get("config")
        .and_then(|config| config.get("group"))
        .and_then(Value::as_str)
        .unwrap_or("");
    group.ends_with("-personal") && !id.starts_with("account:")
}

fn is_http(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn is_relay(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:") && url.contains("/t/")
}

/// The personal providers the relay can stand in front of, in ZCode's order.
pub fn providers(zcode_dir: &Path) -> std::result::Result<Vec<Provider>, Unsupported> {
    let config = read_config(zcode_dir)?;
    let found: Vec<Provider> = rules(&config)
        .into_iter()
        .flatten()
        .filter(|rule| is_personal(rule))
        .filter_map(|rule| {
            let id = rule.get("providerId")?.as_str()?;
            let url = base_url(rule)?;
            (route_safe(id) && is_http(url)).then(|| Provider {
                id: id.into(),
                name: rule
                    .get("providerName")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .into(),
                base_url: url.into(),
            })
        })
        .collect();
    if found.is_empty() {
        return Err(Unsupported::NoProvider);
    }
    Ok(found)
}

pub fn state(record: Option<&ZcodeLinkRecord>, zcode_dir: &Path) -> ZcodeLinkState {
    let Some(record) = record else {
        return ZcodeLinkState::Unlinked;
    };
    let Ok(config) = read_config(zcode_dir) else {
        return ZcodeLinkState::Bypassed;
    };
    let current = |id: &str| {
        rules(&config)
            .into_iter()
            .flatten()
            .find(|rule| rule.get("providerId").and_then(Value::as_str) == Some(id))
            .and_then(base_url)
            .map(str::to_string)
    };
    let intact = record.providers.iter().all(|linked| {
        current(&linked.provider_id).as_deref() == Some(linked.relay_base_url.as_str())
    });
    if !intact || record.providers.is_empty() {
        return ZcodeLinkState::Bypassed;
    }
    let unrouted = providers(zcode_dir)
        .unwrap_or_default()
        .iter()
        .filter(|provider| !is_relay(&provider.base_url))
        .count();
    ZcodeLinkState::Linked { unrouted }
}

/// Aim every personal provider at the relay. `relay_url` gives the relay address for one
/// provider id. Providers already linked keep the address recorded the first time. Returns
/// the record and, for the relay config, each provider id with its real address.
pub fn link(
    zcode_dir: &Path,
    previous: Option<&ZcodeLinkRecord>,
    relay_url: impl Fn(&str) -> String,
) -> Result<ZcodeLinkRecord> {
    let mut config = read_config(zcode_dir).map_err(|reason| Error::unavailable(reason.code()))?;
    let mut linked = Vec::new();
    let rules = rules_mut(&mut config).ok_or_else(|| Error::unavailable("bad-config"))?;
    for rule in rules.iter_mut() {
        if !is_personal(rule) {
            continue;
        }
        let Some(id) = rule
            .get("providerId")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let Some(now) = base_url(rule).map(str::to_string) else {
            continue;
        };
        if !route_safe(&id) {
            continue;
        }
        let relay = relay_url(&id);
        let earlier = previous.and_then(|record| {
            record
                .providers
                .iter()
                .find(|linked| linked.provider_id == id && linked.relay_base_url == now)
        });
        let was = match earlier {
            Some(earlier) => earlier.previous.clone(),
            None if is_relay(&now) => {
                return Err(Error::drift(format!(
                    "ZCode provider {id} already points at a relay this app has no record of"
                )))
            }
            None if !is_http(&now) => continue,
            None => now,
        };
        set_base_url(rule, &relay);
        linked.push(LinkedProvider {
            provider_id: id,
            previous: was,
            relay_base_url: relay,
        });
    }
    if linked.is_empty() {
        return Err(Error::unavailable(Unsupported::NoProvider.code()));
    }
    write_config(zcode_dir, &config)?;
    Ok(ZcodeLinkRecord {
        zcode_dir: zcode_dir.to_path_buf(),
        providers: linked,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeUnlinkReport {
    pub restored: Vec<String>,
    /// Providers someone else changed after linking, or removed; left as they are.
    pub left_alone: Vec<String>,
}

pub fn unlink(record: &ZcodeLinkRecord) -> Result<ZcodeUnlinkReport> {
    let mut report = ZcodeUnlinkReport::default();
    let Ok(mut config) = read_config(&record.zcode_dir) else {
        report.left_alone = record
            .providers
            .iter()
            .map(|linked| linked.provider_id.clone())
            .collect();
        return Ok(report);
    };
    if let Some(rules) = rules_mut(&mut config) {
        for linked in &record.providers {
            let rule = rules.iter_mut().find(|rule| {
                rule.get("providerId").and_then(Value::as_str) == Some(&linked.provider_id)
            });
            match rule {
                Some(rule) if base_url(rule) == Some(linked.relay_base_url.as_str()) => {
                    set_base_url(rule, &linked.previous);
                    report.restored.push(linked.provider_id.clone());
                }
                _ => report.left_alone.push(linked.provider_id.clone()),
            }
        }
    }
    if !report.restored.is_empty() {
        write_config(&record.zcode_dir, &config)?;
    }
    Ok(report)
}

fn indent_of(text: &str) -> usize {
    text.lines()
        .skip(1)
        .find(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .filter(|width| (1..=8).contains(width))
        .unwrap_or(2)
}

/// Back up, write in the file's own indentation, and check the result reads back as `config`.
fn write_config(zcode_dir: &Path, config: &Value) -> Result<()> {
    let path = config_path(zcode_dir);
    let existing = std::fs::read_to_string(&path)?;
    let indent = " ".repeat(indent_of(&existing));
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
    config.serialize(&mut serializer)?;
    if existing.ends_with('\n') {
        out.push(b'\n');
    }
    let text = String::from_utf8(out).map_err(|error| Error::message(error.to_string()))?;
    if serde_json::from_str::<Value>(&text).ok().as_ref() != Some(config) {
        return Err(Error::message("provider_config.json would not read back"));
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    std::fs::copy(
        &path,
        zcode_dir.join(format!("provider_config.json.keysmith-relay-{stamp}.bak")),
    )?;
    atomic_write(&path, &text)
}
