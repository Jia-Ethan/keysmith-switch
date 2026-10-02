//! Clean an agent back to nothing, and roll that back.
//!
//! Cleaning an agent means: take away what Keysmith deployed (the system prompt and the
//! managed block in the agent's config, through the adapter's own uninstall) and, for
//! Claude Code and Codex, empty the user-level memory file (`~/.claude/CLAUDE.md`,
//! `~/.codex/AGENTS.md`), including what the person wrote in it. That is one of two places
//! Keysmith writes an agent's files itself instead of asking an adapter, so it is held to
//! a narrow rule: user scope only, exactly that one file, never a link, and never before a
//! complete copy has been saved and checked.
//!
//! The other is Codex's own memory folder `~/.codex/memories`. It is large and Codex
//! writes to it, so it is only touched when the person asks for it by name, only while
//! Codex is not running, and it is moved (not copied or deleted) into the snapshot. Rolling
//! back copies it out again and leaves the snapshot whole.
//!
//! Before anything is changed a snapshot is written to `~/.keysmith-switch/snapshots/`:
//! the text of the live prompt (when it can be proven to be the live text) and a byte
//! copy of the memory file. Rolling back redeploys that text through the normal adapter
//! plan and puts the memory file's bytes back. Rolling back first saves the current state
//! as a snapshot of its own, so a rollback can be undone too.
//!
//! Leftover user-level instruction files (the `rules`, `agents` and, for Claude Code, `commands`
//! folders, and a wrapper the adapter reports inside the agent folder) are moved whole into the
//! snapshot's `extras` folder and copied back on rollback. A piece that is a link, or holds a git
//! project or a login file, is left where it is.
//!
//! Project files, project and local scope deployments, logins and credentials, any other agent
//! folder (Codex's database, sessions and so on) and the prompt library are never touched.

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
/// Tests and unusual setups say whether Codex is running instead of asking the system: "1" or "0".
pub const CODEX_RUNNING_KEY: &str = "KEYSMITH_SWITCH_CODEX_RUNNING";

/// What a fresh install does not have, named from the user's home folder. Three kinds:
/// - saved: the person's own setup (rules, agents, skills, hooks, Switch's leftovers). Moved whole
///   into the snapshot and copied back on rollback.
/// - saved without the login: config files that also carry keys. The snapshot gets a copy with
///   every key, token and account removed, so no login is ever kept in a snapshot.
/// - erased: logins, session history, plugins and caches. Deleted, not saved.
///
/// A saved folder that is a link, or holds a git project anywhere inside, is left where it is.
/// Project folders, the prompt library and the agent program itself are never named here.
struct Table {
    saved: &'static [&'static str],
    without_login: &'static [&'static str],
    erased: &'static [&'static str],
    /// Files in one folder whose names start with the prefix: (folder, prefix). Old copies of the
    /// config files carry the same keys, so they are erased; the person's own instruction
    /// backups are saved.
    saved_prefixes: &'static [(&'static str, &'static str)],
    erased_prefixes: &'static [(&'static str, &'static str)],
    /// The agent's folder: a wrapper the adapter reports inside it is saved too.
    folder: &'static str,
    /// Where the agent keeps its login besides files (the macOS keychain).
    keychain: &'static [&'static str],
    /// Background jobs Switch left for this agent (macOS LaunchAgent labels): stopped first.
    launch_agents: &'static [&'static str],
}

const CLAUDE: Table = Table {
    saved: &[
        ".claude/rules",
        ".claude/agents",
        ".claude/commands",
        ".claude/hooks",
        ".claude/skills",
        ".claude/output-styles",
        ".claude/keysmith",
    ],
    without_login: &[
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude.json",
    ],
    erased: &[
        ".claude/plugins",
        ".claude/projects",
        ".claude/sessions",
        ".claude/history.jsonl",
        ".claude/file-history",
        ".claude/session-env",
        ".claude/shell-snapshots",
        ".claude/paste-cache",
        ".claude/plans",
        ".claude/todos",
        ".claude/debug",
        ".claude/downloads",
        ".claude/cache",
        ".claude/backups",
        ".claude/teams",
        ".claude/ide",
        ".claude/statsig",
        ".claude/telemetry",
        ".claude/config.json",
        ".claude/policy-limits.json",
        ".claude/.last-cleanup",
        ".claude/.last-update-result.json",
    ],
    saved_prefixes: &[(".claude", "CLAUDE.md.")],
    erased_prefixes: &[
        (".claude", "settings.json."),
        (".claude", "settings.local.json."),
        ("", ".claude.json."),
    ],
    folder: ".claude",
    keychain: &["Claude Code-credentials"],
    launch_agents: &[],
};

