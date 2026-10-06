//! Input rewrite, the app side: the person edits rule tables and switches here; the app
//! writes them out as one snapshot file the relay reads before each request. The app never
//! sees a conversation.
//!
//! Connecting Codex has two halves: [`service`] keeps the relay running, [`link`] points
//! Codex's config at it. The link record ties them together so disconnecting undoes exactly
//! what connecting did.

pub mod link;
pub mod service;

use std::path::{Path, PathBuf};

use serde::Serialize;

use keysmith_rewrite::{Snapshot, ToolSwitches, SNAPSHOT_SCHEMA};

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

pub fn build_snapshot(store: &Store) -> Result<Snapshot> {
    let settings = store.get_settings()?;
    Ok(Snapshot {
        schema: SNAPSHOT_SCHEMA,
        enabled: settings.rewrite_enabled,
        tools: ToolSwitches {
            codex: settings.rewrite_codex_enabled,
            ..Default::default()
        },
        rules: store.active_rules()?,
        by_tool: None,
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
    pub tables: Vec<RuleTable>,
    pub codex: CodexView,
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
        tables: store.list_rule_tables()?,
        codex: codex_view(store.paths(), home),
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

/// Point Codex back at its own provider and stop the relay. Rules stay.
pub fn disconnect_codex(paths: &AppPaths) -> Result<link::UnlinkReport> {
    let report = match read_link_record(paths) {
        Some(record) => link::unlink(&record)?,
        None => link::UnlinkReport::default(),
    };
    service::uninstall(paths)?;
    let _ = std::fs::remove_file(link_record_path(paths));
    Ok(report)
}

/// Apply a change, publish the snapshot, and return what the page shows.
pub fn change(store: &Store, apply: impl FnOnce(&Store) -> Result<()>) -> Result<RewriteView> {
    apply(store)?;
    publish(store)?;
    view(store)
}
