//! Clean selected categories of an agent's user-level data and roll them back.
//!
//! Deployment cleanup uses the adapter's uninstall and saves recoverable Keysmith leftovers.
//! Opting into memory, setup or sessions saves the selected pieces in a local snapshot.
//! Session data and memories folders move by rename while the agent is closed. Login and cache
//! cleanup saves sanitized config copies before removing their originals, erases the other
//! selected pieces and clears the agent's keychain entries.
//!
//! Rollback saves the data it replaces, including any live SQLite sidecars, and verifies
//! copies before putting them back. Saved extras skip links, git projects and login files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::adapter::{AdapterOptions, Envelope};
use crate::db::Store;
use crate::error::{Error, Result};
use crate::harness;
use crate::lock::HomeLock;
use crate::models::{
    now_rfc3339, OperationKind, OperationStatus, PlanActivateInput, PlanDeactivateInput, Scope,
    ToolKind, ToolStatus,
};
use crate::ops;

const MAX_MEMORY_BYTES: u64 = 8 * 1024 * 1024;
/// Tests and unusual setups can override an agent's running state with "1" or "0".
pub const CODEX_RUNNING_KEY: &str = "KEYSMITH_SWITCH_CODEX_RUNNING";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupCategory {
    Deployment,
    Memory,
    Memories,
    #[default]
    Setup,
    Sessions,
    LoginAndCache,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupSelection {
    #[serde(default)]
    pub deployment: bool,
    #[serde(default)]
    pub memory: bool,
    #[serde(default)]
    pub memories: bool,
    #[serde(default)]
    pub setup: bool,
    #[serde(default)]
    pub sessions: bool,
    #[serde(default)]
    pub login_and_cache: bool,
}

impl CleanupSelection {
    pub const fn default_safe() -> Self {
        Self {
            deployment: true,
            memory: false,
            memories: false,
            setup: false,
            sessions: false,
            login_and_cache: false,
        }
    }

    pub const fn all() -> Self {
        Self {
            deployment: true,
            memory: true,
            memories: true,
            setup: true,
            sessions: true,
            login_and_cache: true,
        }
    }

    pub const fn any_selected(&self) -> bool {
        self.deployment
            || self.memory
            || self.memories
            || self.setup
            || self.sessions
            || self.login_and_cache
    }
}

/// Paths are grouped by cleanup category and snapshot policy.
struct Table {
    entries: &'static [EntrySpec],
    prefixes: &'static [PrefixSpec],
    /// Names matching erased prefixes that belong to the agent's Keysmith adapter (its
    /// deployment backups). They are never erased: an adapter that lost its backup cannot
    /// undo its own deployment.
    adapter_owned: &'static [(&'static str, &'static str)],
    /// The agent's folder: a wrapper the adapter reports inside it is saved too.
    folder: &'static str,
    /// Where the agent keeps its login besides files (the macOS keychain).
    keychain: &'static [&'static str],
    /// Background jobs Switch left for this agent (macOS LaunchAgent labels): stopped first.
    launch_agents: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct EntrySpec {
    name: &'static str,
    kind: &'static str,
    category: CleanupCategory,
}

#[derive(Clone, Copy)]
struct PrefixSpec {
    folder: &'static str,
    prefix: &'static str,
    kind: &'static str,
    category: CleanupCategory,
}

const fn entry(name: &'static str, kind: &'static str, category: CleanupCategory) -> EntrySpec {
    EntrySpec {
        name,
        kind,
        category,
    }
}

const fn prefix(
    folder: &'static str,
    starts_with: &'static str,
    kind: &'static str,
    category: CleanupCategory,
) -> PrefixSpec {
    PrefixSpec {
        folder,
        prefix: starts_with,
        kind,
        category,
    }
}

use CleanupCategory::{Deployment, LoginAndCache, Memories, Memory, Sessions, Setup};

const CLAUDE: Table = Table {
    entries: &[
        entry(".claude/keysmith", "saved", Deployment),
        entry(".claude/rules", "saved", Setup),
        entry(".claude/agents", "saved", Setup),
        entry(".claude/commands", "saved", Setup),
        entry(".claude/hooks", "saved", Setup),
        entry(".claude/skills", "saved", Setup),
        entry(".claude/output-styles", "saved", Setup),
        entry(
            ".claude/settings.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(
            ".claude/settings.local.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(".claude.json", "saved-without-login", LoginAndCache),
        entry(".claude/projects", "saved", Sessions),
        entry(".claude/sessions", "saved", Sessions),
        entry(".claude/history.jsonl", "saved", Sessions),
        entry(".claude/file-history", "saved", Sessions),
        entry(".claude/plans", "saved", Sessions),
        entry(".claude/todos", "saved", Sessions),
        entry(".claude/teams", "saved", Sessions),
        entry(".claude/.credentials.json", "erased", LoginAndCache),
        entry(".claude/plugins", "erased", LoginAndCache),
        entry(".claude/logs", "erased", LoginAndCache),
        entry(".claude/session-env", "erased", LoginAndCache),
        entry(".claude/shell-snapshots", "erased", LoginAndCache),
        entry(".claude/paste-cache", "erased", LoginAndCache),
        entry(".claude/debug", "erased", LoginAndCache),
        entry(".claude/downloads", "erased", LoginAndCache),
        entry(".claude/cache", "erased", LoginAndCache),
        entry(".claude/backups", "erased", LoginAndCache),
        entry(".claude/ide", "erased", LoginAndCache),
        entry(".claude/statsig", "erased", LoginAndCache),
        entry(".claude/telemetry", "erased", LoginAndCache),
        entry(".claude/config.json", "erased", LoginAndCache),
        entry(".claude/policy-limits.json", "erased", LoginAndCache),
        entry(".claude/.last-cleanup", "erased", LoginAndCache),
        entry(".claude/.last-update-result.json", "erased", LoginAndCache),
    ],
    prefixes: &[
        prefix(".claude", "CLAUDE.md.", "saved", Memory),
        prefix(".claude", "settings.json.", "erased", LoginAndCache),
        prefix(".claude", "settings.local.json.", "erased", LoginAndCache),
        prefix("", ".claude.json.", "erased", LoginAndCache),
    ],
    adapter_owned: &[],
    folder: ".claude",
    keychain: &["Claude Code-credentials"],
    launch_agents: &[],
};

const CODEX: Table = Table {
    entries: &[
        entry(".codex/.codex-keysmith-manifest.json", "saved", Deployment),
        entry(".codex/rules", "saved", Setup),
        entry(".codex/agents", "saved", Setup),
        entry(".codex/skills", "saved", Setup),
        entry(".codex/prompts", "saved", Setup),
        entry(".codex/hooks.json", "saved", Setup),
        entry(".codex/sessions", "saved", Sessions),
        entry(".codex/archived_sessions", "saved", Sessions),
        entry(".codex/session_index.jsonl", "saved", Sessions),
        entry(".codex/history.jsonl", "saved", Sessions),
        entry(".codex/transcription-history.jsonl", "saved", Sessions),
        entry(".codex/dictation-history", "saved", Sessions),
        entry(".codex/generated_images", "saved", Sessions),
        entry(".codex/attachments", "saved", Sessions),
        entry(".codex/state_5.sqlite", "saved", Sessions),
        entry(".codex/memories_1.sqlite", "saved", Sessions),
        entry(".codex/goals_1.sqlite", "saved", Sessions),
        entry(".codex/queue_1.sqlite", "saved", Sessions),
        entry(".codex/thread_history_1.sqlite", "saved", Sessions),
        entry(".codex/auth.json", "erased", LoginAndCache),
        entry(".codex/plugins", "erased", LoginAndCache),
        entry(".codex/shell_snapshots", "erased", LoginAndCache),
        entry(".codex/log", "erased", LoginAndCache),
        entry(".codex/logs", "erased", LoginAndCache),
        entry(".codex/cache", "erased", LoginAndCache),
        entry(".codex/tmp", "erased", LoginAndCache),
        entry(".codex/backups", "erased", LoginAndCache),
        entry(".codex/runtime-backups", "erased", LoginAndCache),
        entry(".codex/memory-backups", "erased", LoginAndCache),
        entry(".codex/models_cache.json", "erased", LoginAndCache),
        entry(".codex/installation_id", "erased", LoginAndCache),
        entry(".codex/.codex-global-state.json", "erased", LoginAndCache),
        entry(".codex/logs_2.sqlite", "erased", LoginAndCache),
        entry(".codex/vendor_imports", "erased", LoginAndCache),
    ],
    prefixes: &[
        prefix(".codex", "AGENTS.md.", "saved", Memory),
        prefix(".codex", "auth.json.", "erased", LoginAndCache),
        prefix(".codex", "config.toml.", "erased", LoginAndCache),
        prefix(".codex", "logs_2.sqlite", "erased", LoginAndCache),
        prefix(".codex", "state_5.sqlite-", "saved", Sessions),
        prefix(".codex", "memories_1.sqlite-", "saved", Sessions),
        prefix(".codex", "goals_1.sqlite-", "saved", Sessions),
        prefix(".codex", "queue_1.sqlite-", "saved", Sessions),
        prefix(".codex", "thread_history_1.sqlite-", "saved", Sessions),
    ],
    adapter_owned: &[],
    folder: ".codex",
    keychain: &["Codex Auth"],
    launch_agents: &[],
};

/// Grok keeps its program in `bin`, `bundled`, `vendor` and `downloads`: those are never named.
/// `Grok Bot` is a separate app and is not touched.
const GROK: Table = Table {
    entries: &[
        entry(".grok/.grok-keysmith-manifest.json", "saved", Deployment),
        entry(".grok/rules", "saved", Setup),
        entry(".grok/agents", "saved", Setup),
        entry(".grok/skills", "saved", Setup),
        entry(".grok/commands", "saved", Setup),
        entry(".grok/config.toml", "saved-without-login", LoginAndCache),
        entry(".grok/AGENTS.md", "saved", Memory),
        entry(".grok/memory", "saved", Memories),
        entry(".grok/memory-v2", "saved", Memories),
        entry(".grok/sessions", "saved", Sessions),
        entry(".grok/memtrace", "saved", Sessions),
        entry(".grok/active_sessions.json", "saved", Sessions),
        entry(".grok/active_sessions.lock", "saved", Sessions),
        entry(".grok/campaigns_state.json", "saved", Sessions),
        entry(".grok/campaigns_state.json.lock", "saved", Sessions),
        entry(".grok/worktrees.db", "saved", Sessions),
        entry(".grok/auth.json", "erased", LoginAndCache),
        entry(".grok/logs", "erased", LoginAndCache),
        entry(".grok/installed-plugins", "erased", LoginAndCache),
        entry(".grok/marketplace-cache", "erased", LoginAndCache),
        entry(".grok/models_cache.json", "erased", LoginAndCache),
        entry(".grok/settings_cache.json", "erased", LoginAndCache),
        entry(".grok/slash-mru.json", "erased", LoginAndCache),
        entry(".grok/tip_cursor.json", "erased", LoginAndCache),
        entry(".grok/upload_queue", "erased", LoginAndCache),
    ],
    prefixes: &[
        prefix(".grok", "AGENTS.md.", "saved", Memory),
        prefix(".grok", "worktrees.db-", "saved", Sessions),
        prefix(".grok", "auth.json", "erased", LoginAndCache),
        prefix(".grok", "config.toml.", "erased", LoginAndCache),
    ],
    adapter_owned: &[(".grok", "config.toml.keysmith-backup-")],
    folder: ".grok",
    keychain: &[],
    launch_agents: &[],
};

/// ZCode's `~/.zcode/workspace` is where its projects live (git projects inside): never named.
const ZCODE: Table = Table {
    entries: &[
        entry(".zcode-keysmith/system-role.md", "saved", Deployment),
        entry(".zcode-keysmith/config.json", "saved", Deployment),
        entry(".zcode-keysmith/runtime-shim.json", "saved", Deployment),
        entry(".zcode-keysmith/bin", "saved", Deployment),
        entry(".zcode/AGENTS.md", "saved", Memory),
        entry(".zcode/commands", "saved", Setup),
        entry(".zcode/skills", "saved", Setup),
        entry(
            ".zcode/v2/config.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(
            ".zcode/v2/setting.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(
            ".zcode/v2/provider_config.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(
            ".zcode/v2/bot-config.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(
            ".zcode/v2/bot-config.v3.json",
            "saved-without-login",
            LoginAndCache,
        ),
        entry(".zcode/cli/memories", "saved", Memories),
        entry(".zcode/v2/tasks-index.sqlite", "saved", Sessions),
        entry(".zcode/cli/db", "saved", Sessions),
        entry(".zcode/cli/rollout", "saved", Sessions),
        entry(".zcode/cli/exec", "saved", Sessions),
        entry(".zcode/cli/artifacts", "saved", Sessions),
        entry(".zcode/cli/agents", "saved", Sessions),
        entry(
            "Library/Application Support/ZCode/session",
            "saved",
            Sessions,
        ),
        entry(".zcode/v2/credentials.json", "erased", LoginAndCache),
        entry(".zcode/v2/onboarding-record.json", "erased", LoginAndCache),
        entry(".zcode/v2/bot-state.v2.json", "erased", LoginAndCache),
        entry(".zcode/v2/bot-state.v3.json", "erased", LoginAndCache),
        entry(
            ".zcode/v2/bots-model-cache.v2.json",
            "erased",
            LoginAndCache,
        ),
        entry(".zcode/v2/coding-plan-cache.json", "erased", LoginAndCache),
        entry(".zcode/v2/telemetry-state.json", "erased", LoginAndCache),
        entry(".zcode/v2/cache", "erased", LoginAndCache),
        entry(".zcode/v2/logs", "erased", LoginAndCache),
        entry(".zcode/v2/crash", "erased", LoginAndCache),
        entry(".zcode/cli/log", "erased", LoginAndCache),
        entry(".zcode/cli/image-cache", "erased", LoginAndCache),
        entry(".zcode/cli/pdf-cache", "erased", LoginAndCache),
        entry(".zcode/cli/plugins", "erased", LoginAndCache),
        entry(".zcode/backups", "erased", LoginAndCache),
        entry(".zcode/tmp", "erased", LoginAndCache),
        entry(".zcode-keysmith/backups", "erased", LoginAndCache),
        entry(".zcode-keysmith/cache", "erased", LoginAndCache),
        entry(".zcode-keysmith/logs", "erased", LoginAndCache),
        entry(
            "Library/Application Support/ZCode/rum-electron-store",
            "erased",
            LoginAndCache,
        ),
        entry(
            "Library/Application Support/ZCode/zcode-data-size-telemetry.json",
            "erased",
            LoginAndCache,
        ),
        entry("Library/Caches/dev.zcode.app", "erased", LoginAndCache),
    ],
    prefixes: &[
        prefix(
            "Library/LaunchAgents",
            "com.jia.zcode-keysmith.",
            "saved",
            Deployment,
        ),
        prefix(".zcode/v2", "tasks-index.sqlite-", "saved", Sessions),
        prefix(".zcode/v2", "config.json.", "erased", LoginAndCache),
        prefix(".zcode/v2", "credentials.json", "erased", LoginAndCache),
        prefix(".zcode/cli", "config.json.", "erased", LoginAndCache),
    ],
    adapter_owned: &[],
    folder: ".zcode",
    keychain: &["ZCode Safe Storage"],
    launch_agents: &["com.jia.zcode-keysmith.env", "com.jia.zcode-keysmith.rearm"],
};

fn table(tool: ToolKind) -> Option<&'static Table> {
    match tool {
        ToolKind::Claude => Some(&CLAUDE),
        ToolKind::Codex => Some(&CODEX),
        ToolKind::Grok => Some(&GROK),
        ToolKind::Zcode => Some(&ZCODE),
        // pi keeps nothing for Switch to clear besides its deployment, which the adapter's own
        // uninstall takes away. Its sessions, settings and AGENTS.md are left as they are.
        ToolKind::Pi => None,
        // dsh keeps its whole home: settings.yaml holds provider config, .credentials.yaml the
        // credentials themselves, and sessions/storages the transcripts. None of that is
        // Keysmith's to erase, and the deployment is the adapter's own uninstall to remove.
        ToolKind::Dsh => None,
    }
}

fn legacy_extra_category(tool: ToolKind, name: &str, kind: &str) -> CleanupCategory {
    if let Some(table) = table(tool) {
        if let Some(entry) = table.entries.iter().find(|entry| entry.name == name) {
            return entry.category;
        }
        let (folder, file) = name.rsplit_once('/').unwrap_or(("", name));
        if let Some(prefix) = table.prefixes.iter().find(|prefix| {
            prefix.folder == folder
                && file.starts_with(prefix.prefix)
                && file.len() > prefix.prefix.len()
        }) {
            return prefix.category;
        }
    }
    if kind == "erased" {
        LoginAndCache
    } else {
        Setup
    }
}

/// Login and credential files. A saved folder holding one is left where it is.
const SECRET_NAMES: [&str; 5] = [
    "auth.json",
    ".credentials.json",
    "credentials.json",
    ".netrc",
    ".env",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentMeta {
    pub present: bool,
    pub title: Option<String>,
    /// The live text could be saved, so a rollback can deploy it again.
    pub restorable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMeta {
    pub path: String,
    pub bytes: u64,
    pub lines: u64,
    pub sha256: String,
    pub mode: Option<u32>,
}

/// One more piece of the agent's user-level setup that a fresh install does not have (rules,
/// custom agents, commands, a leftover wrapper). It is moved whole into the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraMeta {
    /// Its place under the home folder, such as `.claude/rules` or `.codex/config.toml`.
    pub name: String,
    pub path: String,
    pub files: u64,
    pub bytes: u64,
    /// `saved`, `saved-without-login` (a copy with every key removed) or `erased` (not saved).
    #[serde(default = "kind_saved")]
    pub kind: String,
    #[serde(default)]
    pub category: CleanupCategory,
}

fn kind_saved() -> String {
    "saved".into()
}

/// An agent's own memory folder (Codex: `~/.codex/memories`), counted but never read.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoriesMeta {
    pub path: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AbsentExtra {
    pub name: String,
    #[serde(default)]
    pub category: CleanupCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMeta {
    pub id: String,
    pub created_at: String,
    pub tool: ToolKind,
    /// "cleanup" or "before-rollback".
    pub kind: String,
    pub deployment: DeploymentMeta,
    pub memory: Option<MemoryMeta>,
    /// The agent's memory folder, kept inside the snapshot's own `memories` folder.
    #[serde(default)]
    pub memories: Option<MemoriesMeta>,
    /// The rest of the user-level setup, kept inside the snapshot's own `extras` folder.
    #[serde(default)]
    pub extras: Vec<ExtraMeta>,
    /// Saved paths created by the rollback and moved out again when undoing it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub absent_extras: Vec<AbsentExtra>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub operation_id: String,
    pub tool: ToolKind,
    pub deployment: DeploymentMeta,
    pub memory: Option<MemoryMeta>,
    /// The agent's memory folder, if it has one with something in it.
    pub memories: Option<MemoriesMeta>,
    /// The rest of the user-level setup that a fresh install does not have.
    pub extras: Vec<ExtraMeta>,
    /// The agent's login is in the system keychain: it is removed, and never saved.
    pub login_in_keychain: bool,
    /// The agent is running now, so its sessions, memories and login are in use.
    pub agent_running: bool,
    /// The deployed config was changed by hand: it is left in place, the rest is still cleaned.
    pub config_drifted: bool,
    pub nothing_to_do: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub snapshot_id: Option<String>,
    pub deactivated: bool,
    pub memory_cleared: bool,
    pub memories_cleared: bool,
    pub extras_cleared: u64,
    pub sessions_cleared: u64,
    /// Login and cache entries permanently removed.
    pub erased: u64,
    pub keychain_cleared: bool,
    /// Input rewrite was connected for this agent and has been disconnected.
    pub rewrite_disconnected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackMemory {
    pub restore_bytes: u64,
    pub current_bytes: u64,
    pub current_differs: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackMemories {
    pub restore_files: u64,
    pub restore_bytes: u64,
    pub current_files: u64,
    pub current_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackPlan {
    pub operation_id: String,
    pub snapshot: SnapshotMeta,
    pub current_title: Option<String>,
    pub replaces_deployment: bool,
    pub memory: Option<RollbackMemory>,
    pub memories: Option<RollbackMemories>,
    /// How many saved pieces of setup are put back.
    pub extras: u64,
    pub agent_running: bool,
    /// What is there now is saved as a snapshot first.
    pub saves_current: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    pub saved_snapshot_id: Option<String>,
    pub redeployed: bool,
    pub memory_restored: bool,
    pub memories_restored: bool,
    pub extras_restored: u64,
    pub sessions_restored: u64,
}

fn sha_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn snapshots_dir(store: &Store) -> PathBuf {
    store.paths().home.join("snapshots")
}

/// Ids are written by this module only, but they arrive from the interface: refuse anything
/// that is not one, so an id can never name a path outside the snapshots folder.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn snapshot_path(store: &Store, id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        return Err(Error::invalid("invalid snapshot id"));
    }
    let path = snapshots_dir(store).join(id);
    require_contained(&store.paths().home, &path)?;
    Ok(path)
}

/// Every complete snapshot, newest first. A folder without its `meta.json` is a write that
/// never finished and is not listed.
pub fn list_snapshots(store: &Store) -> Vec<SnapshotMeta> {
    let root = snapshots_dir(store);
    if require_contained(&store.paths().home, &root).is_err() {
        return Vec::new();
    }
    let mut found: Vec<SnapshotMeta> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| read_snapshot(store, entry.file_name().to_str()?).ok())
        .collect();
    found.sort_by(|left, right| right.id.cmp(&left.id));
    found
}

pub fn delete_snapshot(store: &Store, id: &str) -> Result<()> {
    let path = snapshot_path(store, id)?;
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn read_snapshot(store: &Store, id: &str) -> Result<SnapshotMeta> {
    let path = snapshot_path(store, id)?.join("meta.json");
    require_contained(&store.paths().home, &path)?;
    let bytes = std::fs::read(path).map_err(|_| Error::invalid("snapshot not found"))?;
    let damaged = |_| Error::invalid("snapshot is damaged");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(damaged)?;
    let tool = serde_json::from_value::<ToolKind>(value["tool"].clone()).map_err(damaged)?;
    if let Some(extras) = value["extras"].as_array_mut() {
        for extra in extras
            .iter_mut()
            .filter_map(serde_json::Value::as_object_mut)
        {
            if !extra.contains_key("category") {
                let name = extra
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let kind = extra
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("saved");
                extra.insert(
                    "category".into(),
                    json!(legacy_extra_category(tool, name, kind)),
                );
            }
        }
    }
    let meta: SnapshotMeta = serde_json::from_value(value).map_err(damaged)?;
    if meta.id != id {
        return Err(Error::invalid("snapshot is damaged: its ID does not match"));
    }
    Ok(meta)
}

fn snapshot_prompt(store: &Store, meta: &SnapshotMeta) -> Result<Option<String>> {
    if !meta.deployment.restorable {
        return Ok(None);
    }
    let path = snapshot_path(store, &meta.id)?.join("prompt.md");
    require_contained(&store.paths().home, &path)?;
    let bytes = std::fs::read(path)
        .map_err(|_| Error::invalid("snapshot is damaged: its prompt is missing"))?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| Error::invalid("snapshot is damaged: its prompt is unreadable"))
}

fn snapshot_memory(store: &Store, meta: &SnapshotMeta) -> Result<Option<Vec<u8>>> {
    let Some(memory) = &meta.memory else {
        return Ok(None);
    };
    let path = snapshot_path(store, &meta.id)?.join("memory.bin");
    require_contained(&store.paths().home, &path)?;
    let bytes = std::fs::read(path)
        .map_err(|_| Error::invalid("snapshot is damaged: its memory copy is missing"))?;
    if sha_hex(&bytes) != memory.sha256 {
        return Err(Error::invalid(
            "snapshot is damaged: its memory copy does not match",
        ));
    }
    Ok(Some(bytes))
}

/// The memory folder kept in a snapshot, checked against what the snapshot says it holds.
fn snapshot_memories(store: &Store, meta: &SnapshotMeta) -> Result<Option<PathBuf>> {
    let Some(saved) = &meta.memories else {
        return Ok(None);
    };
    let dir = memories_in_snapshot(store, &meta.id)?;
    let info = memories_info(&dir)
        .map_err(|_| Error::invalid("snapshot is damaged: its memories folder is missing"))?;
    if info.files != saved.files || info.bytes != saved.bytes {
        return Err(Error::invalid(
            "snapshot is damaged: its memories folder does not match",
        ));
    }
    Ok(Some(dir))
}

fn home_dir(opts: &AdapterOptions) -> Result<PathBuf> {
    opts.home
        .clone()
        .or_else(dirs::home_dir)
        .ok_or_else(|| Error::invalid("the home folder is unknown"))
}

/// Where an agent's user-level memory file is, for the agents that have one.
fn memory_expected(tool: ToolKind, opts: &AdapterOptions) -> Result<Option<PathBuf>> {
    let home = home_dir(opts)?;
    Ok(match tool {
        ToolKind::Claude => Some(home.join(".claude").join("CLAUDE.md")),
        ToolKind::Codex => Some(home.join(".codex").join("AGENTS.md")),
        _ => None,
    })
}

/// The user-level memory file, for the agents that have one Keysmith knows about (Claude Code's
/// CLAUDE.md, Codex's AGENTS.md). It is accepted only when it is exactly the file in the user's
/// own home and an ordinary file. Claude's path comes from the adapter and is checked against
/// that place; Codex's adapter does not report it, so the place itself is used.
fn memory_file(
    tool: ToolKind,
    envelope: &Envelope,
    opts: &AdapterOptions,
) -> Result<Option<PathBuf>> {
    let Some(expected) = memory_expected(tool, opts)? else {
        return Ok(None);
    };
    if tool == ToolKind::Codex {
        require_contained(&home_dir(opts)?, &expected)?;
        return match std::fs::symlink_metadata(&expected) {
            Ok(meta) if meta.is_file() => Ok(Some(expected)),
            Ok(_) => Err(Error::invalid("the memory file is not an ordinary file")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        };
    }
    let Some(target) = envelope
        .target_paths
        .iter()
        .find(|target| target.role == "memory" && target.exists)
    else {
        return Ok(None);
    };
    let home = home_dir(opts)?;
    let path = Path::new(&target.path);
    // The adapter can resolve aliases above the user's home when reporting the path.
    let canonical_expected = std::fs::canonicalize(&home)
        .ok()
        .map(|root| root.join(".claude/CLAUDE.md"));
    if path != expected && canonical_expected.as_deref() != Some(path) {
        return Err(Error::invalid(
            "the memory file is not the user-level CLAUDE.md; nothing was changed",
        ));
    }
    require_contained(&home, &expected)?;
    let meta = std::fs::symlink_metadata(&expected)
        .map_err(|_| Error::invalid("the memory file is gone"))?;
    if !meta.is_file() {
        return Err(Error::invalid("the memory file is not an ordinary file"));
    }
    Ok(Some(expected))
}

/// Codex's own memory folder, when it has one. A link, or anything that is not a folder, is
/// refused: the folder is moved as a whole, and only the real one in the user's home.
fn memories_dir(tool: ToolKind, opts: &AdapterOptions) -> Result<Option<PathBuf>> {
    if tool != ToolKind::Codex {
        return Ok(None);
    }
    let home = home_dir(opts)?;
    let path = home.join(".codex").join("memories");
    require_contained(&home, &path)?;
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_dir() => Ok(Some(path)),
        Ok(_) => Err(Error::invalid(
            "the memories folder is a link or not a folder; nothing was changed",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// How many files and bytes a folder holds. Links are counted, never followed, and nothing is read.
fn dir_stats(root: &Path) -> Result<(u64, u64)> {
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let meta = std::fs::symlink_metadata(entry.path())?;
            if !is_link(&meta) && meta.is_dir() {
                pending.push(entry.path());
            } else {
                files += 1;
                bytes += meta.len();
            }
        }
    }
    Ok((files, bytes))
}

fn memories_info(path: &Path) -> Result<MemoriesMeta> {
    let (files, bytes) = dir_stats(path)?;
    Ok(MemoriesMeta {
        path: path.to_string_lossy().into_owned(),
        files,
        bytes,
    })
}

/// The folder as it is now, or `None` when it has nothing in it.
fn current_memories(tool: ToolKind, opts: &AdapterOptions) -> Result<Option<MemoriesMeta>> {
    Ok(match memories_dir(tool, opts)? {
        Some(path) => Some(memories_info(&path)?).filter(|info| info.files > 0),
        None => None,
    })
}

/// Whether the agent may be using its sessions, login or memories. Isolated homes skip the
/// system process listing unless a test explicitly exercises the real-home path.
fn agent_running(tool: ToolKind, opts: &AdapterOptions) -> bool {
    if table(tool).is_none() {
        return false;
    }
    if let Some(forced) = opts.extra_env.get(CODEX_RUNNING_KEY) {
        return forced == "1";
    }
    if opts.home.is_some() {
        return false;
    }
    #[cfg(unix)]
    let listing = std::process::Command::new("ps")
        .args(["-axo", "comm="])
        .output();
    #[cfg(windows)]
    use crate::nowindow::NoWindow;
    #[cfg(windows)]
    let listing = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .no_window()
        .output();
    #[cfg(not(any(unix, windows)))]
    return true;
    #[cfg(any(unix, windows))]
    running_in_process_listing(tool, listing)
}

#[cfg(any(unix, windows))]
fn running_in_process_listing(
    tool: ToolKind,
    listing: std::io::Result<std::process::Output>,
) -> bool {
    let Ok(out) = listing else {
        return true;
    };
    !out.status.success()
        || String::from_utf8_lossy(&out.stdout).lines().any(|line| {
            let name = line.trim().trim_start_matches('"');
            let name = name.split('"').next().unwrap_or(name);
            let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
            let base = base.to_ascii_lowercase();
            let base = base.trim_end_matches(".exe");
            match tool {
                ToolKind::Codex => base == "codex",
                // The Claude desktop app (`Claude.app`) is another program with its own data.
                ToolKind::Claude => {
                    !name.contains(".app/")
                        && (base == "claude" || name.contains("/claude/versions/"))
                }
                ToolKind::Grok => base == "grok",
                // The app, its helpers and its command line all hold the data.
                ToolKind::Zcode => base.starts_with("zcode"),
                ToolKind::Pi => base == "pi",
                ToolKind::Dsh => base == "dsh",
            }
        })
}

fn memories_in_snapshot(store: &Store, id: &str) -> Result<PathBuf> {
    let path = snapshot_path(store, id)?.join("memories");
    require_contained(&store.paths().home, &path)?;
    Ok(path)
}

/// The agent's setup lives in the user's home folder; every piece is named from there.
fn agent_home(tool: ToolKind, opts: &AdapterOptions) -> Result<Option<PathBuf>> {
    Ok(match table(tool) {
        Some(_) => Some(home_dir(opts)?),
        None => None,
    })
}

/// A plain relative path with no way out of the folder it is joined to.
fn valid_extra_name(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

fn is_link(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    meta.file_type().is_symlink()
}

/// Check each existing component below the trusted root before descending into it.
fn is_contained(root: &Path, path: &Path) -> Result<bool> {
    use std::path::Component;

    let Ok(relative) = path.strip_prefix(root) else {
        return Ok(false);
    };
    if !relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Ok(false);
    }
    let mut current = root.to_path_buf();
    let mut parts = relative.components().peekable();
    while let Some(part) = parts.next() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if is_link(&meta) || (parts.peek().is_some() && !meta.is_dir()) => {
                return Ok(false);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

fn require_contained(root: &Path, path: &Path) -> Result<()> {
    if !is_contained(root, path)? {
        return Err(Error::invalid(
            "a cleanup path crosses a link or invalid ancestor",
        ));
    }
    Ok(())
}

fn is_secret_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    SECRET_NAMES.iter().any(|secret| {
        name.strip_prefix(secret)
            .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('.'))
    })
}

/// One piece of setup: its place under the home folder and what is done to it.
type Entry = (String, String, CleanupCategory);

/// Files in `folder` (under home) whose names start with `prefix`, as places under home.
fn prefixed(home: &Path, folder: &str, prefix: &str) -> Result<Vec<String>> {
    let dir = if folder.is_empty() {
        home.to_path_buf()
    } else {
        home.join(folder)
    };
    if !is_contained(home, &dir)? {
        return Ok(Vec::new());
    }
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(prefix) && name.len() > prefix.len())
        .map(|name| {
            if folder.is_empty() {
                name
            } else {
                format!("{folder}/{name}")
            }
        })
        .collect();
    found.sort();
    Ok(found)
}

/// Which pieces to look at, in a fixed order: the agent's tables, the old copies of its config
/// files, and the wrapper the adapter reports when it lies inside the agent's folder.
fn extra_entries(
    store: &Store,
    tool: ToolKind,
    envelope: &Envelope,
    selection: CleanupSelection,
    opts: &AdapterOptions,
) -> Result<Vec<Entry>> {
    let (Some(table), Some(home)) = (table(tool), agent_home(tool, opts)?) else {
        return Ok(Vec::new());
    };
    let mut entries: Vec<Entry> = Vec::new();
    let mut add = |name: String, kind: &str, category: CleanupCategory| {
        if !entries.iter().any(|(known, _, _)| *known == name) {
            entries.push((name, kind.to_string(), category));
        }
    };
    for item in table.entries {
        add(item.name.to_string(), item.kind, item.category);
    }
    for item in table.prefixes {
        if !category_selected(&selection, item.category) {
            continue;
        }
        for name in prefixed(&home, item.folder, item.prefix)? {
            let owned = item.kind == "erased"
                && table
                    .adapter_owned
                    .iter()
                    .any(|(owned_folder, owned_prefix)| {
                        name.strip_prefix(&format!("{owned_folder}/"))
                            .is_some_and(|file| file.starts_with(owned_prefix))
                    });
            if !owned {
                add(name, item.kind, item.category);
            }
        }
    }
    // Skills a harness pack installed for Codex live in the shared `~/.agents/skills`.
    if selection.setup && tool == ToolKind::Codex {
        for name in crate::components::codex_shared_skills(store, &home) {
            add(name, "saved", CleanupCategory::Setup);
        }
    }
    if selection.deployment {
        let agent_folder = home.join(table.folder);
        for target in envelope.target_paths.iter().filter(|t| t.role == "wrapper") {
            let path = Path::new(&target.path);
            let Ok(relative) = path.strip_prefix(&home) else {
                continue;
            };
            let name = relative.to_string_lossy().replace('\\', "/");
            let reserved = table.prefixes.iter().any(|item| {
                path.parent() == Some(home.join(item.folder).as_path())
                    && path
                        .file_name()
                        .is_some_and(|file| file.to_string_lossy().starts_with(item.prefix))
            });
            if path.starts_with(&agent_folder) && valid_extra_name(&name) && !reserved {
                add(name, "saved", CleanupCategory::Deployment);
            }
        }
    }
    entries.retain(|(_, _, category)| category_selected(&selection, *category));
    Ok(entries)
}

/// Whether a folder holds a git project or a login file anywhere inside. Links are not followed.
fn holds_project_or_secret(root: &Path) -> Result<bool> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == ".git" || is_secret_name(&name) {
                return Ok(true);
            }
            let meta = std::fs::symlink_metadata(entry.path())?;
            if !is_link(&meta) && meta.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(false)
}

/// How many files and bytes a file or folder holds.
fn path_stats(path: &Path) -> Result<(u64, u64)> {
    let meta = std::fs::symlink_metadata(path)?;
    if is_link(&meta) || (!meta.is_dir() && !meta.is_file()) {
        return Err(Error::invalid(
            "a cleanup path is not an ordinary file or folder",
        ));
    }
    if meta.is_dir() {
        dir_stats(path)
    } else {
        Ok((1, meta.len()))
    }
}

/// One piece of setup as it is now, or `None` when it is missing, empty or a link. A saved folder
/// that holds a git project or a login file is left alone as well, and so is a saved file that
/// is itself a login file.
fn extra_info(
    home: &Path,
    name: &str,
    kind: &str,
    category: CleanupCategory,
) -> Result<Option<ExtraMeta>> {
    if !valid_extra_name(name) {
        return Ok(None);
    }
    let path = home.join(name);
    if !is_contained(home, &path)? {
        return Ok(None);
    }
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if is_link(&meta) || (!meta.is_dir() && !meta.is_file()) {
        return Ok(None);
    }
    match kind {
        "saved" => {
            if path.file_name().is_some_and(is_secret_name) {
                return Ok(None);
            }
            if meta.is_dir() && holds_project_or_secret(&path)? {
                return Ok(None);
            }
        }
        "saved-without-login" if !meta.is_file() => return Ok(None),
        "saved-without-login" | "erased" => {}
        _ => return Ok(None),
    }
    let (files, bytes) = path_stats(&path)?;
    Ok(Some(ExtraMeta {
        name: name.to_string(),
        path: path.to_string_lossy().into_owned(),
        files,
        bytes,
        kind: kind.to_string(),
        category,
    }))
}

/// The database name shared by a saved SQLite file and all of its sidecars.
fn sqlite_database<'a>(name: &'a str, kind: &str) -> Option<&'a str> {
    if kind != "saved" {
        return None;
    }
    let file = name.rsplit('/').next()?;
    [".sqlite", ".db"]
        .into_iter()
        .filter_map(|extension| {
            file.find(&format!("{extension}-"))
                .map(|offset| offset + extension.len())
                .or_else(|| file.ends_with(extension).then_some(file.len()))
        })
        .min()
        .map(|end| &name[..name.len() - file.len() + end])
}

fn sqlite_sidecars(root: &Path, database: &str, strict: bool) -> Result<Vec<String>> {
    let (folder, file) = database.rsplit_once('/').unwrap_or(("", database));
    let directory = root.join(folder);
    if !is_contained(root, &directory)? {
        return Ok(Vec::new());
    }
    let siblings = match std::fs::read_dir(directory) {
        Ok(siblings) => siblings,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let prefix = format!("{file}-");
    let mut found = Vec::new();
    for sibling in siblings {
        let sibling = sibling?;
        let basename = sibling.file_name();
        if !basename.to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let name = basename.to_str().map(|file| {
            if folder.is_empty() {
                file.to_string()
            } else {
                format!("{folder}/{file}")
            }
        });
        let meta = std::fs::symlink_metadata(sibling.path())?;
        match name {
            Some(name) if valid_extra_name(&name) && meta.is_file() && !is_link(&meta) => {
                found.push(name);
            }
            _ if strict => {
                return Err(Error::command_failed(format!(
                    "{database} has a sidecar that cannot be moved safely"
                )));
            }
            _ => {}
        }
    }
    found.sort();
    Ok(found)
}

fn sqlite_entries(root: &Path, database: &str, strict: bool) -> Result<Vec<String>> {
    let path = root.join(database);
    if !is_contained(root, &path)? {
        return Ok(Vec::new());
    }
    let present = match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() && !is_link(&meta) => true,
        Ok(_) => {
            return Err(Error::command_failed(format!(
                "{database} is not an ordinary database file"
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    let mut names = sqlite_sidecars(root, database, strict || present)?;
    if present {
        names.insert(0, database.to_string());
    }
    Ok(names)
}

fn current_extras(
    tool: ToolKind,
    entries: &[Entry],
    for_rollback: bool,
    opts: &AdapterOptions,
) -> Result<Vec<ExtraMeta>> {
    let Some(home) = agent_home(tool, opts)? else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    let mut databases = std::collections::HashSet::new();
    for (name, kind, category) in entries {
        if !valid_extra_name(name) {
            continue;
        }
        if let Some(database) = sqlite_database(name, kind) {
            if databases.insert(database) {
                for member in sqlite_entries(&home, database, for_rollback)? {
                    let info = extra_info(&home, &member, kind, *category)?.ok_or_else(|| {
                        Error::command_failed(format!(
                            "{database} changed while checking its sidecars"
                        ))
                    })?;
                    found.push(info);
                }
            }
        } else if let Some(info) = extra_info(&home, name, kind, *category)? {
            if info.files > 0 || for_rollback {
                found.push(info);
            }
        }
    }
    Ok(found)
}

fn category_selected(selection: &CleanupSelection, category: CleanupCategory) -> bool {
    match category {
        CleanupCategory::Deployment => selection.deployment,
        CleanupCategory::Memory => selection.memory,
        CleanupCategory::Memories => selection.memories,
        CleanupCategory::Setup => selection.setup,
        CleanupCategory::Sessions => selection.sessions,
        CleanupCategory::LoginAndCache => selection.login_and_cache,
    }
}

/// What a rollback would move aside: the saved folders and files that are there now. A config file
/// that is there is left in place, since it may hold a login made since the cleanup.
fn current_rollback_pieces(
    tool: ToolKind,
    snapshot: &SnapshotMeta,
    opts: &AdapterOptions,
) -> Result<Vec<ExtraMeta>> {
    let entries: Vec<Entry> = snapshot
        .extras
        .iter()
        .filter(|extra| extra.kind == "saved")
        .map(|extra| (extra.name.clone(), extra.kind.clone(), extra.category))
        .chain(
            snapshot
                .absent_extras
                .iter()
                .map(|extra| (extra.name.clone(), kind_saved(), extra.category)),
        )
        .collect();
    let mut current = current_extras(tool, &entries, true, opts)?;
    let mut seen = std::collections::HashSet::new();
    current.retain(|extra| seen.insert(extra.name.clone()));
    if let Some(root) = agent_home(tool, opts)? {
        for extra in &snapshot.absent_extras {
            if !valid_extra_name(&extra.name) {
                return Err(Error::invalid(
                    "snapshot is damaged: an absent piece has a bad name",
                ));
            }
            let path = root.join(&extra.name);
            require_contained(&root, &path)?;
            if path_exists(&path)? && !current.iter().any(|piece| piece.name == extra.name) {
                return Err(Error::command_failed(format!(
                    "{} cannot be moved safely",
                    extra.name
                )));
            }
        }
    }
    Ok(current)
}

fn path_exists(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn missing_rollback_extras(
    home: Option<&Path>,
    snapshot: &SnapshotMeta,
) -> Result<Vec<AbsentExtra>> {
    let Some(root) = home else {
        return Ok(Vec::new());
    };
    let mut absent = Vec::new();
    for extra in snapshot.extras.iter().filter(|extra| extra.kind == "saved") {
        let path = root.join(&extra.name);
        if is_contained(root, &path)? && !path_exists(&path)? {
            absent.push(AbsentExtra {
                name: extra.name.clone(),
                category: extra.category,
            });
        }
    }
    Ok(absent)
}

fn absences_need_snapshot(snapshot: &SnapshotMeta, absent: &[AbsentExtra]) -> bool {
    // Claude's uninstall leaves its managed instruction backups in this directory even for a
    // memory-and-deployment cleanup. That directory alone must preserve the existing rollback
    // behavior. If other data needs a safety snapshot, record all restored absences in it.
    absent.iter().any(|extra| {
        !(snapshot.tool == ToolKind::Claude
            && snapshot.deployment.restorable
            && extra.category == Deployment
            && extra.name == ".claude/keysmith")
    })
}

fn rollback_needs_idle_agent(snapshot: &SnapshotMeta) -> bool {
    snapshot.memories.is_some()
        || snapshot
            .extras
            .iter()
            .map(|extra| extra.category)
            .chain(snapshot.absent_extras.iter().map(|extra| extra.category))
            .any(|category| matches!(category, Sessions | Memories))
}

/// What goes into a snapshot: everything except what is erased.
fn kept(extras: &[ExtraMeta]) -> Vec<ExtraMeta> {
    extras
        .iter()
        .filter(|e| e.kind != "erased")
        .cloned()
        .collect()
}

fn extra_in_snapshot(store: &Store, id: &str, name: &str) -> Result<PathBuf> {
    if !valid_extra_name(name) {
        return Err(Error::invalid(
            "snapshot is damaged: a saved piece has a bad name",
        ));
    }
    let path = snapshot_path(store, id)?.join("extras").join(name);
    require_contained(&store.paths().home, &path)?;
    Ok(path)
}

/// The pieces of setup kept in a snapshot. A saved piece is checked against what the snapshot
/// says it holds; a config copy made without its login has no original to compare with.
fn snapshot_extras(store: &Store, meta: &SnapshotMeta) -> Result<Vec<(ExtraMeta, PathBuf)>> {
    let mut found = Vec::new();
    for saved in &meta.extras {
        let path = extra_in_snapshot(store, &meta.id, &saved.name)?;
        let (files, bytes) = path_stats(&path).map_err(|_| {
            Error::invalid(format!("snapshot is damaged: {} is missing", saved.name))
        })?;
        let same = if saved.kind == "saved-without-login" {
            files == 1
        } else {
            files == saved.files && bytes == saved.bytes
        };
        if !same {
            return Err(Error::invalid(format!(
                "snapshot is damaged: {} does not match",
                saved.name
            )));
        }
        found.push((saved.clone(), path));
    }
    let root = snapshot_path(store, &meta.id)?.join("extras");
    for (database, group) in sqlite_groups(&found, |(extra, _)| extra) {
        let archived = sqlite_entries(&root, database, true)?;
        if archived.len() != group.len()
            || !group
                .iter()
                .all(|(extra, _)| archived.contains(&extra.name))
        {
            return Err(Error::invalid(format!(
                "snapshot is damaged: {database} has an incomplete SQLite group"
            )));
        }
    }
    Ok(found)
}

fn sqlite_groups<T>(
    entries: &[T],
    extra: impl Fn(&T) -> &ExtraMeta,
) -> std::collections::BTreeMap<&str, Vec<&T>> {
    let mut groups: std::collections::BTreeMap<_, Vec<_>> = Default::default();
    for entry in entries {
        let meta = extra(entry);
        if let Some(database) = sqlite_database(&meta.name, &meta.kind) {
            groups.entry(database).or_default().push(entry);
        }
    }
    groups
}

/// The saved pieces must be on the snapshots' volume, so moving them is a rename that cannot
/// lose anything. A config copy is written, not moved, and needs no such check.
fn check_extras_volume(store: &Store, extras: &[ExtraMeta]) -> Result<()> {
    let moved: Vec<&ExtraMeta> = extras.iter().filter(|e| e.kind == "saved").collect();
    if moved.is_empty() {
        return Ok(());
    }
    prepare_snapshots_dir(store)?;
    if let Some(extra) = moved
        .iter()
        .find(|e| !same_volume(Path::new(&e.path), &snapshots_dir(store)))
    {
        return Err(Error::command_failed(format!(
            "{} is on another disk than the snapshots, so it cannot be moved safely",
            extra.name
        )));
    }
    Ok(())
}

// ---- Config copies without their login ---------------------------------------------------

/// Key names that hold a login, a key or an account. A size setting such as `max_tokens` is not one.
fn secret_key(name: &str) -> bool {
    let lower = name.to_ascii_lowercase().replace(['-', '_'], "");
    let sizing = [
        "max", "limit", "budget", "context", "output", "input", "thinking", "count",
    ];
    if lower.contains("tokens")
        && !lower.contains("auth")
        && sizing.iter().any(|w| lower.contains(w))
    {
        return false;
    }
    [
        "token",
        "secret",
        "password",
        "passwd",
        "apikey",
        "credential",
        "oauth",
        "auth",
        "cookie",
        "bearer",
        "userid",
        "accountuuid",
        "emailaddress",
        "history",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

fn strip_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|key, inner| {
                if secret_key(key) {
                    return false;
                }
                if let serde_json::Value::String(text) = inner {
                    if crate::redact::redact_text(text) != *text {
                        return false;
                    }
                }
                true
            });
            map.values_mut().for_each(strip_json);
        }
        serde_json::Value::Array(items) => {
            items.retain(|item| {
                !matches!(item, serde_json::Value::String(text) if crate::redact::redact_text(text) != *text)
            });
            items.iter_mut().for_each(strip_json);
        }
        _ => {}
    }
}

struct StripTomlLogin;

impl toml_edit::visit_mut::VisitMut for StripTomlLogin {
    fn visit_table_like_mut(&mut self, table: &mut dyn toml_edit::TableLike) {
        let removed: Vec<String> = table
            .iter()
            .filter(|(key, item)| {
                secret_key(key) || item.as_value().is_some_and(toml_value_holds_login)
            })
            .map(|(key, _)| key.to_string())
            .collect();
        for key in removed {
            table.remove(&key);
        }
        toml_edit::visit_mut::visit_table_like_mut(self, table);
    }

    fn visit_array_mut(&mut self, array: &mut toml_edit::Array) {
        array.retain(|value| !toml_value_holds_login(value));
        toml_edit::visit_mut::visit_array_mut(self, array);
    }
}

fn toml_value_holds_login(value: &toml_edit::Value) -> bool {
    value
        .as_str()
        .is_some_and(|text| crate::redact::redact_text(text) != text)
}

/// The text of a config file with every key, token, account and history entry taken out. When
/// what is left still looks like it holds a login, nothing is saved and the cleanup stops.
fn sanitized_copy(path: &Path, name: &str) -> Result<Vec<u8>> {
    let raw = std::fs::read(path)?;
    let text = String::from_utf8(raw).map_err(|_| {
        Error::command_failed(format!(
            "{name} is not text, so it cannot be saved without its login"
        ))
    })?;
    let cleaned = if name.ends_with(".toml") {
        use toml_edit::visit_mut::VisitMut;
        let mut doc = text.parse::<toml_edit::DocumentMut>().map_err(|_| {
            Error::command_failed(format!(
                "{name} could not be read, so it cannot be saved without its login"
            ))
        })?;
        StripTomlLogin.visit_document_mut(&mut doc);
        doc.to_string()
    } else {
        let mut value: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
            Error::command_failed(format!(
                "{name} could not be read, so it cannot be saved without its login"
            ))
        })?;
        strip_json(&mut value);
        let mut pretty = serde_json::to_string_pretty(&value)?;
        pretty.push('\n');
        pretty
    };
    let holds_login = if name.ends_with(".toml") {
        cleaned.lines().any(|line| {
            // The redactor also recognizes token count assignments such as max_tokens = 4096.
            let is_setting = line.split_once('=').is_some_and(|(key, value)| {
                let value = value.trim();
                !secret_key(key.trim())
                    && (value.parse::<f64>().is_ok() || matches!(value, "true" | "false"))
            });
            !is_setting && crate::redact::redact_text(line) != line
        })
    } else {
        crate::redact::redact_text(&cleaned) != cleaned
    };
    if holds_login {
        return Err(Error::command_failed(format!(
            "{name} still holds a login after removing the keys; nothing was changed"
        )));
    }
    Ok(cleaned.into_bytes())
}

/// A copy of a folder, keeping links as links. The destination must not exist yet.
fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&source)?;
        if is_link(&meta) {
            #[cfg(unix)]
            std::os::unix::fs::symlink(std::fs::read_link(&source)?, &target)?;
            #[cfg(not(unix))]
            return Err(Error::invalid("a saved folder contains a reparse point"));
        } else if meta.is_dir() {
            copy_dir(&source, &target)?;
        } else if meta.is_file() {
            std::fs::copy(&source, &target)?;
        } else {
            return Err(Error::invalid(
                "a saved folder contains an unsupported file",
            ));
        }
    }
    Ok(())
}