const CODEX: Table = Table {
    saved: &[
        ".codex/rules",
        ".codex/agents",
        ".codex/skills",
        ".codex/prompts",
        ".codex/hooks.json",
        ".codex/.codex-keysmith-manifest.json",
    ],
    without_login: &[".codex/config.toml"],
    erased: &[
        ".codex/auth.json",
        ".codex/plugins",
        ".codex/sessions",
        ".codex/archived_sessions",
        ".codex/session_index.jsonl",
        ".codex/history.jsonl",
        ".codex/shell_snapshots",
        ".codex/log",
        ".codex/logs",
        ".codex/cache",
        ".codex/tmp",
        ".codex/backups",
        ".codex/runtime-backups",
        ".codex/memory-backups",
        ".codex/models_cache.json",
        ".codex/installation_id",
        ".codex/.codex-global-state.json",
        ".codex/transcription-history.jsonl",
        ".codex/dictation-history",
        ".codex/generated_images",
        ".codex/attachments",
        ".codex/vendor_imports",
        ".codex/state_5.sqlite",
        ".codex/logs_2.sqlite",
        ".codex/memories_1.sqlite",
        ".codex/goals_1.sqlite",
        ".codex/queue_1.sqlite",
        ".codex/thread_history_1.sqlite",
    ],
    saved_prefixes: &[(".codex", "AGENTS.md.")],
    erased_prefixes: &[
        (".codex", "auth.json."),
        (".codex", "config.toml."),
        (".codex", "state_5.sqlite-"),
        (".codex", "logs_2.sqlite-"),
        (".codex", "memories_1.sqlite-"),
        (".codex", "goals_1.sqlite-"),
        (".codex", "queue_1.sqlite-"),
        (".codex", "thread_history_1.sqlite-"),
    ],
    folder: ".codex",
    keychain: &["Codex Auth"],
    launch_agents: &[],
};

/// Grok keeps its program in `bin`, `bundled`, `vendor` and `downloads`: those are never named.
/// `Grok Bot` is a separate app and is not touched.
const GROK: Table = Table {
    saved: &[
        ".grok/rules",
        ".grok/agents",
        ".grok/skills",
        ".grok/AGENTS.md",
        ".grok/memory",
        ".grok/memory-v2",
        ".grok/.grok-keysmith-manifest.json",
    ],
    without_login: &[".grok/config.toml"],
    erased: &[
        ".grok/auth.json",
        ".grok/auth.json.lock",
        ".grok/sessions",
        ".grok/memtrace",
        ".grok/logs",
        ".grok/installed-plugins",
        ".grok/marketplace-cache",
        ".grok/models_cache.json",
        ".grok/settings_cache.json",
        ".grok/slash-mru.json",
        ".grok/tip_cursor.json",
        ".grok/upload_queue",
        ".grok/active_sessions.json",
        ".grok/active_sessions.lock",
        ".grok/campaigns_state.json",
        ".grok/campaigns_state.json.lock",
        ".grok/worktrees.db",
    ],
    saved_prefixes: &[(".grok", "AGENTS.md.")],
    erased_prefixes: &[(".grok", "auth.json."), (".grok", "config.toml.")],
    folder: ".grok",
    keychain: &[],
    launch_agents: &[],
};

