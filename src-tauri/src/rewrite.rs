//! Input rewrite, the app side: the person edits rule tables and switches here; the app
//! writes them out as one snapshot file the relay reads before each request. The app never
//! sees a conversation.

use std::path::PathBuf;

use serde::Serialize;

use keysmith_rewrite::{Snapshot, ToolSwitches, SNAPSHOT_SCHEMA};

use crate::db::rules::RuleTable;
use crate::db::Store;
use crate::error::Result;
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
        },
        rules: store.active_rules()?,
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
}

pub fn view(store: &Store) -> Result<RewriteView> {
    let settings = store.get_settings()?;
    Ok(RewriteView {
        enabled: settings.rewrite_enabled,
        codex_enabled: settings.rewrite_codex_enabled,
        tables: store.list_rule_tables()?,
    })
}

/// Apply a change, publish the snapshot, and return what the page shows.
pub fn change(store: &Store, apply: impl FnOnce(&Store) -> Result<()>) -> Result<RewriteView> {
    apply(store)?;
    publish(store)?;
    view(store)
}