/// Both paths must be on one volume for a move to be a rename that cannot lose anything.
#[cfg(unix)]
fn same_volume(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(left), std::fs::metadata(right)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => false,
    }
}

#[cfg(windows)]
fn same_volume(left: &Path, right: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::{AsRawHandle, RawHandle};

    const FILE_READ_ATTRIBUTES: u32 = 0x0080;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_ID_INFO_CLASS: i32 = 18;

    #[repr(C)]
    struct FileIdInfo {
        volume_serial_number: u64,
        file_id: [u8; 16],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandleEx(
            file: RawHandle,
            class: i32,
            info: *mut FileIdInfo,
            size: u32,
        ) -> i32;
    }

    fn volume(path: &Path) -> Option<u64> {
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .ok()?;
        let mut info = FileIdInfo {
            volume_serial_number: 0,
            file_id: [0; 16],
        };
        // SAFETY: the open handle and the FILE_ID_INFO output buffer are valid for the call.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FILE_ID_INFO_CLASS,
                &mut info,
                std::mem::size_of::<FileIdInfo>() as u32,
            )
        };
        (ok != 0).then_some(info.volume_serial_number)
    }

    volume(left).is_some_and(|serial| volume(right) == Some(serial))
}

#[cfg(not(any(unix, windows)))]
fn same_volume(_left: &Path, _right: &Path) -> bool {
    false
}