/// ZCode's `~/.zcode/workspace` is where its projects live (git projects inside): never named.
const ZCODE: Table = Table {
    saved: &[
        ".zcode/AGENTS.md",
        ".zcode/commands",
        ".zcode/skills",
        ".zcode/cli/memories",
        // Switch's own folder. Its `backups` and `cache` hold copies of the app's runtime
        // (large, and not the person's setup), so they are erased below, not saved.
        ".zcode-keysmith/system-role.md",
        ".zcode-keysmith/config.json",
        ".zcode-keysmith/runtime-shim.json",
        ".zcode-keysmith/bin",
    ],
    without_login: &[
        // `~/.zcode/cli/config.json` is a link to this one, and a link is never touched.
        ".zcode/v2/config.json",
        ".zcode/v2/setting.json",
        ".zcode/v2/provider_config.json",
        ".zcode/v2/bot-config.json",
        ".zcode/v2/bot-config.v3.json",
    ],
    erased: &[
        ".zcode/v2/credentials.json",
        ".zcode/v2/onboarding-record.json",
        ".zcode/v2/bot-state.v2.json",
        ".zcode/v2/bot-state.v3.json",
        ".zcode/v2/bots-model-cache.v2.json",
        ".zcode/v2/coding-plan-cache.json",
        ".zcode/v2/telemetry-state.json",
        ".zcode/v2/tasks-index.sqlite",
        ".zcode/v2/cache",
        ".zcode/v2/logs",
        ".zcode/v2/crash",
        ".zcode/cli/db",
        ".zcode/cli/rollout",
        ".zcode/cli/exec",
        ".zcode/cli/artifacts",
        ".zcode/cli/agents",
        ".zcode/cli/log",
        ".zcode/cli/image-cache",
        ".zcode/cli/pdf-cache",
        ".zcode/cli/plugins",
        ".zcode/backups",
        ".zcode/tmp",
        ".zcode-keysmith/backups",
        ".zcode-keysmith/cache",
        ".zcode-keysmith/logs",
        "Library/Application Support/ZCode/session",
        "Library/Application Support/ZCode/rum-electron-store",
        "Library/Application Support/ZCode/zcode-data-size-telemetry.json",
        "Library/Caches/dev.zcode.app",
    ],
    saved_prefixes: &[("Library/LaunchAgents", "com.jia.zcode-keysmith.")],
    erased_prefixes: &[
        (".zcode/v2", "tasks-index.sqlite"),
        (".zcode/v2", "config.json."),
        (".zcode/v2", "credentials.json."),
        (".zcode/cli", "config.json."),
    ],
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
    /// The agent is running now, so its memory folder cannot be cleared or restored.
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
    /// Logins, session history and caches that were deleted (they are not in the snapshot).
    pub erased: u64,
    pub keychain_cleared: bool,
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
    Ok(snapshots_dir(store).join(id))
}

/// Every complete snapshot, newest first. A folder without its `meta.json` is a write that
/// never finished and is not listed.
pub fn list_snapshots(store: &Store) -> Vec<SnapshotMeta> {
    let mut found: Vec<SnapshotMeta> = std::fs::read_dir(snapshots_dir(store))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| std::fs::read(entry.path().join("meta.json")).ok())
        .filter_map(|bytes| serde_json::from_slice::<SnapshotMeta>(&bytes).ok())
        .filter(|meta| valid_id(&meta.id))
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
    let path = snapshot_path(store, id)?;
    let bytes =
        std::fs::read(path.join("meta.json")).map_err(|_| Error::invalid("snapshot not found"))?;
    serde_json::from_slice(&bytes).map_err(|_| Error::invalid("snapshot is damaged"))
}

fn snapshot_prompt(store: &Store, meta: &SnapshotMeta) -> Result<Option<String>> {
    if !meta.deployment.restorable {
        return Ok(None);
    }
    let bytes = std::fs::read(snapshot_path(store, &meta.id)?.join("prompt.md"))
        .map_err(|_| Error::invalid("snapshot is damaged: its prompt is missing"))?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| Error::invalid("snapshot is damaged: its prompt is unreadable"))
}

fn snapshot_memory(store: &Store, meta: &SnapshotMeta) -> Result<Option<Vec<u8>>> {
    let Some(memory) = &meta.memory else {
        return Ok(None);
    };
    let bytes = std::fs::read(snapshot_path(store, &meta.id)?.join("memory.bin"))
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
    let path = PathBuf::from(&target.path);
    let meta =
        std::fs::symlink_metadata(&path).map_err(|_| Error::invalid("the memory file is gone"))?;
    if !meta.is_file() {
        return Err(Error::invalid("the memory file is not an ordinary file"));
    }
    let same = match (
        std::fs::canonicalize(&path),
        std::fs::canonicalize(&expected),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    };
    if !same {
        return Err(Error::invalid(
            "the memory file is not the user-level CLAUDE.md; nothing was changed",
        ));
    }
    Ok(Some(path))
}

/// Codex's own memory folder, when it has one. A link, or anything that is not a folder, is
/// refused: the folder is moved as a whole, and only the real one in the user's home.
fn memories_dir(tool: ToolKind, opts: &AdapterOptions) -> Result<Option<PathBuf>> {
    if tool != ToolKind::Codex {
        return Ok(None);
    }
    let path = home_dir(opts)?.join(".codex").join("memories");
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
            if meta.is_dir() {
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

/// Whether the agent is running, which keeps its sessions, login and memory folder in use. A
/// process list that cannot be read counts as not running; the person is also told to quit it
/// first. A test or preview run with a home of its own never asks the system.
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
    let listing = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output();
    #[cfg(not(any(unix, windows)))]
    return false;
    #[cfg(any(unix, windows))]
    listing.ok().is_some_and(|out| {
        String::from_utf8_lossy(&out.stdout).lines().any(|line| {
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
            }
        })
    })
}

