//! Input rewrite, the app side: the person edits rule tables and switches here; the app
//! writes them out as one snapshot file the relay reads before each request. The app never
//! sees a conversation.
//!
//! Connecting Codex has two halves: [`service`] keeps the relay running, [`link`] points
//! Codex's config at it. The link record ties them together so disconnecting undoes exactly
//! what connecting did.

pub mod claude;
pub mod grok;
pub mod link;
pub mod service;
pub mod zcode;

use std::path::{Path, PathBuf};

use serde::Serialize;

use std::collections::BTreeMap;

use keysmith_rewrite::{Snapshot, Tool, ToolSwitches, SNAPSHOT_SCHEMA};

use crate::db::rules::RuleTable;
use crate::db::Store;
use crate::error::{Error, Result};
use crate::paths::{atomic_write, AppPaths};

pub fn rewrite_dir(paths: &AppPaths) -> PathBuf {
    paths.home.join("input-rewrite")
}

pub fn snapshot_path(paths: &AppPaths) -> PathBuf {
    rewrite_dir(paths).join("rules.json")
}

/// The snapshot the relay reads. `rules` is Codex's list, the one a v0.4.0 relay reads;
/// `byTool` is written only when some agent's list differs from it.
pub fn build_snapshot(store: &Store) -> Result<Snapshot> {
    let settings = store.get_settings()?;
    let codex = store.active_rules(Tool::Codex)?;
    let mut by_tool = BTreeMap::new();
    for tool in Tool::ALL {
        by_tool.insert(tool, store.active_rules(tool)?);
    }
    let uniform = by_tool.values().all(|rules| *rules == codex);
    Ok(Snapshot {
        schema: SNAPSHOT_SCHEMA,
        enabled: settings.rewrite_enabled,
        tools: ToolSwitches {
            codex: settings.rewrite_codex_enabled,
            claude: settings.rewrite_claude_enabled,
            grok: settings.rewrite_grok_enabled,
            zcode: settings.rewrite_zcode_enabled,
        },
        rules: codex,
        by_tool: (!uniform).then_some(by_tool),
    })
}