fn memory_info(path: &Path) -> Result<(MemoryMeta, Vec<u8>)> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_MEMORY_BYTES {
        return Err(Error::invalid("the memory file is too large to snapshot"));
    }
    let bytes = std::fs::read(path)?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        Some(meta.permissions().mode() & 0o7777)
    };
    #[cfg(not(unix))]
    let mode = None;
    Ok((
        MemoryMeta {
            path: path.to_string_lossy().into_owned(),
            bytes: bytes.len() as u64,
            lines: bytes.iter().filter(|b| **b == b'\n').count() as u64
                + u64::from(!bytes.is_empty() && bytes.last() != Some(&b'\n')),
            sha256: sha_hex(&bytes),
            mode,
        },
        bytes,
    ))
}

fn prepare_snapshots_dir(store: &Store) -> Result<PathBuf> {
    let root = snapshots_dir(store);
    require_contained(&store.paths().home, &root)?;
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

/// Write a snapshot, check what was written, and only then make it visible.
#[allow(clippy::too_many_arguments)]
fn create_snapshot(
    store: &Store,
    tool: ToolKind,
    kind: &str,
    deployment: DeploymentMeta,
    prompt: Option<&str>,
    memory: Option<(MemoryMeta, &[u8])>,
    memories: Option<MemoriesMeta>,
    extras: Vec<ExtraMeta>,
) -> Result<SnapshotMeta> {
    let id = format!(
        "{}-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        tool.as_str(),
        &uuid::Uuid::new_v4().simple().to_string()[..6]
    );
    let meta = SnapshotMeta {
        id: id.clone(),
        created_at: now_rfc3339(),
        tool,
        kind: kind.to_string(),
        deployment,
        memory: memory.as_ref().map(|(info, _)| info.clone()),
        memories,
        extras,
        absent_extras: Vec::new(),
    };
    let root = prepare_snapshots_dir(store)?;
    let staging = root.join(format!(".{id}.writing"));
    std::fs::create_dir(&staging)?;
    let written = (|| -> Result<()> {
        if let Some(prompt) = prompt {
            std::fs::write(staging.join("prompt.md"), prompt)?;
        }
        if let Some((info, bytes)) = &memory {
            std::fs::write(staging.join("memory.bin"), bytes)?;
            if sha_hex(&std::fs::read(staging.join("memory.bin"))?) != info.sha256 {
                return Err(Error::command_failed(
                    "the snapshot copy could not be verified",
                ));
            }
        }
        // A config file is saved only as a copy with every key and account taken out, so a
        // snapshot never holds a login. A file that cannot be cleaned that way stops the cleanup.
        for extra in meta
            .extras
            .iter()
            .filter(|e| e.kind == "saved-without-login")
        {
            if !valid_extra_name(&extra.name) {
                return Err(Error::invalid("a saved piece has a bad name"));
            }
            let target = staging.join("extras").join(&extra.name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(
                &target,
                sanitized_copy(Path::new(&extra.path), &extra.name)?,
            )?;
        }
        // Only list data already present in the snapshot. Renamed data is recorded after the move.
        let mut written_meta = meta.clone();
        written_meta.memories = None;
        written_meta
            .extras
            .retain(|extra| extra.kind == "saved-without-login");
        std::fs::write(
            staging.join("meta.json"),
            serde_json::to_vec_pretty(&written_meta)?,
        )?;
        std::fs::rename(&staging, root.join(&id))?;
        Ok(())
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(meta)
}

fn truncate_file(root: &Path, path: &Path) -> Result<()> {
    require_contained(root, path)?;
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(Error::invalid("the memory file is not an ordinary file"));
    }
    std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)?;
    Ok(())
}

/// Move the agent's memory folder into the snapshot that was made for it. Nothing is copied or
/// deleted: the folder is renamed, so it is either where it was or all inside the snapshot.
fn move_memories_into(
    store: &Store,
    snapshot: &SnapshotMeta,
    root: &Path,
    from: &Path,
) -> Result<()> {
    require_contained(root, from)?;
    let to = memories_in_snapshot(store, &snapshot.id)?;
    rename_to_vacant(from, &to).map_err(|error| {
        Error::command_failed(format!(
            "the memories folder could not be moved into the snapshot: {error}"
        ))
    })?;
    if let Err(error) = rewrite_meta(store, snapshot, |meta| {
        meta.memories = snapshot.memories.clone()
    }) {
        rename_to_vacant(&to, from)?;
        return Err(error);
    }
    Ok(())
}

/// Change what a written snapshot says it holds, starting from what is on disk.
fn rewrite_meta(
    store: &Store,
    snapshot: &SnapshotMeta,
    change: impl FnOnce(&mut SnapshotMeta),
) -> Result<()> {
    let mut meta = read_snapshot(store, &snapshot.id)?;
    change(&mut meta);
    let path = snapshot_path(store, &snapshot.id)?.join("meta.json");
    require_contained(&store.paths().home, &path)?;
    crate::paths::atomic_write(&path, &serde_json::to_string_pretty(&meta)?)
}

/// Rename a batch into vacant paths, reversing completed renames if a later rename fails.
fn rename_group(from_root: &Path, to_root: &Path, paths: &[(PathBuf, PathBuf)]) -> Result<()> {
    let rename = |from_root, from: &Path, to_root, to: &Path| {
        require_contained(from_root, from)?;
        require_contained(to_root, to)?;
        rename_to_vacant(from, to)
    };
    for (index, (from, to)) in paths.iter().enumerate() {
        let result = rename(from_root, from, to_root, to);
        if let Err(error) = result {
            let mut undo_error = None;
            for (from, to) in paths[..index].iter().rev() {
                if let Err(error) = rename(to_root, to, from_root, from) {
                    undo_error.get_or_insert(error);
                }
            }
            if let Some(undo_error) = undo_error {
                return Err(Error::command_failed(format!(
                    "{error}; could not reverse a SQLite group rename: {undo_error}"
                )));
            }
            return Err(error);
        }
    }
    Ok(())
}

fn rename_to_vacant(from: &Path, to: &Path) -> Result<()> {
    match std::fs::symlink_metadata(to) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => {
            return Err(Error::command_failed(format!(
                "{} is in the way",
                to.display()
            )))
        }
    }
    std::fs::rename(from, to)?;
    Ok(())
}

fn move_sqlite_group(
    store: &Store,
    snapshot: &SnapshotMeta,
    root: &Path,
    database: &str,
    group: &[&ExtraMeta],
) -> Result<Vec<ExtraMeta>> {
    let names = sqlite_entries(root, database, true)?;
    if names.len() != group.len() || !group.iter().all(|extra| names.contains(&extra.name)) {
        return Err(Error::command_failed(format!(
            "{database} changed while checking its sidecars"
        )));
    }
    let mut paths = Vec::new();
    let mut moved = Vec::new();
    for extra in group {
        let now = extra_info(root, &extra.name, &extra.kind, extra.category)?.ok_or_else(|| {
            Error::command_failed(format!("{database} changed while checking its sidecars"))
        })?;
        let from = root.join(&extra.name);
        require_contained(root, &from)?;
        let to = extra_in_snapshot(store, &snapshot.id, &extra.name)?;
        std::fs::create_dir_all(to.parent().expect("snapshot extra has a parent"))?;
        paths.push((from, to));
        moved.push(now);
    }
    let snapshot_home = &store.paths().home;
    rename_group(root, snapshot_home, &paths)?;
    if let Err(error) = rewrite_meta(store, snapshot, |meta| meta.extras.extend(moved.clone())) {
        let reverse: Vec<_> = paths
            .into_iter()
            .rev()
            .map(|(from, to)| (to, from))
            .collect();
        rename_group(snapshot_home, root, &reverse)?;
        return Err(error);
    }
    Ok(moved)
}

/// Move the saved pieces by rename and remove originals whose sanitized copies are in the
/// snapshot. Record each successful move so partial cleanup remains reversible.
fn move_extras_into(store: &Store, snapshot: &SnapshotMeta, root: &Path) -> Result<(u64, u64)> {
    let mut moved = Vec::new();
    let mut failure = None;
    let mut groups = sqlite_groups(&snapshot.extras, |extra| extra);
    for extra in &snapshot.extras {
        if let Some(database) = sqlite_database(&extra.name, &extra.kind) {
            if let Some(group) = groups.remove(database) {
                match move_sqlite_group(store, snapshot, root, database, &group) {
                    Ok(group) => moved.extend(group),
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            continue;
        }
        let from = root.join(&extra.name);
        let result = (|| -> Result<Option<ExtraMeta>> {
            // Uninstall may have changed a piece. Check its type and contents again before saving it.
            let Some(now) = extra_info(root, &extra.name, &extra.kind, extra.category)? else {
                return Ok(None);
            };
            let to = extra_in_snapshot(store, &snapshot.id, &extra.name)?;
            require_contained(root, &from)?;
            if extra.kind == "saved-without-login" {
                std::fs::symlink_metadata(&to)?;
                std::fs::remove_file(&from)?;
            } else {
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                rename_to_vacant(&from, &to)?;
                if let Err(error) =
                    rewrite_meta(store, snapshot, |meta| meta.extras.push(now.clone()))
                {
                    rename_to_vacant(&to, &from)?;
                    return Err(error);
                }
            }
            Ok(Some(now))
        })();
        match result {
            Ok(Some(now)) => moved.push(now),
            Ok(None) => {}
            Err(error) => {
                failure = Some(Error::command_failed(format!(
                    "{} could not be moved into the snapshot: {error}",
                    extra.name
                )));
                break;
            }
        }
    }
    let count = moved.len() as u64;
    let sessions = moved
        .iter()
        .filter(|extra| extra.category == CleanupCategory::Sessions)
        .count() as u64;
    match failure {
        Some(error) => Err(error),
        None => Ok((count, sessions)),
    }
}

/// Delete logins, plugins and caches. Everything is tried;
/// the first failure is reported at the end. A link is never followed or removed.
fn erase_extras(home: &Path, erased: &[ExtraMeta]) -> Result<u64> {
    let mut count = 0;
    let mut failure = None;
    for extra in erased {
        if !valid_extra_name(&extra.name) {
            continue;
        }
        let path = home.join(&extra.name);
        if !is_contained(home, &path)? {
            continue;
        }
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if is_link(&meta) || (!meta.is_dir() && !meta.is_file()) {
            continue;
        }
        let removed = if meta.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match removed {
            Ok(()) => count += 1,
            Err(error) => {
                failure.get_or_insert_with(|| {
                    Error::command_failed(format!("{} could not be removed: {error}", extra.name))
                });
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(count),
    }
}

/// The agent's login in the macOS keychain. Only the real home is ever asked: a test or a
/// preview run with a home of its own never reaches the person's keychain.
#[cfg(target_os = "macos")]
fn keychain_services(tool: ToolKind, opts: &AdapterOptions) -> Vec<&'static str> {
    if opts.home.is_some() {
        return Vec::new();
    }
    let Some(table) = table(tool) else {
        return Vec::new();
    };
    table
        .keychain
        .iter()
        .copied()
        .filter(|service| {
            std::process::Command::new("security")
                .args(["find-generic-password", "-s", service])
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn keychain_services(_tool: ToolKind, _opts: &AdapterOptions) -> Vec<&'static str> {
    Vec::new()
}

/// Stop the background jobs Switch left for the agent, so a moved or deleted file is not loaded
/// again. Best effort, and only for the real home. A job that is not loaded is not an error.
#[cfg(target_os = "macos")]
fn stop_launch_agents(tool: ToolKind, opts: &AdapterOptions) {
    let Some(table) = table(tool) else { return };
    if opts.home.is_some() {
        return;
    }
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    for label in table.launch_agents {
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &format!("gui/{uid}/{label}")])
            .output();
    }
}

#[cfg(not(target_os = "macos"))]
fn stop_launch_agents(_tool: ToolKind, _opts: &AdapterOptions) {}

/// Remove every keychain entry of the service (one per account). Returns whether any was removed.
fn erase_keychain(services: &[&str]) -> Result<bool> {
    let mut removed = false;
    for service in services {
        for _ in 0..16 {
            let done = std::process::Command::new("security")
                .args(["delete-generic-password", "-s", service])
                .output()
                .map_err(|error| {
                    Error::command_failed(format!("the keychain could not be reached: {error}"))
                })?;
            if !done.status.success() {
                break;
            }
            removed = true;
        }
    }
    Ok(removed)
}

/// Replace a file's bytes through a temporary file beside it, keeping its permissions.
fn restore_file(root: &Path, path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    require_contained(root, path)?;
    let name = path
        .file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = path.with_file_name(format!(
        ".{name}.keysmith-restore-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    if let Err(error) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }
    Ok(())
}

/// What is on the machine now: the deployment, its saved body, and the memory file with its bytes.
type CurrentState = (
    DeploymentMeta,
    Option<String>,
    Option<(MemoryMeta, Vec<u8>)>,
);

fn current_state(
    store: &Store,
    tool: ToolKind,
    envelope: &Envelope,
    opts: &AdapterOptions,
) -> Result<CurrentState> {
    let (deployment, body) = current_deployment(store, tool, envelope);
    let memory = match memory_file(tool, envelope, opts)? {
        Some(path) => Some(memory_info(&path)?),
        None => None,
    };
    Ok((deployment, body, memory))
}

fn current_deployment(
    store: &Store,
    tool: ToolKind,
    envelope: &Envelope,
) -> (DeploymentMeta, Option<String>) {
    let present = envelope.status == ToolStatus::Active;
    let body = if present {
        harness::proven_live_body(tool, envelope)
    } else {
        None
    };
    let title = if present {
        deployed_title(store, tool, envelope, body.as_deref())
    } else {
        None
    };
    (
        DeploymentMeta {
            present,
            title,
            restorable: body.is_some(),
        },
        body,
    )
}

/// A name for what is deployed: the library entry with the same text, if there is one.
fn deployed_title(
    store: &Store,
    tool: ToolKind,
    _envelope: &Envelope,
    body: Option<&str>,
) -> Option<String> {
    let sha = crate::db::markdown::content_sha(body?);
    store
        .list_prompts(tool, None, None, crate::models::PromptSort::Updated)
        .ok()?
        .into_iter()
        .find(|prompt| prompt.sha256 == sha)
        .map(|prompt| prompt.title)
}

fn preview_envelope(from: &Envelope, command: &str) -> Envelope {
    let mut envelope = from.clone();
    envelope.command = command.into();
    envelope.preview = true;
    envelope.ok = true;
    envelope.blockers.clear();
    envelope
}

/// Whether Switch's ZCode adapter still has the ZCode app itself patched. Its only copy of the
/// original is in `~/.zcode-keysmith/backups`, so nothing may be erased while this is true.
fn zcode_app_patched(tool: ToolKind, opts: &AdapterOptions, envelope: &Envelope) -> Result<bool> {
    if tool != ToolKind::Zcode {
        return Ok(false);
    }
    if envelope.doctor.checks.iter().any(|check| {
        check.name == "zcode_runtime_patched" && check.detail.as_deref() == Some("true")
    }) {
        return Ok(true);
    }
    let home = home_dir(opts)?;
    let managed = home.join(".zcode-keysmith");
    let path = managed.join("config.json");
    if !is_contained(&home, &path).unwrap_or(false) {
        return Ok(true);
    }
    let config = match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => match std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        {
            Some(config) if config.is_object() => config,
            _ => return Ok(true),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::Value::Null,
        _ => return Ok(true),
    };
    if config
        .get("app_bundle_modified")
        .is_some_and(|value| value.as_bool() != Some(false))
    {
        return Ok(true);
    }

    const MARKER: &[u8] = b"ZCODE_KEYSMITH_SYSTEM_FILE";
    let mut runtimes = envelope
        .target_paths
        .iter()
        .filter(|target| target.role == "zcode_runtime")
        .map(|target| target.path.as_str())
        .chain(config["zcode_runtime"].as_str())
        .collect::<Vec<_>>();
    runtimes.sort_unstable();
    runtimes.dedup();
    let mut verified = !runtimes.is_empty();
    for runtime in runtimes {
        let path = Path::new(runtime);
        if path.is_absolute() && path.is_file() {
            if let Ok(bytes) = std::fs::read(path) {
                if bytes.windows(MARKER.len()).any(|window| window == MARKER) {
                    return Ok(true);
                }
                continue;
            }
        }
        verified = false;
    }
    if verified {
        return Ok(false);
    }

    let backups = managed.join("backups");
    if !is_contained(&home, &backups).unwrap_or(false) {
        return Ok(true);
    }
    Ok(match std::fs::read_dir(backups) {
        Ok(mut entries) => entries.next().is_some(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    })
}

const ZCODE_PATCHED: &str =
    "the ZCode app is still patched by Switch and its original would be lost; repair it first";

pub async fn plan_cleanup(
    store: &Store,
    tool: ToolKind,
    opts: &AdapterOptions,
) -> Result<CleanupPlan> {
    if let Some(reason) = tool.unavailable_reason() {
        return Err(Error::unavailable(reason.to_string()));
    }
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    let (deployment, _body, memory) = current_state(store, tool, &envelope, opts)?;
    // Drift does not stop a cleanup: the memory file and the rest of the setup are still
    // cleaned, and the drifted deployment is left for a repair. A half-done transaction does.
    let config_drifted = envelope.status == ToolStatus::Drift;
    let mut blockers = Vec::new();
    if envelope.recovery_required {
        blockers.push(
            "the agent's setup was changed after the last deploy; repair it first".to_string(),
        );
    }
    // Only the adapter's own uninstall puts the original app back, and it only runs for an
    // active deployment.
    if !deployment.present && zcode_app_patched(tool, opts, &envelope)? {
        blockers.push(ZCODE_PATCHED.to_string());
    }
    let memory_meta = memory.as_ref().map(|(info, _)| info.clone());
    let memories = current_memories(tool, opts)?;
    let entries = extra_entries(store, tool, &envelope, CleanupSelection::all(), opts)?;
    let extras = current_extras(tool, &entries, false, opts)?;
    let login_in_keychain = !keychain_services(tool, opts).is_empty();
    let nothing_to_do = !deployment.present
        && memory_meta.as_ref().is_none_or(|m| m.bytes == 0)
        && memories.is_none()
        && extras.is_empty()
        && !login_in_keychain;
    let operation = ops::store_preview(
        store,
        tool,
        OperationKind::Cleanup,
        None,
        Scope::User,
        None,
        json!({
            "tool": tool,
            "deployed": deployment.present,
            "memorySha": memory_meta.as_ref().map(|m| m.sha256.clone()),
            "memories": memories.as_ref().map(|m| json!({ "files": m.files, "bytes": m.bytes })),
            "extras": extras,
            "keychain": login_in_keychain,
        }),
        &preview_envelope(&envelope, "plan-cleanup"),
    )?;
    Ok(CleanupPlan {
        operation_id: operation.id,
        tool,
        deployment,
        memory: memory_meta,
        memories,
        extras,
        login_in_keychain,
        agent_running: agent_running(tool, opts),
        config_drifted,
        nothing_to_do,
        blockers,
    })
}

fn fail_plan(store: &Store, operation_id: &str, error: &Error) {
    let _ = store.update_operation(
        operation_id,
        OperationStatus::Failed,
        None,
        Some(&error.to_string()),
    );
}

pub async fn confirm_cleanup(
    store: &Store,
    operation_id: &str,
    selection: CleanupSelection,
    opts: &AdapterOptions,
) -> Result<CleanupResult> {
    let plan = ops::claim_preview(store, operation_id, OperationKind::Cleanup)?;
    let result = async {
        let request = serde_json::from_str(&plan.request_json)?;
        run_cleanup(store, plan.tool, &request, selection, opts).await
    }
    .await;
    match &result {
        Ok(_) => {
            store.update_operation(operation_id, OperationStatus::Succeeded, None, None)?;
        }
        Err(error) => fail_plan(store, operation_id, error),
    }
    result
}

async fn run_cleanup(
    store: &Store,
    tool: ToolKind,
    request: &serde_json::Value,
    selection: CleanupSelection,
    opts: &AdapterOptions,
) -> Result<CleanupResult> {
    if !selection.any_selected() {
        return Err(Error::invalid("nothing selected to clean"));
    }
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.recovery_required {
        return Err(Error::command_failed(
            "the agent's setup was changed after the last deploy; repair it first",
        ));
    }
    let (deployment, body) = current_deployment(store, tool, &envelope);
    if (selection.deployment || selection.login_and_cache)
        && (!deployment.present || !selection.deployment)
        && zcode_app_patched(tool, opts, &envelope)?
    {
        return Err(Error::command_failed(ZCODE_PATCHED));
    }
    if selection.deployment
        && request.get("deployed").and_then(|value| value.as_bool()) != Some(deployment.present)
    {
        return Err(Error::user_cancel(
            "plan already used: things changed since the preview",
        ));
    }

    let memory = if selection.memory {
        let memory = memory_file(tool, &envelope, opts)?
            .as_deref()
            .map(memory_info)
            .transpose()?;
        let planned_sha = request.get("memorySha").and_then(|value| value.as_str());
        let now_sha = memory.as_ref().map(|(info, _)| info.sha256.as_str());
        if planned_sha != now_sha {
            return Err(Error::user_cancel(
                "plan already used: things changed since the preview",
            ));
        }
        memory
    } else {
        None
    };

    // The folder is checked before anything is changed: it is the step that can least be undone
    // by hand, so it must be the one that cannot be refused halfway.
    let memories = if selection.memories {
        let now = current_memories(tool, opts)?;
        let planned = request.get("memories").filter(|value| !value.is_null());
        let planned_same = match (&now, planned) {
            (None, None) => true,
            (Some(info), Some(value)) => {
                value.get("files").and_then(|v| v.as_u64()) == Some(info.files)
                    && value.get("bytes").and_then(|v| v.as_u64()) == Some(info.bytes)
            }
            _ => false,
        };
        if !planned_same {
            return Err(Error::user_cancel(
                "plan already used: things changed since the preview",
            ));
        }
        if let Some(info) = &now {
            if !same_volume(Path::new(&info.path), &prepare_snapshots_dir(store)?) {
                return Err(Error::command_failed(
                    "the memories folder is on another disk than the snapshots, so it cannot be moved safely",
                ));
            }
        }
        now
    } else {
        None
    };

    // Regenerate the selected entries so a newly created prefix match also invalidates the preview.
    let entries = extra_entries(store, tool, &envelope, selection, opts)?;
    let extras = current_extras(tool, &entries, false, opts)?;
    let mut planned_extras: Vec<ExtraMeta> = serde_json::from_value(
        request
            .get("extras")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|_| Error::user_cancel("plan already used: things changed since the preview"))?;
    planned_extras.retain(|extra| category_selected(&selection, extra.category));
    if planned_extras != extras {
        return Err(Error::user_cancel(
            "plan already used: things changed since the preview",
        ));
    }
    check_extras_volume(store, &extras)?;
    let to_keep = kept(&extras);
    let to_erase: Vec<ExtraMeta> = extras
        .iter()
        .filter(|e| e.kind == "erased")
        .cloned()
        .collect();
    let keychain = if selection.login_and_cache {
        keychain_services(tool, opts)
    } else {
        Vec::new()
    };
    if selection.login_and_cache
        && request.get("keychain").and_then(|value| value.as_bool()) != Some(!keychain.is_empty())
    {
        return Err(Error::user_cancel(
            "plan already used: things changed since the preview",
        ));
    }
    let touches_running_data =
        selection.memories || selection.sessions || selection.login_and_cache;
    if touches_running_data && agent_running(tool, opts) {
        return Err(Error::command_failed(
            "close the agent first: its sessions, memories or login are in use",
        ));
    }

    let selected_deployment = selection.deployment && deployment.present;
    let selected_memory =
        selection.memory && memory.as_ref().is_some_and(|(info, _)| info.bytes > 0);
    let needs_snapshot = selected_deployment
        || selected_memory
        || selection.memories && memories.is_some()
        || !to_keep.is_empty();

    // Hold the home lock across preflight validation, optional input-rewrite disconnect and
    // snapshot creation so a bad config, symlink escape, lock failure or freshly started agent
    // stops cleanly before input rewrite is disconnected.
    let (rewrite_disconnected, snapshot) = {
        let _lock = HomeLock::acquire(store.paths())?;
        if touches_running_data && agent_running(tool, opts) {
            return Err(Error::command_failed(
                "close the agent first: its sessions, memories or login are in use",
            ));
        }
        if let Some(home) = agent_home(tool, opts)? {
            for extra in to_keep
                .iter()
                .filter(|extra| extra.kind == "saved-without-login")
            {
                let path = Path::new(&extra.path);
                require_contained(&home, path)?;
                let _ = sanitized_copy(path, &extra.name)?;
            }
        }
        let rewrite_disconnected = if selection.login_and_cache {
            crate::rewrite::disconnect_tool(store.paths(), tool)?
        } else {
            false
        };
        let snapshot = if needs_snapshot {
            Some(create_snapshot(
                store,
                tool,
                "cleanup",
                if selection.deployment {
                    deployment.clone()
                } else {
                    DeploymentMeta {
                        present: false,
                        title: None,
                        restorable: false,
                    }
                },
                if selection.deployment {
                    body.as_deref()
                } else {
                    None
                },
                memory
                    .as_ref()
                    .filter(|(info, _)| info.bytes > 0)
                    .map(|(info, bytes)| (info.clone(), bytes.as_slice())),
                memories.clone(),
                to_keep.clone(),
            )?)
        } else {
            None
        };
        (rewrite_disconnected, snapshot)
    };
    let discard = |why: Error| -> Error {
        if let Some(snapshot) = &snapshot {
            let _ = delete_snapshot(store, &snapshot.id);
        }
        why
    };

    let mut deactivated = false;
    if selection.deployment && deployment.present {
        let plan = ops::plan_deactivate(
            store,
            PlanDeactivateInput {
                prompt_id: None,
                tool,
                scope: Scope::User,
                project_dir: None,
                salvage_config: false,
            },
            opts,
        )
        .await
        .map_err(&discard)?;
        if !plan.envelope.ok || !plan.envelope.blockers.is_empty() {
            return Err(discard(Error::command_failed(
                plan.envelope
                    .error
                    .clone()
                    .or_else(|| plan.envelope.blockers.first().cloned())
                    .unwrap_or_else(|| "the removal was refused".into()),
            )));
        }
        let done = ops::confirm_deactivate(store, &plan.operation_id, opts).await?;
        if !done.envelope.ok {
            // Something may have changed: the snapshot stays so it can be rolled back.
            return Err(Error::command_failed(
                done.envelope
                    .error
                    .clone()
                    .unwrap_or_else(|| "the removal did not finish".into()),
            ));
        }
        deactivated = true;
        if tool == ToolKind::Zcode {
            // Uninstall can move the config aside even when it skips restoring the runtime.
            let after = ops::tool_status(store, tool, Scope::User, None, opts).await?;
            if zcode_app_patched(tool, opts, &after)? {
                return Err(Error::command_failed(ZCODE_PATCHED));
            }
        }
    }

    let lock = HomeLock::acquire(store.paths())?;
    if touches_running_data && agent_running(tool, opts) {
        let error = Error::command_failed(
            "close the agent first: its sessions, memories or login are in use",
        );
        if !deactivated {
            return Err(discard(error));
        }
        return Err(error);
    }

    let memory_cleared = if let Some((info, _)) = memory.filter(|(info, _)| info.bytes > 0) {
        truncate_file(&home_dir(opts)?, Path::new(&info.path))?;
        true
    } else {
        false
    };

    // Last, and the folder is put back empty so the agent finds what it expects.
    let mut memories_cleared = false;
    if let (Some(info), Some(snapshot)) = (&memories, &snapshot) {
        let from = Path::new(&info.path);
        move_memories_into(store, snapshot, &home_dir(opts)?, from)?;
        std::fs::create_dir(from)?;
        memories_cleared = true;
    }

    let (mut extras_cleared, mut sessions_cleared) = (0, 0);
    let mut erased = 0;
    if let Some(home) = agent_home(tool, opts)? {
        if selection.deployment {
            stop_launch_agents(tool, opts);
        }
        if let (false, Some(snapshot)) = (to_keep.is_empty(), &snapshot) {
            (extras_cleared, sessions_cleared) = move_extras_into(store, snapshot, &home)?;
        }
        // Logins and caches go last, after everything saved is safe.
        if selection.login_and_cache {
            erased = erase_extras(&home, &to_erase)?;
        }
    }
    let keychain_cleared = if selection.login_and_cache {
        erase_keychain(&keychain)?
    } else {
        false
    };
    drop(lock);
    // A drifted Grok deployment is not removed through its adapter, but the cleanup has just
    // moved its manifest and rule away: nothing of it is left, so Switch's record says so too.
    // The cleanup itself is done by now; a failed read here only leaves the record as it was.
    if selection.deployment && tool == ToolKind::Grok && !deactivated {
        if let Err(error) = ops::forget_removed_deployment(store, tool, opts).await {
            let _ = crate::logging::write_line("cleanup-record", &error.to_string());
        }
    }
    Ok(CleanupResult {
        snapshot_id: snapshot.map(|meta| meta.id),
        deactivated,
        memory_cleared,
        memories_cleared,
        extras_cleared,
        sessions_cleared,
        erased,
        keychain_cleared,
        rewrite_disconnected,
    })
}

pub async fn plan_rollback(
    store: &Store,
    snapshot_id: &str,
    opts: &AdapterOptions,
) -> Result<RollbackPlan> {
    let snapshot = read_snapshot(store, snapshot_id)?;
    snapshot_memory(store, &snapshot)?;
    snapshot_memories(store, &snapshot)?;
    snapshot_extras(store, &snapshot)?;
    let tool = snapshot.tool;
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    let (deployment, _body, memory) = current_state(store, tool, &envelope, opts)?;
    let current_extras = current_rollback_pieces(tool, &snapshot, opts)?;
    let missing_extras = missing_rollback_extras(agent_home(tool, opts)?.as_deref(), &snapshot)?;
    let mut blockers = Vec::new();
    if envelope.status == ToolStatus::Drift || envelope.recovery_required {
        blockers.push(
            "the agent's setup was changed after the last deploy; repair it first".to_string(),
        );
    }
    let memory_plan = match (&snapshot.memory, &memory) {
        (Some(saved), now) => Some(RollbackMemory {
            restore_bytes: saved.bytes,
            current_bytes: now.as_ref().map_or(0, |(info, _)| info.bytes),
            current_differs: now
                .as_ref()
                .map_or(saved.bytes > 0, |(info, _)| info.sha256 != saved.sha256),
        }),
        (None, _) => None,
    };
    let overwrites_memory = memory_plan
        .as_ref()
        .is_some_and(|m| m.current_differs && m.current_bytes > 0);
    let current_memories = match &snapshot.memories {
        Some(_) => current_memories(tool, opts)?,
        None => None,
    };
    let memories_plan = snapshot.memories.as_ref().map(|saved| RollbackMemories {
        restore_files: saved.files,
        restore_bytes: saved.bytes,
        current_files: current_memories.as_ref().map_or(0, |m| m.files),
        current_bytes: current_memories.as_ref().map_or(0, |m| m.bytes),
    });
    let operation = ops::store_preview(
        store,
        tool,
        OperationKind::Rollback,
        None,
        Scope::User,
        None,
        json!({ "tool": tool, "snapshotId": snapshot.id }),
        &preview_envelope(&envelope, "plan-rollback"),
    )?;
    Ok(RollbackPlan {
        operation_id: operation.id,
        current_title: deployment.title.clone(),
        replaces_deployment: deployment.present && snapshot.deployment.restorable,
        saves_current: deployment.present
            || overwrites_memory
            || current_memories.is_some()
            || !current_extras.is_empty()
            || absences_need_snapshot(&snapshot, &missing_extras),
        memory: memory_plan,
        memories: memories_plan,
        extras: snapshot.extras.len() as u64,
        agent_running: rollback_needs_idle_agent(&snapshot) && agent_running(tool, opts),
        snapshot,
        blockers,
    })
}

pub async fn confirm_rollback(
    store: &Store,
    operation_id: &str,
    opts: &AdapterOptions,
) -> Result<RollbackResult> {
    let plan = ops::claim_preview(store, operation_id, OperationKind::Rollback)?;
    let result = async {
        let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
        let id = request
            .get("snapshotId")
            .and_then(|value| value.as_str())
            .ok_or_else(|| Error::user_cancel("plan not found"))?;
        run_rollback(store, id, opts).await
    }
    .await;
    match &result {
        Ok(_) => {
            store.update_operation(operation_id, OperationStatus::Succeeded, None, None)?;
        }
        Err(error) => fail_plan(store, operation_id, error),
    }
    result
}

async fn run_rollback(store: &Store, id: &str, opts: &AdapterOptions) -> Result<RollbackResult> {
    let snapshot = read_snapshot(store, id)?;
    let tool = snapshot.tool;
    let saved_prompt = snapshot_prompt(store, &snapshot)?;
    let saved_memory = snapshot_memory(store, &snapshot)?;
    let saved_memories = snapshot_memories(store, &snapshot)?;
    let saved_extras = snapshot_extras(store, &snapshot)?;
    let snapshot_root = snapshot_path(store, &snapshot.id)?;
    if rollback_needs_idle_agent(&snapshot) && agent_running(tool, opts) {
        return Err(Error::command_failed(
            "close the agent first: it is using session or memories data",
        ));
    }

    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.status == ToolStatus::Drift || envelope.recovery_required {
        return Err(Error::command_failed(
            "the agent's setup was changed after the last deploy; repair it first",
        ));
    }
    let (deployment, body, memory) = current_state(store, tool, &envelope, opts)?;

    let lock = HomeLock::acquire(store.paths())?;
    if rollback_needs_idle_agent(&snapshot) && agent_running(tool, opts) {
        return Err(Error::command_failed(
            "close the agent first: it is using session or memories data",
        ));
    }
    // What is there now is kept, so this rollback can be undone as well.
    let current_differs = match (&snapshot.memory, &memory) {
        (Some(saved), Some((info, _))) => info.sha256 != saved.sha256 && info.bytes > 0,
        _ => false,
    };
    let current_folder = match &saved_memories {
        Some(_) => current_memories(tool, opts)?,
        None => None,
    };
    if let Some(info) = &current_folder {
        if !same_volume(Path::new(&info.path), &prepare_snapshots_dir(store)?) {
            return Err(Error::command_failed(
                "the memories folder is on another disk than the snapshots, so it cannot be moved safely",
            ));
        }
    }
    let current_pieces = current_rollback_pieces(tool, &snapshot, opts)?;
    check_extras_volume(store, &current_pieces)?;
    let home = agent_home(tool, opts)?;
    let missing_extras = missing_rollback_extras(home.as_deref(), &snapshot)?;
    let mut staged_groups = std::collections::BTreeMap::new();
    if let Some(root) = &home {
        for (saved, _) in &saved_extras {
            require_contained(root, &root.join(&saved.name))?;
        }
        for (database, group) in sqlite_groups(&saved_extras, |(extra, _)| extra) {
            require_contained(root, &root.join(database))?;
            staged_groups.insert(
                database,
                stage_sqlite_group(root, &store.paths().home, &group)?,
            );
        }
    }
    let saved_snapshot = if deployment.present
        || current_differs
        || current_folder.is_some()
        || !current_pieces.is_empty()
        || absences_need_snapshot(&snapshot, &missing_extras)
    {
        Some(create_snapshot(
            store,
            tool,
            "before-rollback",
            deployment.clone(),
            body.as_deref(),
            memory
                .as_ref()
                .map(|(info, bytes)| (info.clone(), bytes.as_slice())),
            current_folder.clone(),
            current_pieces.clone(),
        )?)
    } else {
        None
    };
    let saved_snapshot_id = saved_snapshot.as_ref().map(|meta| meta.id.clone());

    // The config files and instruction folders come back first: the adapter's deploy needs them.
    let mut extras_restored = 0;
    let mut sessions_restored = 0;
    if !saved_extras.is_empty() || !current_pieces.is_empty() {
        let root = home
            .ok_or_else(|| Error::invalid("this agent has no setup folder; nothing was changed"))?;
        // Whatever is there now goes into the snapshot made above first.
        if let Some(snapshot) = &saved_snapshot {
            let (moved, _) = move_extras_into(store, snapshot, &root)?;
            if moved != current_pieces.len() as u64 {
                return Err(Error::command_failed(
                    "the current files changed while rolling back; nothing was restored",
                ));
            }
        }
        for (saved, source) in &saved_extras {
            if let Some(database) = sqlite_database(&saved.name, &saved.kind) {
                if let Some(staged) = staged_groups.remove(database) {
                    if let Err(error) = rename_group(&root, &root, &staged.paths) {
                        if let Some(before) = &saved_snapshot {
                            put_back_sqlite_group(store, before, &root, database)?;
                        }
                        return Err(error);
                    }
                    if let Some(before) = &saved_snapshot {
                        record_restored_absences(store, before, &missing_extras, |extra| {
                            sqlite_database(&extra.name, "saved") == Some(database)
                        })?;
                    }
                    extras_restored += staged.paths.len() as u64;
                    sessions_restored += staged.sessions;
                }
                continue;
            }
            require_contained(&root, &root.join(&saved.name))?;
            // A config file saved without its login is put back only where there is none now:
            // one the agent wrote since (a new login) is never replaced by the stripped copy.
            if saved.kind == "saved-without-login"
                && std::fs::symlink_metadata(root.join(&saved.name)).is_ok()
            {
                continue;
            }
            let restored = u64::from(restore_extra(&root, &store.paths().home, saved, source)?);
            if let Some(before) = &saved_snapshot {
                record_restored_absences(store, before, &missing_extras, |extra| {
                    extra.name == saved.name
                })?;
            }
            extras_restored += restored;
            if saved.category == CleanupCategory::Sessions {
                sessions_restored += restored;
            }
        }
    }
    drop(lock);
    // The deploy is planned now that its config is back.
    let planned = match &saved_prompt {
        Some(text) => {
            let title = format!(
                "{} · {}",
                snapshot.deployment.title.as_deref().unwrap_or("Restored"),
                &snapshot.created_at[..10.min(snapshot.created_at.len())]
            );
            let prompt_id = harness::store_restored_prompt(store, tool, &title, text)?;
            let plan = ops::plan_activate(
                store,
                PlanActivateInput {
                    prompt_id,
                    scope: Scope::User,
                    project_dir: None,
                    runtime: false,
                    append_file: None,
                    max_tokens: None,
                },
                opts,
            )
            .await?;
            if !plan.envelope.ok || !plan.envelope.blockers.is_empty() {
                return Err(Error::command_failed(
                    plan.envelope
                        .error
                        .clone()
                        .or_else(|| plan.envelope.blockers.first().cloned())
                        .unwrap_or_else(|| "the deploy was refused".into()),
                ));
            }
            Some(plan)
        }
        None => None,
    };

    let mut redeployed = false;
    if let Some(plan) = planned {
        let done = ops::confirm_activate(store, &plan.operation_id, opts).await?;
        if !done.envelope.ok {
            return Err(Error::command_failed(
                done.envelope
                    .error
                    .clone()
                    .unwrap_or_else(|| "the deploy did not finish".into()),
            ));
        }
        redeployed = true;
    }

    let mut memory_restored = false;
    if let (Some(saved), Some(bytes)) = (&snapshot.memory, &saved_memory) {
        // The file is found again through the adapter, and held to the same rule as when it was cleared.
        let after = ops::tool_status(store, tool, Scope::User, None, opts).await?;
        let path = memory_file(tool, &after, opts)?
            .or_else(|| {
                let expected = memory_expected(tool, opts).ok()??;
                (expected.to_string_lossy() == saved.path).then_some(expected)
            })
            .ok_or_else(|| Error::invalid("the memory file could not be found"))?;
        let _lock = HomeLock::acquire(store.paths())?;
        restore_file(&home_dir(opts)?, &path, bytes, saved.mode)?;
        memory_restored = true;
    }

    let mut memories_restored = false;
    if let (Some(saved), Some(source)) = (&snapshot.memories, &saved_memories) {
        let _lock = HomeLock::acquire(store.paths())?;
        if agent_running(tool, opts) {
            return Err(Error::command_failed(
                "close the agent first: it is using session or memories data",
            ));
        }
        require_contained(&store.paths().home, &snapshot_root)?;
        require_contained(&snapshot_root, source)?;
        let target = PathBuf::from(&saved.path);
        let home = home_dir(opts)?;
        let expected = home.join(".codex").join("memories");
        if tool != ToolKind::Codex || target != expected {
            return Err(Error::invalid(
                "the memories folder is not the one in the user's home; nothing was changed",
            ));
        }
        require_contained(&home, &expected)?;
        let parent = expected
            .parent()
            .ok_or_else(|| Error::invalid("the memories folder has no parent"))?;
        std::fs::create_dir_all(parent)?;
        // The copy is made and checked beside the target first; only then is the current
        // folder moved out and the copy renamed in.
        let staging = parent.join(format!(
            ".memories.keysmith-restore-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let copied = copy_dir(source, &staging).and_then(|()| {
            let info = memories_info(&staging)?;
            if info.files != saved.files || info.bytes != saved.bytes {
                return Err(Error::command_failed(
                    "the copy of the memories folder could not be verified",
                ));
            }
            Ok(())
        });
        if let Err(error) = copied {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        // Whatever the folder holds now goes into the snapshot made above.
        let moved_away = match (&current_memories(tool, opts)?, &saved_snapshot) {
            (Some(_), Some(snapshot)) => {
                if let Err(error) = move_memories_into(store, snapshot, &home, &expected) {
                    let _ = std::fs::remove_dir_all(&staging);
                    return Err(error);
                }
                true
            }
            (Some(_), None) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(Error::command_failed(
                    "the memories folder changed while rolling back; nothing was replaced",
                ));
            }
            (None, _) => {
                // Only empty folders are in the way: they hold nothing to keep.
                if expected.exists() {
                    std::fs::remove_dir_all(&expected)?;
                }
                false
            }
        };
        if let Err(error) = std::fs::rename(&staging, &expected) {
            let _ = std::fs::remove_dir_all(&staging);
            if moved_away {
                if let Some(snapshot) = &saved_snapshot {
                    require_contained(&home, &expected)?;
                    rename_to_vacant(&memories_in_snapshot(store, &snapshot.id)?, &expected)?;
                    rewrite_meta(store, snapshot, |meta| meta.memories = None)?;
                }
            }
            return Err(error.into());
        }
        memories_restored = true;
    }

    Ok(RollbackResult {
        saved_snapshot_id,
        redeployed,
        memory_restored,
        memories_restored,
        extras_restored,
        sessions_restored,
    })
}

/// Put one saved piece of setup back: copy and check it beside its place, then rename it in. Only
/// an empty folder may be in the way; anything else there is left alone and the piece is refused.
fn restore_extra(
    root: &Path,
    snapshot_home: &Path,
    saved: &ExtraMeta,
    source: &Path,
) -> Result<bool> {
    let target = root.join(&saved.name);
    require_contained(root, &target)?;
    require_contained(snapshot_home, source)?;
    if let Ok(meta) = std::fs::symlink_metadata(&target) {
        let empty_dir = meta.is_dir() && std::fs::read_dir(&target)?.next().is_none();
        if !empty_dir {
            return Err(Error::command_failed(format!(
                "{} is in the way and was left as it is; nothing more was restored",
                saved.name
            )));
        }
        std::fs::remove_dir(&target)?;
    }
    let parent = target
        .parent()
        .ok_or_else(|| Error::invalid("a saved piece has no parent folder"))?;
    std::fs::create_dir_all(parent)?;
    let last = target
        .file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .unwrap_or_else(|| "extra".into());
    let staging = parent.join(format!(
        ".{last}.keysmith-restore-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let copied = (|| -> Result<()> {
        if std::fs::symlink_metadata(source)?.is_dir() {
            copy_dir(source, &staging)?;
        } else {
            std::fs::copy(source, &staging)?;
        }
        // A config copy made without its login is checked against the copy in the snapshot, since
        // it is not the size of the original.
        let expected = if saved.kind == "saved-without-login" {
            path_stats(source)?
        } else {
            (saved.files, saved.bytes)
        };
        if path_stats(&staging)? != expected {
            return Err(Error::command_failed(format!(
                "the copy of {} could not be verified",
                saved.name
            )));
        }
        std::fs::rename(&staging, &target)?;
        Ok(())
    })();
    if let Err(error) = copied {
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_file(&staging);
        return Err(error);
    }
    Ok(true)
}

struct StagedSqlite {
    _directory: tempfile::TempDir,
    paths: Vec<(PathBuf, PathBuf)>,
    sessions: u64,
}

fn stage_sqlite_group(
    root: &Path,
    snapshot_home: &Path,
    group: &[&(ExtraMeta, PathBuf)],
) -> Result<StagedSqlite> {
    let target = root.join(&group[0].0.name);
    require_contained(root, &target)?;
    let parent = target
        .parent()
        .ok_or_else(|| Error::invalid("a saved database has no parent folder"))?;
    std::fs::create_dir_all(parent)?;
    let directory = tempfile::Builder::new()
        .prefix(".keysmith-sqlite-")
        .tempdir_in(parent)?;
    let mut paths = Vec::new();
    let mut sessions = 0;
    for (saved, source) in group {
        let target = root.join(&saved.name);
        require_contained(root, &target)?;
        require_contained(snapshot_home, source)?;
        let staging = directory.path().join(paths.len().to_string());
        let meta = std::fs::symlink_metadata(source)?;
        if !meta.is_file()
            || is_link(&meta)
            || std::fs::copy(source, &staging)? != saved.bytes
            || path_stats(&staging)? != (saved.files, saved.bytes)
        {
            return Err(Error::command_failed(format!(
                "the copy of {} could not be verified",
                saved.name
            )));
        }
        paths.push((staging, target));
        sessions += u64::from(saved.category == Sessions);
    }
    Ok(StagedSqlite {
        _directory: directory,
        paths,
        sessions,
    })
}

fn put_back_sqlite_group(
    store: &Store,
    before: &SnapshotMeta,
    root: &Path,
    database: &str,
) -> Result<()> {
    let mut paths = Vec::new();
    for extra in &before.extras {
        if sqlite_database(&extra.name, &extra.kind) == Some(database) {
            let target = root.join(&extra.name);
            require_contained(root, &target)?;
            paths.push((extra_in_snapshot(store, &before.id, &extra.name)?, target));
        }
    }
    if !paths.is_empty() {
        rename_group(&store.paths().home, root, &paths)?;
        rewrite_meta(store, before, |meta| {
            meta.extras
                .retain(|extra| sqlite_database(&extra.name, &extra.kind) != Some(database));
        })?;
    }
    Ok(())
}

fn record_restored_absences(
    store: &Store,
    before: &SnapshotMeta,
    missing: &[AbsentExtra],
    restored: impl Fn(&AbsentExtra) -> bool,
) -> Result<()> {
    let restored: Vec<_> = missing
        .iter()
        .filter(|extra| restored(extra))
        .cloned()
        .collect();
    if !restored.is_empty() {
        rewrite_meta(store, before, |meta| meta.absent_extras.extend(restored))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::envelope::TargetPath;

    fn envelope_pointing_at(path: &Path) -> Envelope {
        let mut envelope = Envelope::new(ToolKind::Claude, "status");
        envelope.target_paths.push(TargetPath {
            path: path.to_string_lossy().into_owned(),
            role: "memory".into(),
            exists: true,
        });
        envelope
    }

    #[test]
    fn legacy_categories_infer_exact_and_prefix_matches_before_falling_back() {
        assert_eq!(
            legacy_extra_category(ToolKind::Claude, ".claude.json", "saved-without-login"),
            LoginAndCache
        );
        assert_eq!(
            legacy_extra_category(ToolKind::Grok, ".grok/worktrees.db-journal", "saved"),
            Sessions
        );
        assert_eq!(
            legacy_extra_category(ToolKind::Grok, ".grok/custom", "erased"),
            LoginAndCache
        );
        assert_eq!(
            legacy_extra_category(ToolKind::Grok, ".grok/custom", "saved"),
            Setup
        );
    }

    #[test]
    fn cleanup_category_and_selection_json_are_stable() {
        assert_eq!(
            serde_json::to_value(CleanupCategory::LoginAndCache).unwrap(),
            "login-and-cache"
        );
        let selection = CleanupSelection::default_safe();
        assert!(selection.deployment);
        assert!(!selection.memory && !selection.memories && !selection.setup);
        assert!(!selection.sessions && !selection.login_and_cache);
        assert!(selection.any_selected());
        assert!(CleanupSelection::all().any_selected());
        assert!(!serde_json::from_value::<CleanupSelection>(json!({}))
            .unwrap()
            .any_selected());
    }

    #[test]
    fn sqlite_group_renames_revert_if_the_wal_destination_is_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let home = root.join("home");
        let store = Store::open(&crate::paths::AppPaths::from_home(root.join("switch"))).unwrap();
        std::fs::create_dir_all(home.join(".grok")).unwrap();
        let entries = [".grok/worktrees.db", ".grok/worktrees.db-wal"];
        let extras = entries
            .iter()
            .map(|name| {
                std::fs::write(home.join(name), name).unwrap();
                extra_info(&home, name, "saved", Sessions).unwrap().unwrap()
            })
            .collect();
        let snapshot = create_snapshot(
            &store,
            ToolKind::Grok,
            "cleanup",
            DeploymentMeta {
                present: false,
                title: None,
                restorable: false,
            },
            None,
            None,
            None,
            extras,
        )
        .unwrap();
        let [db, wal] = entries.map(|name| extra_in_snapshot(&store, &snapshot.id, name).unwrap());
        std::fs::create_dir_all(&wal).unwrap();

        assert!(move_extras_into(&store, &snapshot, &home).is_err());
        assert!(!db.exists());
        assert!(read_snapshot(&store, &snapshot.id)
            .unwrap()
            .extras
            .is_empty());
        for name in entries {
            assert_eq!(std::fs::read_to_string(home.join(name)).unwrap(), name);
        }
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn process_listing_failures_count_as_running() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        use std::process::{ExitStatus, Output};

        let tool = ToolKind::Codex;
        assert!(running_in_process_listing(
            tool,
            Err(std::io::ErrorKind::NotFound.into())
        ));
        let output = |status, stdout: &[u8]| {
            Ok(Output {
                status: ExitStatus::from_raw(status),
                stdout: stdout.to_vec(),
                stderr: Vec::new(),
            })
        };
        assert!(running_in_process_listing(tool, output(256, b"")));
        assert!(!running_in_process_listing(
            tool,
            output(0, b"another-process")
        ));
        assert!(running_in_process_listing(tool, output(0, b"codex.exe")));
    }

    #[test]
    fn extras_include_their_cleanup_category_in_the_preview() {
        let extra = ExtraMeta {
            name: ".claude/projects".into(),
            path: "/home/test/.claude/projects".into(),
            files: 1,
            bytes: 4,
            kind: "saved".into(),
            category: CleanupCategory::Sessions,
        };
        let value = serde_json::to_value(&extra).unwrap();
        assert_eq!(value["category"], "sessions");
        assert_eq!(
            serde_json::from_value::<ExtraMeta>(json!({
                "name": ".claude/rules",
                "path": "/home/test/.claude/rules",
                "files": 1,
                "bytes": 4,
                "kind": "saved"
            }))
            .unwrap()
            .category,
            CleanupCategory::Setup
        );
    }

    #[test]
    fn only_the_user_level_claude_md_in_the_users_own_home_is_accepted() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        let user_level = home.join(".claude/CLAUDE.md");
        std::fs::write(&user_level, "mine\n").unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("CLAUDE.md"), "project\n").unwrap();
        let opts = AdapterOptions {
            home: Some(home.clone()),
            ..AdapterOptions::default()
        };

        // The adapter reports the user-level file: accepted.
        let accepted =
            memory_file(ToolKind::Claude, &envelope_pointing_at(&user_level), &opts).unwrap();
        assert_eq!(
            accepted.map(|p| std::fs::canonicalize(p).unwrap()),
            Some(std::fs::canonicalize(&user_level).unwrap())
        );

        // A project's CLAUDE.md, or any other file, is refused whatever the adapter says.
        for other in [
            project.join("CLAUDE.md"),
            home.join(".claude/settings.json"),
            tmp.path().join("elsewhere.md"),
        ] {
            std::fs::write(&other, "x\n").unwrap();
            assert!(
                memory_file(ToolKind::Claude, &envelope_pointing_at(&other), &opts).is_err(),
                "{other:?}"
            );
        }

        // Other agents have no memory file Keysmith touches.
        assert!(
            memory_file(ToolKind::Codex, &envelope_pointing_at(&user_level), &opts)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    #[cfg(unix)]
    fn containment_allows_a_link_above_the_root_but_refuses_links_below_it() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(real.join("home")).unwrap();
        let alias = tmp.path().join("alias");
        symlink(&real, &alias).unwrap();
        let home = alias.join("home");
        let path = home.join("new/subdir/file");
        assert!(is_contained(&home, &path).unwrap());
        symlink(&real, home.join("new")).unwrap();
        assert!(!is_contained(&home, &path).unwrap());
    }

    #[test]
    fn extras_skip_links_projects_and_logins() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("rules")).unwrap();
        std::fs::write(root.join("rules/a.md"), "rule\n").unwrap();
        std::fs::create_dir_all(root.join("agents/repo/.git")).unwrap();
        std::fs::create_dir_all(root.join("commands")).unwrap();
        std::fs::write(root.join("commands/auth.json.bak"), "{}").unwrap();

        let rules = extra_info(root, "rules", "saved", CleanupCategory::Setup)
            .unwrap()
            .unwrap();
        assert_eq!((rules.files, rules.bytes), (1, 5));
        assert!(extra_info(root, "agents", "saved", CleanupCategory::Setup)
            .unwrap()
            .is_none());
        assert!(
            extra_info(root, "commands", "saved", CleanupCategory::Setup)
                .unwrap()
                .is_none()
        );
        assert!(extra_info(root, "missing", "saved", CleanupCategory::Setup)
            .unwrap()
            .is_none());
        for bad in ["", "../rules", "/etc"] {
            assert!(!valid_extra_name(bad), "{bad:?}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("rules"), root.join("linked")).unwrap();
            assert!(extra_info(root, "linked", "saved", CleanupCategory::Setup)
                .unwrap()
                .is_none());
            assert!(
                extra_info(root, "linked", "erased", CleanupCategory::LoginAndCache)
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn config_copies_keep_settings_and_lose_every_login() {
        let tmp = tempfile::tempdir().unwrap();
        let json = tmp.path().join("settings.json");
        std::fs::write(
            &json,
            r#"{"model":"opus","hooks":{"Stop":[]},"env":{"ANTHROPIC_AUTH_TOKEN":"sk-ant-abcdef123456","API_TIMEOUT_MS":"5"},"oauthAccount":{"emailAddress":"a@b.c"}}"#,
        )
        .unwrap();
        let out = String::from_utf8(sanitized_copy(&json, "settings.json").unwrap()).unwrap();
        assert!(out.contains("opus") && out.contains("hooks") && out.contains("API_TIMEOUT_MS"));
        assert!(
            !out.contains("sk-ant") && !out.contains("a@b.c") && !out.contains("oauth"),
            "{out}"
        );

        let sizes = tmp.path().join("limits.json");
        std::fs::write(
            &sizes,
            r#"{"max_tokens":4096,"maxOutputTokens":8000,"apiKeyRequired":false}"#,
        )
        .unwrap();
        let out = String::from_utf8(sanitized_copy(&sizes, "limits.json").unwrap()).unwrap();
        assert!(
            out.contains("max_tokens")
                && out.contains("maxOutputTokens")
                && !out.contains("apiKey"),
            "{out}"
        );

        let toml = tmp.path().join("config.toml");
        std::fs::write(&toml, "model = \"gpt\"\nmodel_max_output_tokens = 8000\nexperimental_bearer_token = \"abc123\"\n[mcp_servers.x]\ncommand = \"npx\"\n").unwrap();
        let out = String::from_utf8(sanitized_copy(&toml, "config.toml").unwrap()).unwrap();
        assert!(out.contains("model_max_output_tokens = 8000"), "{out}");
        assert!(
            out.contains("model") && out.contains("mcp_servers") && !out.contains("abc123"),
            "{out}"
        );

        std::fs::write(
            &toml,
            "[providers.sample]\nmodel = 'fixture'\noptions = { 'api_key' = 'inline-fixture', retries = 2 }\n'password' = '''\nmultiline-fixture\n'''\n[oauth]\nvalue = 'oauth-fixture'\n",
        )
        .unwrap();
        let out = String::from_utf8(sanitized_copy(&toml, "config.toml").unwrap()).unwrap();
        let parsed: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(
            parsed["providers"]["sample"]["options"]["retries"].as_integer(),
            Some(2)
        );
        for secret in ["inline-fixture", "multiline-fixture", "oauth-fixture"] {
            assert!(!out.contains(secret));
        }
    }

    #[test]
    fn ids_are_plain_and_short() {
        assert!(valid_id("20260101T000000Z-claude-abc123"));
        for bad in ["", "..", "../x", "a/b", "a b", "a\\b", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }
}