fn memories_in_snapshot(store: &Store, id: &str) -> Result<PathBuf> {
    Ok(snapshot_path(store, id)?.join("memories"))
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

fn is_secret_name(name: &std::ffi::OsStr) -> bool {
    SECRET_NAMES.iter().any(|secret| name == *secret)
}

/// One piece of setup: its place under the home folder and what is done to it.
type Entry = (String, String);

/// Files in `folder` (under home) whose names start with `prefix`, as places under home.
fn prefixed(home: &Path, folder: &str, prefix: &str) -> Vec<String> {
    let dir = if folder.is_empty() {
        home.to_path_buf()
    } else {
        home.join(folder)
    };
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
    found
}

/// Which pieces to look at, in a fixed order: the agent's tables, the old copies of its config
/// files, and the wrapper the adapter reports when it lies inside the agent's folder.
fn extra_entries(tool: ToolKind, envelope: &Envelope, opts: &AdapterOptions) -> Result<Vec<Entry>> {
    let (Some(table), Some(home)) = (table(tool), agent_home(tool, opts)?) else {
        return Ok(Vec::new());
    };
    let mut entries: Vec<Entry> = Vec::new();
    let mut add = |name: String, kind: &str| {
        if !entries.iter().any(|(known, _)| *known == name) {
            entries.push((name, kind.to_string()));
        }
    };
    for name in table.saved {
        add(name.to_string(), "saved");
    }
    for name in table.without_login {
        add(name.to_string(), "saved-without-login");
    }
    for (folder, prefix) in table.saved_prefixes {
        for name in prefixed(&home, folder, prefix) {
            add(name, "saved");
        }
    }
    for name in table.erased {
        add(name.to_string(), "erased");
    }
    for (folder, prefix) in table.erased_prefixes {
        for name in prefixed(&home, folder, prefix) {
            add(name, "erased");
        }
    }
    let agent_folder = home.join(table.folder);
    for target in envelope.target_paths.iter().filter(|t| t.role == "wrapper") {
        if let Ok(relative) = Path::new(&target.path).strip_prefix(&home) {
            if Path::new(&target.path).starts_with(&agent_folder) {
                let name = relative.to_string_lossy().replace('\\', "/");
                if valid_extra_name(&name) {
                    add(name, "saved");
                }
            }
        }
    }
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
            if std::fs::symlink_metadata(entry.path())?.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(false)
}

/// How many files and bytes a file or folder holds.
fn path_stats(path: &Path) -> Result<(u64, u64)> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        dir_stats(path)
    } else {
        Ok((1, meta.len()))
    }
}