/// Write the snapshot the relay reads. Called after every change to rules or switches.
/// Changes arrive from more than one thread: building and writing hold one lock, so the file
/// always ends with the latest settings, never an older snapshot that finished last.
pub fn publish(store: &Store) -> Result<()> {
    static PUBLISH: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turn = PUBLISH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = build_snapshot(store)?;
    atomic_write(
        &snapshot_path(store.paths()),
        &serde_json::to_string_pretty(&snapshot)?,
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteView {
    pub enabled: bool,
    pub codex_enabled: bool,
    pub claude_enabled: bool,
    pub grok_enabled: bool,
    pub zcode_enabled: bool,
    pub tables: Vec<RuleTable>,
    pub codex: CodexView,
    pub claude: ClaudeView,
    pub zcode: ZcodeView,
    pub grok: GrokView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokView {
    pub link: grok::GrokLinkState,
    pub service: service::ServiceStatus,
    /// Hosts the catalog models really talk to.
    pub hosts: Vec<String>,
    /// Catalog models kept out of the relay because a custom model of the person's names them
    /// as its upstream.
    pub left_out: Vec<String>,
    pub unsupported: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeView {
    pub link: zcode::ZcodeLinkState,
    pub service: service::ServiceStatus,
    /// The personal providers connecting would route, with their own hosts only.
    pub providers: Vec<ZcodeProviderView>,
    pub unsupported: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeProviderView {
    pub name: String,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeView {
    pub link: claude::ClaudeLinkState,
    pub service: service::ServiceStatus,
    /// The host requests go to without the relay (the one restored on disconnect).
    pub upstream_host: Option<String>,
    pub unsupported: Option<String>,
    /// The file and key connecting writes, for the page to show before it does.
    pub settings_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexView {
    pub link: link::LinkState,
    pub service: service::ServiceStatus,
    /// The provider Codex uses now and whether the relay can stand in front of it.
    pub provider: Option<link::ProviderInfo>,
    pub unsupported: Option<String>,
    pub codex_dir: Option<String>,
}

pub fn view(store: &Store) -> Result<RewriteView> {
    view_with(store, None)
}

/// `home` overrides where `.codex` is looked up (tests, isolated previews).
pub fn view_with(store: &Store, home: Option<&Path>) -> Result<RewriteView> {
    let settings = store.get_settings()?;
    Ok(RewriteView {
        enabled: settings.rewrite_enabled,
        codex_enabled: settings.rewrite_codex_enabled,
        claude_enabled: settings.rewrite_claude_enabled,
        grok_enabled: settings.rewrite_grok_enabled,
        zcode_enabled: settings.rewrite_zcode_enabled,
        tables: store.list_rule_tables()?,
        codex: codex_view(store.paths(), home),
        claude: claude_view(store.paths(), home),
        zcode: zcode_view(store.paths(), home),
        grok: grok_view(store.paths(), home),
    })
}

fn link_record_path(paths: &AppPaths) -> PathBuf {
    rewrite_dir(paths).join("codex-link.json")
}

pub fn read_link_record(paths: &AppPaths) -> Option<link::LinkRecord> {
    std::fs::read(link_record_path(paths))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn codex_view(paths: &AppPaths, home: Option<&Path>) -> CodexView {
    let record = read_link_record(paths);
    let codex_dir = record
        .as_ref()
        .map(|record| record.codex_dir.clone())
        .or_else(|| link::codex_dir(home));
    let (link_state, provider, unsupported) = match &codex_dir {
        Some(dir) => {
            let state = link::state(record.as_ref(), dir);
            match link::current_provider(dir) {
                Ok(provider) => (state, Some(provider), None),
                Err(reason) => (state, None, Some(reason.code().to_string())),
            }
        }
        None => (link::LinkState::Unlinked, None, Some("no-config".into())),
    };
    CodexView {
        link: link_state,
        service: service::status(paths),
        provider,
        unsupported,
        codex_dir: codex_dir.map(|dir| dir.display().to_string()),
    }
}

/// Start the relay and point Codex at it. Safe to repeat: re-connecting after another tool
/// switched Codex's provider forwards for the provider Codex uses now.
pub fn connect_codex(paths: &AppPaths, home: Option<&Path>) -> Result<()> {
    let record = read_link_record(paths);
    let codex_dir = record
        .as_ref()
        .map(|record| record.codex_dir.clone())
        .or_else(|| link::codex_dir(home))
        .ok_or_else(|| Error::unavailable("no-config: cannot find the Codex folder"))?;
    let mut config = service::ensure_config(paths)?;

    // Decide the provider first so nothing changes when Codex cannot be linked. While Codex
    // still uses the managed provider, keep forwarding for the provider recorded at link time.
    let keep = record.as_ref().filter(|_| link::uses_managed(&codex_dir));
    let (provider_id, upstream) = match keep {
        Some(record) => {
            let upstream = config
                .upstreams
                .get(&record.provider)
                .cloned()
                .ok_or_else(|| Error::drift("relay has no upstream for the linked provider"))?;
            (record.provider.clone(), upstream)
        }
        None => {
            let provider = link::current_provider(&codex_dir).map_err(|reason| {
                Error::unavailable(format!("{}: Codex cannot be linked", reason.code()))
            })?;
            (provider.id, provider.base_url)
        }
    };
    config.upstreams.insert(provider_id.clone(), upstream);
    service::write_config(paths, &config)?;
    service::install(paths)?;

    let (new_record, _) = link::link(&codex_dir, keep, &service::base_url(&config, &provider_id))?;
    atomic_write(
        &link_record_path(paths),
        &serde_json::to_string_pretty(&new_record)?,
    )
}

/// Point Codex back at its own provider. Rules stay; the relay stops when no agent uses it.
pub fn disconnect_codex(paths: &AppPaths) -> Result<link::UnlinkReport> {
    let report = match read_link_record(paths) {
        Some(record) => link::unlink(&record)?,
        None => link::UnlinkReport::default(),
    };
    let _ = std::fs::remove_file(link_record_path(paths));
    stop_service_if_unused(paths)?;
    Ok(report)
}

/// The relay keeps running while any agent has a link record.
fn stop_service_if_unused(paths: &AppPaths) -> Result<()> {
    if read_link_record(paths).is_none()
        && read_claude_record(paths).is_none()
        && read_zcode_record(paths).is_none()
        && read_grok_record(paths).is_none()
    {
        service::uninstall(paths)?;
    }
    Ok(())
}

// ----- Claude Code ------------------------------------------------------------------

/// The upstream id under `claude/` in the relay config. Claude Code has one address.
const CLAUDE_UPSTREAM: &str = "anthropic";

fn claude_record_path(paths: &AppPaths) -> PathBuf {
    rewrite_dir(paths).join("claude-link.json")
}

pub fn read_claude_record(paths: &AppPaths) -> Option<claude::ClaudeLinkRecord> {
    std::fs::read(claude_record_path(paths))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn claude_view(paths: &AppPaths, home: Option<&Path>) -> ClaudeView {
    let record = read_claude_record(paths);
    let dir = record
        .as_ref()
        .map(|record| record.claude_dir.clone())
        .or_else(|| claude::claude_dir(home));
    let Some(dir) = dir else {
        return ClaudeView {
            link: claude::ClaudeLinkState::Unlinked,
            service: service::status(paths),
            upstream_host: None,
            unsupported: Some("no-config".into()),
            settings_path: None,
        };
    };
    let link = claude::state(record.as_ref(), &dir);
    // While linked, the address that matters is the one the relay forwards to.
    let (upstream, unsupported) = match (&link, &record) {
        (claude::ClaudeLinkState::Linked, Some(record)) => (
            Some(
                record
                    .previous
                    .clone()
                    .unwrap_or_else(|| claude::DEFAULT_UPSTREAM.into()),
            ),
            None,
        ),
        _ => match claude::current_upstream(&dir) {
            Ok(upstream) => (Some(upstream), None),
            Err(reason) => (None, Some(reason.code().to_string())),
        },
    };
    ClaudeView {
        link,
        service: service::status(paths),
        upstream_host: upstream.as_deref().and_then(claude::upstream_host),
        unsupported,
        settings_path: Some(dir.join("settings.json").display().to_string()),
    }
}

/// Start the relay and point Claude Code at it.
pub fn connect_claude(paths: &AppPaths, home: Option<&Path>) -> Result<()> {
    let record = read_claude_record(paths);
    let dir = record
        .as_ref()
        .map(|record| record.claude_dir.clone())
        .or_else(|| claude::claude_dir(home))
        .ok_or_else(|| Error::unavailable("no-config: cannot find the Claude Code folder"))?;
    let mut config = service::ensure_config(paths)?;
    let still_linked = record
        .as_ref()
        .filter(|record| claude::state(Some(record), &dir) == claude::ClaudeLinkState::Linked);
    let upstream = match still_linked {
        Some(record) => record
            .previous
            .clone()
            .unwrap_or_else(|| claude::DEFAULT_UPSTREAM.into()),
        None => claude::current_upstream(&dir).map_err(|reason| {
            Error::unavailable(format!("{}: Claude Code cannot be linked", reason.code()))
        })?,
    };
    config
        .routes
        .insert(format!("claude/{CLAUDE_UPSTREAM}"), upstream);
    service::write_config(paths, &config)?;
    service::install(paths)?;
    let relay_base_url = service::route_url(&config, "claude", CLAUDE_UPSTREAM);
    let new_record = claude::link(&dir, still_linked, &relay_base_url)?;
    atomic_write(
        &claude_record_path(paths),
        &serde_json::to_string_pretty(&new_record)?,
    )
}

/// Put Claude Code's address back. Rules stay; the relay stops when no agent uses it.
pub fn disconnect_claude(paths: &AppPaths) -> Result<claude::ClaudeUnlinkReport> {
    let report = match read_claude_record(paths) {
        Some(record) => claude::unlink(&record)?,
        None => claude::ClaudeUnlinkReport::default(),
    };
    let _ = std::fs::remove_file(claude_record_path(paths));
    stop_service_if_unused(paths)?;
    Ok(report)
}

// ----- ZCode ------------------------------------------------------------------------

fn zcode_record_path(paths: &AppPaths) -> PathBuf {
    rewrite_dir(paths).join("zcode-link.json")
}

pub fn read_zcode_record(paths: &AppPaths) -> Option<zcode::ZcodeLinkRecord> {
    std::fs::read(zcode_record_path(paths))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn zcode_view(paths: &AppPaths, home: Option<&Path>) -> ZcodeView {
    let record = read_zcode_record(paths);
    let dir = record
        .as_ref()
        .map(|record| record.zcode_dir.clone())
        .or_else(|| zcode::zcode_dir(home));
    let Some(dir) = dir else {
        return ZcodeView {
            link: zcode::ZcodeLinkState::Unlinked,
            service: service::status(paths),
            providers: Vec::new(),
            unsupported: Some("no-config".into()),
            config_path: None,
        };
    };
    let link = zcode::state(record.as_ref(), &dir);
    let host = |url: &str| claude::upstream_host(url);
    let (providers, unsupported) = match zcode::providers(&dir) {
        Ok(found) => (
            found
                .into_iter()
                .map(|provider| {
                    // A linked provider shows the address it forwards to, not the relay.
                    let real = record
                        .as_ref()
                        .and_then(|record| {
                            record
                                .providers
                                .iter()
                                .find(|linked| linked.provider_id == provider.id)
                        })
                        .filter(|linked| linked.relay_base_url == provider.base_url)
                        .map(|linked| linked.previous.clone())
                        .unwrap_or(provider.base_url);
                    ZcodeProviderView {
                        name: provider.name,
                        host: host(&real),
                    }
                })
                .collect(),
            None,
        ),
        Err(reason) => (Vec::new(), Some(reason.code().to_string())),
    };
    ZcodeView {
        link,
        service: service::status(paths),
        providers,
        unsupported,
        config_path: Some(dir.join("provider_config.json").display().to_string()),
    }
}

/// Start the relay and aim every personal ZCode provider at it.
pub fn connect_zcode(paths: &AppPaths, home: Option<&Path>) -> Result<()> {
    let record = read_zcode_record(paths);
    let dir = record
        .as_ref()
        .map(|record| record.zcode_dir.clone())
        .or_else(|| zcode::zcode_dir(home))
        .ok_or_else(|| Error::unavailable("no-config: cannot find the ZCode folder"))?;
    // Nothing changes when ZCode cannot be linked.
    zcode::providers(&dir).map_err(|reason| {
        Error::unavailable(format!("{}: ZCode cannot be linked", reason.code()))
    })?;
    // ZCode re-reads its file every second, so the relay must know each route before any
    // provider points at it. Routes for providers already on the relay keep their address.
    let mut config = service::ensure_config(paths)?;
    for provider in zcode::providers(&dir).unwrap_or_default() {
        let key = format!("zcode/{}", provider.id);
        let still_linked = record.as_ref().and_then(|record| {
            record.providers.iter().find(|linked| {
                linked.provider_id == provider.id && linked.relay_base_url == provider.base_url
            })
        });
        let upstream = match still_linked {
            Some(linked) => linked.previous.clone(),
            None => provider.base_url,
        };
        config.routes.insert(key, upstream);
    }
    service::write_config(paths, &config)?;
    service::install(paths)?;
    let new_record = zcode::link(&dir, record.as_ref(), |id| {
        service::route_url(&config, "zcode", id)
    })?;
    // Drop routes for providers that are no longer linked.
    config.routes.retain(|key, _| {
        key.strip_prefix("zcode/").is_none_or(|id| {
            new_record
                .providers
                .iter()
                .any(|linked| linked.provider_id == id)
        })
    });
    service::write_config(paths, &config)?;
    atomic_write(
        &zcode_record_path(paths),
        &serde_json::to_string_pretty(&new_record)?,
    )
}

/// Put every ZCode provider's address back. Rules stay; the relay stops when no agent uses it.
pub fn disconnect_zcode(paths: &AppPaths) -> Result<zcode::ZcodeUnlinkReport> {
    let report = match read_zcode_record(paths) {
        Some(record) => zcode::unlink(&record)?,
        None => zcode::ZcodeUnlinkReport::default(),
    };
    let _ = std::fs::remove_file(zcode_record_path(paths));
    stop_service_if_unused(paths)?;
    Ok(report)
}

// ----- Grok Build -------------------------------------------------------------------

fn grok_record_path(paths: &AppPaths) -> PathBuf {
    rewrite_dir(paths).join("grok-link.json")
}

pub fn read_grok_record(paths: &AppPaths) -> Option<grok::GrokLinkRecord> {
    std::fs::read(grok_record_path(paths))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

/// The relay upstream id for one real base URL: one route per distinct address.
fn grok_upstream_id(base_url: &str) -> String {
    let digest = sha2_hex(base_url.as_bytes());
    format!("u{}", &digest[..12])
}

fn sha2_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn grok_view(paths: &AppPaths, home: Option<&Path>) -> GrokView {
    let record = read_grok_record(paths);
    let dir = record
        .as_ref()
        .map(|record| record.grok_dir.clone())
        .or_else(|| grok::grok_dir(home));
    let Some(dir) = dir else {
        return GrokView {
            link: grok::GrokLinkState::Unlinked,
            service: service::status(paths),
            hosts: Vec::new(),
            left_out: Vec::new(),
            unsupported: Some("no-config".into()),
            config_path: None,
        };
    };
    let link = grok::state(record.as_ref(), &dir);
    let (hosts, unsupported) = match grok::routable(&dir) {
        Ok(models) => {
            let mut hosts: Vec<String> = models
                .iter()
                .filter_map(|model| claude::upstream_host(&model.base_url))
                .collect();
            hosts.sort();
            hosts.dedup();
            (hosts, None)
        }
        Err(reason) => (Vec::new(), Some(reason.code().to_string())),
    };
    GrokView {
        link,
        service: service::status(paths),
        hosts,
        left_out: grok::left_out(&dir),
        unsupported,
        config_path: Some(dir.join("config.toml").display().to_string()),
    }
}

/// Start the relay and route every catalog model through it.
pub fn connect_grok(paths: &AppPaths, home: Option<&Path>) -> Result<()> {
    let record = read_grok_record(paths);
    let dir = record
        .as_ref()
        .map(|record| record.grok_dir.clone())
        .or_else(|| grok::grok_dir(home))
        .ok_or_else(|| Error::unavailable("no-config: cannot find the Grok folder"))?;
    let models = grok::routable(&dir).map_err(|reason| {
        Error::unavailable(format!("{}: Grok cannot be linked", reason.code()))
    })?;
    // Routes first: Grok may start a request the moment the file changes.
    let mut config = service::ensure_config(paths)?;
    config.routes.retain(|key, _| !key.starts_with("grok/"));
    let mut table = BTreeMap::new();
    for model in &models {
        let upstream = grok_upstream_id(&model.base_url);
        config
            .routes
            .insert(format!("grok/{upstream}"), model.base_url.clone());
        table.insert(
            model.id.clone(),
            service::route_url(&config, "grok", &upstream),
        );
    }
    service::write_config(paths, &config)?;
    service::install(paths)?;
    let new_record = grok::link(&dir, table)?;
    atomic_write(
        &grok_record_path(paths),
        &serde_json::to_string_pretty(&new_record)?,
    )
}

/// Remove the region. Rules stay; the relay stops when no agent uses it.
pub fn disconnect_grok(paths: &AppPaths) -> Result<grok::GrokUnlinkReport> {
    let report = match read_grok_record(paths) {
        Some(record) => grok::unlink(&record)?,
        None => grok::GrokUnlinkReport::default(),
    };
    let _ = std::fs::remove_file(grok_record_path(paths));
    stop_service_if_unused(paths)?;
    Ok(report)
}

/// Disconnect one agent if it is connected; true when it was. Cleanup calls this before it
/// saves that agent's settings, so a rollback never brings back an address pointing at a
/// relay that is gone.
pub fn disconnect_tool(paths: &AppPaths, tool: crate::models::ToolKind) -> Result<bool> {
    use crate::models::ToolKind;
    match tool {
        ToolKind::Codex if read_link_record(paths).is_some() => {
            disconnect_codex(paths)?;
            Ok(true)
        }
        ToolKind::Claude if read_claude_record(paths).is_some() => {
            disconnect_claude(paths)?;
            Ok(true)
        }
        ToolKind::Zcode if read_zcode_record(paths).is_some() => {
            disconnect_zcode(paths)?;
            Ok(true)
        }
        ToolKind::Grok if read_grok_record(paths).is_some() => {
            disconnect_grok(paths)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Apply a change, publish the snapshot, and return what the page shows.
pub fn change(store: &Store, apply: impl FnOnce(&Store) -> Result<()>) -> Result<RewriteView> {
    apply(store)?;
    publish(store)?;
    view(store)
}
