//! Pointing Codex at the relay, and back.
//!
//! Linking adds one provider table, `[model_providers.keysmith-relay]`, copied from the
//! provider Codex was using with `base_url` swapped for the relay, and points the top-level
//! `model_provider` at it. Those two are the only things this module owns; the person's own
//! provider table is never edited. The provider Codex used before is kept in a link record so
//! unlinking can put it back, and only while the two owned values are still what linking wrote.
//!
//! Prompt deploys own `model_instructions_file` and nothing here, so a deploy or its undo
//! leaves the link alone.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::error::{Error, Result};
use crate::paths::atomic_write;

pub const MANAGED_PROVIDER: &str = "keysmith-relay";

/// What linking needs to remember to undo itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRecord {
    pub codex_dir: PathBuf,
    /// The top-level `model_provider` before linking; `None` when it was not set.
    pub previous_provider: Option<String>,
    /// The provider the relay forwards for.
    pub provider: String,
    pub relay_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum LinkState {
    /// Codex sends through the relay.
    Linked {
        provider: String,
    },
    /// A link record exists but Codex no longer uses the relay (another tool switched the
    /// provider, or the file was edited).
    Bypassed {
        provider: Option<String>,
    },
    Unlinked,
}

/// Where the provider Codex currently uses sends its requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    NoConfig,
    /// Codex's built-in OpenAI provider: its address depends on how the person signed in.
    BuiltIn(String),
    MissingProvider(String),
    NoBaseUrl(String),
    NotHttp(String),
    WireApi(String),
}

impl Unsupported {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoConfig => "no-config",
            Self::BuiltIn(_) => "built-in-provider",
            Self::MissingProvider(_) => "missing-provider",
            Self::NoBaseUrl(_) => "no-base-url",
            Self::NotHttp(_) => "not-http",
            Self::WireApi(_) => "wire-api",
        }
    }

    fn into_error(self) -> Error {
        let detail = match &self {
            Self::NoConfig => "Codex config.toml was not found".to_string(),
            Self::BuiltIn(id) => format!("built-in provider {id} is not supported yet"),
            Self::MissingProvider(id) => format!("provider {id} is not defined in config.toml"),
            Self::NoBaseUrl(id) => format!("provider {id} has no base_url"),
            Self::NotHttp(id) => format!("provider {id} base_url is not http(s)"),
            Self::WireApi(id) => format!("provider {id} does not use the Responses API"),
        };
        Error::unavailable(format!("{}: {detail}", self.code()))
    }
}

pub fn codex_dir(home_override: Option<&Path>) -> Option<PathBuf> {
    if home_override.is_none() {
        if let Ok(dir) = std::env::var("CODEX_HOME") {
            if !dir.trim().is_empty() {
                return Some(PathBuf::from(dir.trim()));
            }
        }
    }
    let home = match home_override {
        Some(home) => home.to_path_buf(),
        None => dirs::home_dir()?,
    };
    Some(home.join(".codex"))
}

fn config_path(codex_dir: &Path) -> PathBuf {
    codex_dir.join("config.toml")
}

fn read_doc(codex_dir: &Path) -> Result<Option<DocumentMut>> {
    let path = config_path(codex_dir);
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)?;
    text.parse::<DocumentMut>()
        .map(Some)
        .map_err(|error| Error::invalid(format!("Codex config.toml cannot be parsed: {error}")))
}

fn top_provider(doc: &DocumentMut) -> Option<String> {
    doc.get("model_provider")
        .and_then(Item::as_str)
        .map(str::to_string)
}

fn provider_table<'a>(doc: &'a DocumentMut, id: &str) -> Option<&'a Table> {
    doc.get("model_providers")?
        .as_table_like()?
        .get(id)?
        .as_table()
}

fn managed_base_url(doc: &DocumentMut) -> Option<String> {
    provider_table(doc, MANAGED_PROVIDER)?
        .get("base_url")?
        .as_str()
        .map(str::to_string)
}

/// The provider Codex uses now, if the relay can stand in front of it.
pub fn current_provider(codex_dir: &Path) -> std::result::Result<ProviderInfo, Unsupported> {
    let doc = read_doc(codex_dir)
        .ok()
        .flatten()
        .ok_or(Unsupported::NoConfig)?;
    let id = top_provider(&doc).unwrap_or_else(|| "openai".to_string());
    if id == MANAGED_PROVIDER {
        return Err(Unsupported::MissingProvider(id));
    }
    let Some(table) = provider_table(&doc, &id) else {
        return Err(
            if matches!(id.as_str(), "openai" | "oss" | "ollama" | "lmstudio") {
                Unsupported::BuiltIn(id)
            } else {
                Unsupported::MissingProvider(id)
            },
        );
    };
    if let Some(wire) = table.get("wire_api").and_then(Item::as_str) {
        if wire != "responses" {
            return Err(Unsupported::WireApi(id));
        }
    }
    let base_url = table
        .get("base_url")
        .and_then(Item::as_str)
        .map(str::to_string)
        .ok_or_else(|| Unsupported::NoBaseUrl(id.clone()))?;
    let parsed = url::Url::parse(&base_url).map_err(|_| Unsupported::NotHttp(id.clone()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Unsupported::NotHttp(id));
    }
    let name = table
        .get("name")
        .and_then(Item::as_str)
        .unwrap_or(&id)
        .to_string();
    Ok(ProviderInfo { id, name, base_url })
}

/// True when Codex's top-level provider is the one linking writes.
pub fn uses_managed(codex_dir: &Path) -> bool {
    read_doc(codex_dir)
        .ok()
        .flatten()
        .and_then(|doc| top_provider(&doc))
        .as_deref()
        == Some(MANAGED_PROVIDER)
}