/// One piece of setup as it is now, or `None` when it is missing, empty or a link. A saved folder
/// that holds a git project or a login file is left alone as well, and so is a saved file that
/// is itself a login file.
fn extra_info(home: &Path, name: &str, kind: &str) -> Result<Option<ExtraMeta>> {
    if !valid_extra_name(name) {
        return Ok(None);
    }
    let path = home.join(name);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if meta.file_type().is_symlink() || (!meta.is_dir() && !meta.is_file()) {
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
    Ok((files > 0).then(|| ExtraMeta {
        name: name.to_string(),
        path: path.to_string_lossy().into_owned(),
        files,
        bytes,
        kind: kind.to_string(),
    }))
}

fn current_extras(
    tool: ToolKind,
    entries: &[Entry],
    opts: &AdapterOptions,
) -> Result<Vec<ExtraMeta>> {
    let Some(home) = agent_home(tool, opts)? else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    for (name, kind) in entries {
        if let Some(info) = extra_info(&home, name, kind)? {
            found.push(info);
        }
    }
    Ok(found)
}

fn extras_json(extras: &[ExtraMeta]) -> serde_json::Value {
    json!(extras
        .iter()
        .map(|e| json!({ "name": e.name, "files": e.files, "bytes": e.bytes, "kind": e.kind }))
        .collect::<Vec<_>>())
}

fn entries_of(extras: &[ExtraMeta]) -> Vec<Entry> {
    extras
        .iter()
        .map(|e| (e.name.clone(), e.kind.clone()))
        .collect()
}

/// What a rollback would move aside: the saved folders and files that are there now. A config file
/// that is there is left in place, since it may hold a login made since the cleanup.
fn current_rollback_pieces(
    tool: ToolKind,
    snapshot: &SnapshotMeta,
    opts: &AdapterOptions,
) -> Result<Vec<ExtraMeta>> {
    let entries: Vec<Entry> = entries_of(&snapshot.extras)
        .into_iter()
        .filter(|(_, kind)| kind == "saved")
        .collect();
    current_extras(tool, &entries, opts)
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
    Ok(snapshot_path(store, id)?.join("extras").join(name))
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
    Ok(found)
}

/// The saved pieces must be on the snapshots' volume, so moving them is a rename that cannot
/// lose anything. A config copy is written, not moved, and needs no such check.
fn check_extras_volume(store: &Store, extras: &[ExtraMeta]) -> Result<()> {
    let moved: Vec<&ExtraMeta> = extras.iter().filter(|e| e.kind == "saved").collect();
    if moved.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(snapshots_dir(store))?;
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

/// A TOML line carries a login when its key names one or its value looks like one. A number or
/// a switch is never a login (`model_max_output_tokens = 8000`).
fn toml_line_holds_login(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some((key, value)) = line.split_once('=').filter(|_| !trimmed.starts_with('[')) else {
        return crate::redact::redact_text(line) != line;
    };
    let key = key.trim().trim_matches('"');
    if secret_key(key) {
        return true;
    }
    let value = value.trim();
    if value.parse::<f64>().is_ok() || value == "true" || value == "false" {
        return false;
    }
    crate::redact::redact_text(line) != line
}

fn strip_toml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines().filter(|line| !toml_line_holds_login(line)) {
        out.push_str(line);
        out.push('\n');
    }
    out
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
        strip_toml(&text)
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
    let still_holds_login = if name.ends_with(".toml") {
        cleaned.lines().any(toml_line_holds_login)
    } else {
        crate::redact::redact_text(&cleaned) != cleaned
    };
    if still_holds_login {
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
        if meta.is_dir() {
            copy_dir(&source, &target)?;
        } else if meta.file_type().is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(std::fs::read_link(&source)?, &target)?;
        } else {
            std::fs::copy(&source, &target)?;
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

#[cfg(not(unix))]
fn same_volume(_left: &Path, _right: &Path) -> bool {
    true
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
    let _lock = HomeLock::acquire(store.paths())?;
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
    };
    let root = snapshots_dir(store);
    let staging = root.join(format!(".{id}.writing"));
    std::fs::create_dir_all(&staging)?;
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
        std::fs::write(staging.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
        std::fs::rename(&staging, root.join(&id))?;
        Ok(())
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(meta)
}

fn truncate_file(path: &Path) -> Result<()> {
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
/// deleted: the folder is renamed, so it is either where it was or all inside the snapshot. When
/// it cannot be moved the snapshot is rewritten to say it holds no folder.
fn move_memories_into(store: &Store, snapshot: &SnapshotMeta, from: &Path) -> Result<()> {
    let to = memories_in_snapshot(store, &snapshot.id)?;
    if let Err(error) = std::fs::rename(from, &to) {
        rewrite_meta(store, snapshot, |meta| meta.memories = None);
        return Err(Error::command_failed(format!(
            "the memories folder could not be moved into the snapshot: {error}"
        )));
    }
    Ok(())
}

/// Change what a written snapshot says it holds, starting from what is on disk.
fn rewrite_meta(store: &Store, snapshot: &SnapshotMeta, change: impl FnOnce(&mut SnapshotMeta)) {
    let mut meta = read_snapshot(store, &snapshot.id).unwrap_or_else(|_| snapshot.clone());
    change(&mut meta);
    if let (Ok(path), Ok(bytes)) = (
        snapshot_path(store, &snapshot.id),
        serde_json::to_vec_pretty(&meta),
    ) {
        let _ = std::fs::write(path.join("meta.json"), bytes);
    }
}

/// Move each piece of setup into the snapshot made for it, by rename only. A piece that is gone
/// by now (the adapter's uninstall may take its wrapper away) is dropped from the snapshot, and
/// so is every piece after one that cannot be moved. Returns how many were moved.
fn move_extras_into(store: &Store, snapshot: &SnapshotMeta, root: &Path) -> Result<u64> {
    let mut moved = Vec::new();
    let mut failure = None;
    let mut changed = false;
    for extra in &snapshot.extras {
        let from = root.join(&extra.name);
        if std::fs::symlink_metadata(&from).is_err() {
            continue;
        }
        // What the piece holds now: the adapter's uninstall may have taken its own file out of
        // it (Grok's deployed rule lives in `rules`), and that file is saved as the prompt.
        let mut extra = extra.clone();
        if extra.kind == "saved" {
            let (files, bytes) = path_stats(&from)?;
            if files == 0 {
                changed = true;
                continue;
            }
            if (files, bytes) != (extra.files, extra.bytes) {
                changed = true;
                extra.files = files;
                extra.bytes = bytes;
            }
        }
        let to = extra_in_snapshot(store, &snapshot.id, &extra.name)?;
        let result = if extra.kind == "saved-without-login" {
            // The copy without its login is already in the snapshot; only now is the original
            // taken away, and only when that copy is really there.
            std::fs::symlink_metadata(&to).and_then(|_| std::fs::remove_file(&from))
        } else {
            to.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::rename(&from, &to))
        };
        if let Err(error) = result {
            failure = Some(Error::command_failed(format!(
                "{} could not be moved into the snapshot: {error}",
                extra.name
            )));
            break;
        }
        moved.push(extra);
    }
    let count = moved.len() as u64;
    if changed || moved.len() != snapshot.extras.len() {
        rewrite_meta(store, snapshot, |meta| meta.extras = moved);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(count),
    }
}

/// Delete what is not saved: logins, session history, plugins and caches. Everything is tried;
/// the first failure is reported at the end. A link is never followed or removed.
fn erase_extras(home: &Path, erased: &[ExtraMeta]) -> Result<u64> {
    let mut count = 0;
    let mut failure = None;
    for extra in erased {
        if !valid_extra_name(&extra.name) {
            continue;
        }
        let path = home.join(&extra.name);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
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
fn restore_file(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
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
    let memory = match memory_file(tool, envelope, opts)? {
        Some(path) => Some(memory_info(&path)?),
        None => None,
    };
    Ok((
        DeploymentMeta {
            present,
            title,
            restorable: body.is_some(),
        },
        body,
        memory,
    ))
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
fn zcode_app_patched(tool: ToolKind, opts: &AdapterOptions) -> Result<bool> {
    if tool != ToolKind::Zcode {
        return Ok(false);
    }
    let path = home_dir(opts)?.join(".zcode-keysmith").join("config.json");
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(false);
    };
    Ok(serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| value.get("app_bundle_modified").and_then(|v| v.as_bool()))
        .unwrap_or(false))
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
    if !deployment.present && zcode_app_patched(tool, opts)? {
        blockers.push(ZCODE_PATCHED.to_string());
    }
    let memory_meta = memory.as_ref().map(|(info, _)| info.clone());
    let memories = current_memories(tool, opts)?;
    let entries = extra_entries(tool, &envelope, opts)?;
    let extras = current_extras(tool, &entries, opts)?;
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
            "extraNames": entries,
            "extras": extras_json(&extras),
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

/// `clear_memories` is the person's own tick for the agent's memory folder: without it that
/// folder is left exactly as it is, however much is in it.
pub async fn confirm_cleanup(
    store: &Store,
    operation_id: &str,
    clear_memories: bool,
    opts: &AdapterOptions,
) -> Result<CleanupResult> {
    let plan = ops::require_preview(store, operation_id, OperationKind::Cleanup)?;
    let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
    let result = run_cleanup(store, plan.tool, &request, clear_memories, opts).await;
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
    clear_memories: bool,
    opts: &AdapterOptions,
) -> Result<CleanupResult> {
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.recovery_required {
        return Err(Error::command_failed(
            "the agent's setup was changed after the last deploy; repair it first",
        ));
    }
    let (deployment, body, memory) = current_state(store, tool, &envelope, opts)?;
    // What was previewed is what is there now, or nothing happens.
    let planned_sha = request.get("memorySha").and_then(|value| value.as_str());
    let now_sha = memory.as_ref().map(|(info, _)| info.sha256.as_str());
    if planned_sha != now_sha
        || request.get("deployed").and_then(|value| value.as_bool()) != Some(deployment.present)
    {
        return Err(Error::user_cancel(
            "plan already used: things changed since the preview",
        ));
    }

    // The folder is checked before anything is changed: it is the step that can least be undone
    // by hand, so it must be the one that cannot be refused halfway.
    let memories = if clear_memories {
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
            if agent_running(tool, opts) {
                return Err(Error::command_failed(
                    "close the agent first: it is using its memories folder",
                ));
            }
            std::fs::create_dir_all(snapshots_dir(store))?;
            if !same_volume(Path::new(&info.path), &snapshots_dir(store)) {
                return Err(Error::command_failed(
                    "the memories folder is on another disk than the snapshots, so it cannot be moved safely",
                ));
            }
        }
        now
    } else {
        None
    };

    // The pieces of setup are looked for under the same names as in the preview.
    let entries: Vec<Entry> = request
        .get("extraNames")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let extras = current_extras(tool, &entries, opts)?;
    if request.get("extras").cloned().unwrap_or_else(|| json!([])) != extras_json(&extras) {
        return Err(Error::user_cancel(
            "plan already used: things changed since the preview",
        ));
    }
    check_extras_volume(store, &extras)?;
    if !deployment.present && zcode_app_patched(tool, opts)? {
        return Err(Error::command_failed(ZCODE_PATCHED));
    }
    let to_keep = kept(&extras);
    let to_erase: Vec<ExtraMeta> = extras
        .iter()
        .filter(|e| e.kind == "erased")
        .cloned()
        .collect();
    let keychain = keychain_services(tool, opts);
    if (!to_erase.is_empty() || !keychain.is_empty()) && agent_running(tool, opts) {
        return Err(Error::command_failed(
            "close the agent first: its sessions and login are in use",
        ));
    }

    let snapshot = if deployment.present
        || memory.as_ref().is_some_and(|(info, _)| info.bytes > 0)
        || memories.is_some()
        || !to_keep.is_empty()
    {
        Some(create_snapshot(
            store,
            tool,
            "cleanup",
            deployment.clone(),
            body.as_deref(),
            memory
                .as_ref()
                .map(|(info, bytes)| (info.clone(), bytes.as_slice())),
            memories.clone(),
            to_keep.clone(),
        )?)
    } else {
        None
    };
    let discard = |why: Error| -> Error {
        if let Some(snapshot) = &snapshot {
            let _ = delete_snapshot(store, &snapshot.id);
        }
        why
    };

    let mut deactivated = false;
    if deployment.present {
        let plan = ops::plan_deactivate(
            store,
            PlanDeactivateInput {
                prompt_id: None,
                tool,
                scope: Scope::User,
                project_dir: None,
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
        // The uninstall must have put the original ZCode app back before its copy is erased.
        // The snapshot stays: the deployment is already gone and can be rolled back.
        if zcode_app_patched(tool, opts)? {
            return Err(Error::command_failed(ZCODE_PATCHED));
        }
    }

    let mut memory_cleared = false;
    if let Some((info, _)) = &memory {
        if info.bytes > 0 {
            let _lock = HomeLock::acquire(store.paths())?;
            truncate_file(Path::new(&info.path))?;
            memory_cleared = true;
        }
    }

    // Last, and the folder is put back empty so the agent finds what it expects.
    let mut memories_cleared = false;
    if let (Some(info), Some(snapshot)) = (&memories, &snapshot) {
        let _lock = HomeLock::acquire(store.paths())?;
        let from = Path::new(&info.path);
        move_memories_into(store, snapshot, from)?;
        std::fs::create_dir(from)?;
        memories_cleared = true;
    }

    let mut extras_cleared = 0;
    let mut erased = 0;
    if let Some(home) = agent_home(tool, opts)? {
        let _lock = HomeLock::acquire(store.paths())?;
        stop_launch_agents(tool, opts);
        if let (false, Some(snapshot)) = (to_keep.is_empty(), &snapshot) {
            extras_cleared = move_extras_into(store, snapshot, &home)?;
        }
        // Logins, session history and caches go last, after everything saved is safe.
        erased = erase_extras(&home, &to_erase)?;
    }
    let keychain_cleared = erase_keychain(&keychain)?;
    Ok(CleanupResult {
        snapshot_id: snapshot.map(|meta| meta.id),
        deactivated,
        memory_cleared,
        memories_cleared,
        extras_cleared,
        erased,
        keychain_cleared,
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
            || !current_extras.is_empty(),
        memory: memory_plan,
        memories: memories_plan,
        extras: snapshot.extras.len() as u64,
        agent_running: snapshot.memories.is_some() && agent_running(tool, opts),
        snapshot,
        blockers,
    })
}

pub async fn confirm_rollback(
    store: &Store,
    operation_id: &str,
    opts: &AdapterOptions,
) -> Result<RollbackResult> {
    let plan = ops::require_preview(store, operation_id, OperationKind::Rollback)?;
    let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
    let id = request
        .get("snapshotId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| Error::user_cancel("plan not found"))?
        .to_string();
    let result = run_rollback(store, &id, opts).await;
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
    if saved_memories.is_some() && agent_running(tool, opts) {
        return Err(Error::command_failed(
            "close the agent first: it is using its memories folder",
        ));
    }

    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.status == ToolStatus::Drift || envelope.recovery_required {
        return Err(Error::command_failed(
            "the agent's setup was changed after the last deploy; repair it first",
        ));
    }
    let (deployment, body, memory) = current_state(store, tool, &envelope, opts)?;

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
        std::fs::create_dir_all(snapshots_dir(store))?;
        if !same_volume(Path::new(&info.path), &snapshots_dir(store)) {
            return Err(Error::command_failed(
                "the memories folder is on another disk than the snapshots, so it cannot be moved safely",
            ));
        }
    }
    let current_pieces = current_rollback_pieces(tool, &snapshot, opts)?;
    check_extras_volume(store, &current_pieces)?;
    let saved_snapshot = if deployment.present
        || current_differs
        || current_folder.is_some()
        || !current_pieces.is_empty()
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
    if !saved_extras.is_empty() {
        let root = agent_home(tool, opts)?
            .ok_or_else(|| Error::invalid("this agent has no setup folder; nothing was changed"))?;
        let _lock = HomeLock::acquire(store.paths())?;
        // Whatever is there now goes into the snapshot made above first.
        if let Some(snapshot) = &saved_snapshot {
            move_extras_into(store, snapshot, &root)?;
        }
        for (saved, source) in &saved_extras {
            // A config file saved without its login is put back only where there is none now:
            // one the agent wrote since (a new login) is never replaced by the stripped copy.
            if saved.kind == "saved-without-login"
                && std::fs::symlink_metadata(root.join(&saved.name)).is_ok()
            {
                continue;
            }
            extras_restored += u64::from(restore_extra(&root, saved, source)?);
        }
    }
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
        restore_file(&path, bytes, saved.mode)?;
        memory_restored = true;
    }

    let mut memories_restored = false;
    if let (Some(saved), Some(source)) = (&snapshot.memories, &saved_memories) {
        let _lock = HomeLock::acquire(store.paths())?;
        let target = PathBuf::from(&saved.path);
        let expected = home_dir(opts)?.join(".codex").join("memories");
        if tool != ToolKind::Codex || target != expected {
            return Err(Error::invalid(
                "the memories folder is not the one in the user's home; nothing was changed",
            ));
        }
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
                if let Err(error) = move_memories_into(store, snapshot, &expected) {
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
                    let _ = std::fs::rename(memories_in_snapshot(store, &snapshot.id)?, &expected);
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
    })
}

/// Put one saved piece of setup back: copy and check it beside its place, then rename it in. Only
/// an empty folder may be in the way; anything else there is left alone and the piece is refused.
fn restore_extra(root: &Path, saved: &ExtraMeta, source: &Path) -> Result<bool> {
    let target = root.join(&saved.name);
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
    fn extras_skip_links_projects_and_logins() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("rules")).unwrap();
        std::fs::write(root.join("rules/a.md"), "rule\n").unwrap();
        std::fs::create_dir_all(root.join("agents/repo/.git")).unwrap();
        std::fs::create_dir_all(root.join("commands")).unwrap();
        std::fs::write(root.join("commands/auth.json"), "{}").unwrap();

        let rules = extra_info(root, "rules", "saved").unwrap().unwrap();
        assert_eq!((rules.files, rules.bytes), (1, 5));
        assert!(extra_info(root, "agents", "saved").unwrap().is_none());
        assert!(extra_info(root, "commands", "saved").unwrap().is_none());
        assert!(extra_info(root, "missing", "saved").unwrap().is_none());
        for bad in ["", "../rules", "/etc"] {
            assert!(!valid_extra_name(bad), "{bad:?}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("rules"), root.join("linked")).unwrap();
            assert!(extra_info(root, "linked", "saved").unwrap().is_none());
            assert!(extra_info(root, "linked", "erased").unwrap().is_none());
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
    }

    #[test]
    fn ids_are_plain_and_short() {
        assert!(valid_id("20260101T000000Z-claude-abc123"));
        for bad in ["", "..", "../x", "a/b", "a b", "a\\b", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }
}
