//! Input rewrite, the app side: the person edits rule tables and switches here; the app
//! writes them out as one snapshot file the relay reads before each request. The app never
//! sees a conversation.
//!
//! Connecting Codex has two halves: [`service`] keeps the relay running, [`link`] points
//! Codex's config at it. The link record ties them together so disconnecting undoes exactly
//! what connecting did.

pub mod claude;
pub mod link;
pub mod service;

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
pub fn publish(store: &Store) -> Result<()> {
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
    if read_link_record(paths).is_none() && read_claude_record(paths).is_none() {
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
        _ => Ok(false),
    }
}

/// Apply a change, publish the snapshot, and return what the page shows.
pub fn change(store: &Store, apply: impl FnOnce(&Store) -> Result<()>) -> Result<RewriteView> {
    apply(store)?;
    publish(store)?;
    view(store)
}