pub fn state(record: Option<&LinkRecord>, codex_dir: &Path) -> LinkState {
    let doc = read_doc(codex_dir).ok().flatten();
    let provider = doc.as_ref().and_then(top_provider);
    match record {
        Some(record)
            if provider.as_deref() == Some(MANAGED_PROVIDER)
                && doc.as_ref().and_then(managed_base_url).as_deref()
                    == Some(record.relay_base_url.as_str()) =>
        {
            LinkState::Linked {
                provider: record.provider.clone(),
            }
        }
        Some(_) => LinkState::Bypassed { provider },
        None => LinkState::Unlinked,
    }
}

/// Copy the provider table, aim it at the relay, and switch Codex to it. Re-linking while
/// already linked forwards for the provider recorded then.
pub fn link(
    codex_dir: &Path,
    previous: Option<&LinkRecord>,
    relay_base_url: &str,
) -> Result<(LinkRecord, ProviderInfo)> {
    let mut doc = read_doc(codex_dir)?.ok_or_else(|| Unsupported::NoConfig.into_error())?;
    let already = top_provider(&doc).as_deref() == Some(MANAGED_PROVIDER);
    let (provider, previous_provider) = match (already, previous) {
        (true, Some(record)) => {
            let info =
                provider_info_by_id(&doc, &record.provider).map_err(Unsupported::into_error)?;
            (info, record.previous_provider.clone())
        }
        (true, None) => {
            return Err(Error::drift(
                "Codex already uses a keysmith-relay provider this app did not record",
            ))
        }
        (false, _) => (
            current_provider(codex_dir).map_err(Unsupported::into_error)?,
            top_provider(&doc),
        ),
    };

    let source = provider_table(&doc, &provider.id)
        .cloned()
        .ok_or_else(|| Unsupported::MissingProvider(provider.id.clone()).into_error())?;
    let mut managed = source;
    managed.set_implicit(false);
    managed.decor_mut().clear();
    managed.insert("name", value(format!("{} (via Keysmith)", provider.name)));
    managed.insert("base_url", value(relay_base_url));
    managed.insert("wire_api", value("responses"));
    // The relay speaks HTTP only.
    managed.insert("supports_websockets", value(false));

    let providers = doc
        .entry("model_providers")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(|| Error::invalid("model_providers in config.toml is not a table"))?;
    providers.insert(MANAGED_PROVIDER, Item::Table(managed));
    set_top_provider(&mut doc, MANAGED_PROVIDER);

    write_config(codex_dir, &doc)?;
    Ok((
        LinkRecord {
            codex_dir: codex_dir.to_path_buf(),
            previous_provider,
            provider: provider.id.clone(),
            relay_base_url: relay_base_url.to_string(),
        },
        provider,
    ))
}

fn provider_info_by_id(
    doc: &DocumentMut,
    id: &str,
) -> std::result::Result<ProviderInfo, Unsupported> {
    let table = provider_table(doc, id).ok_or_else(|| Unsupported::MissingProvider(id.into()))?;
    let base_url = table
        .get("base_url")
        .and_then(Item::as_str)
        .ok_or_else(|| Unsupported::NoBaseUrl(id.into()))?
        .to_string();
    let name = table
        .get("name")
        .and_then(Item::as_str)
        .unwrap_or(id)
        .to_string();
    Ok(ProviderInfo {
        id: id.into(),
        name,
        base_url,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkReport {
    pub restored_provider: bool,
    pub removed_managed: bool,
    /// Values left alone because someone else changed them after linking.
    pub left_alone: Vec<String>,
}

/// Put back what linking changed, but only values that still hold what linking wrote.
pub fn unlink(record: &LinkRecord) -> Result<UnlinkReport> {
    let mut report = UnlinkReport::default();
    let Some(mut doc) = read_doc(&record.codex_dir)? else {
        return Ok(report);
    };
    if top_provider(&doc).as_deref() == Some(MANAGED_PROVIDER) {
        match &record.previous_provider {
            Some(previous) => {
                set_top_provider(&mut doc, previous);
            }
            None => {
                doc.remove("model_provider");
            }
        }
        report.restored_provider = true;
    } else {
        report.left_alone.push("model_provider".into());
    }
    if managed_base_url(&doc).as_deref() == Some(record.relay_base_url.as_str()) {
        if let Some(providers) = doc
            .get_mut("model_providers")
            .and_then(Item::as_table_like_mut)
        {
            providers.remove(MANAGED_PROVIDER);
            report.removed_managed = true;
        }
    } else if provider_table(&doc, MANAGED_PROVIDER).is_some() {
        report
            .left_alone
            .push(format!("model_providers.{MANAGED_PROVIDER}"));
    }
    if report.restored_provider || report.removed_managed {
        write_config(&record.codex_dir, &doc)?;
    }
    Ok(report)
}

/// Back up, then replace atomically. One backup per change, next to the file.
/// Set the top-level `model_provider`, keeping the comments and position of an existing key.
fn set_top_provider(doc: &mut DocumentMut, provider: &str) {
    match doc.get_mut("model_provider").and_then(Item::as_value_mut) {
        Some(current) => {
            let decor = current.decor().clone();
            *current = toml_edit::Value::from(provider);
            *current.decor_mut() = decor;
        }
        None => {
            doc.insert("model_provider", value(provider));
        }
    }
}

fn write_config(codex_dir: &Path, doc: &DocumentMut) -> Result<()> {
    let path = config_path(codex_dir);
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup = codex_dir.join(format!("config.toml.keysmith-relay-{stamp}.bak"));
    std::fs::copy(&path, &backup)?;
    atomic_write(&path, &doc.to_string())
}
