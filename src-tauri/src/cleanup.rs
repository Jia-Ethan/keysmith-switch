//! Clean an agent back to nothing, and roll that back.
//!
//! Cleaning an agent means: take away what Keysmith deployed (the system prompt and the
//! managed block in the agent's config, through the adapter's own uninstall) and, for
//! Claude Code, empty the user-level memory file `~/.claude/CLAUDE.md`, including what
//! the person wrote in it. That last part is the only place Keysmith writes an agent's
//! file itself instead of asking an adapter, so it is held to a narrow rule: user scope
//! only, exactly that one file, never a link, and never before a complete copy has been
//! saved and checked.
//!
//! Before anything is changed a snapshot is written to `~/.keysmith-switch/snapshots/`:
//! the text of the live prompt (when it can be proven to be the live text) and a byte
//! copy of the memory file. Rolling back redeploys that text through the normal adapter
//! plan and puts the memory file's bytes back. Rolling back first saves the current state
//! as a snapshot of its own, so a rollback can be undone too.
//!
//! Project files, project and local scope deployments, an agent's own memory folders and
//! the prompt library are never touched.

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub operation_id: String,
    pub tool: ToolKind,
    pub deployment: DeploymentMeta,
    pub memory: Option<MemoryMeta>,
    pub nothing_to_do: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub snapshot_id: Option<String>,
    pub deactivated: bool,
    pub memory_cleared: bool,
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
pub struct RollbackPlan {
    pub operation_id: String,
    pub snapshot: SnapshotMeta,
    pub current_title: Option<String>,
    pub replaces_deployment: bool,
    pub memory: Option<RollbackMemory>,
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

/// The user-level memory file, for the agents that have one Keysmith knows about (Claude Code).
/// The path comes from the adapter, and is accepted only when it is exactly the file in the
/// user's own home and an ordinary file.
fn memory_file(
    tool: ToolKind,
    envelope: &Envelope,
    opts: &AdapterOptions,
) -> Result<Option<PathBuf>> {
    if tool != ToolKind::Claude {
        return Ok(None);
    }
    let Some(target) = envelope
        .target_paths
        .iter()
        .find(|target| target.role == "memory" && target.exists)
    else {
        return Ok(None);
    };
    let home = opts
        .home
        .clone()
        .or_else(dirs::home_dir)
        .ok_or_else(|| Error::invalid("the home folder is unknown"))?;
    let expected = home.join(".claude").join("CLAUDE.md");
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
fn create_snapshot(
    store: &Store,
    tool: ToolKind,
    kind: &str,
    deployment: DeploymentMeta,
    prompt: Option<&str>,
    memory: Option<(MemoryMeta, &[u8])>,
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
    let mut blockers = Vec::new();
    if envelope.status == ToolStatus::Drift || envelope.recovery_required {
        blockers.push(
            "the agent's setup was changed after the last deploy; repair it first".to_string(),
        );
    }
    let memory_meta = memory.as_ref().map(|(info, _)| info.clone());
    let nothing_to_do = !deployment.present && memory_meta.as_ref().is_none_or(|m| m.bytes == 0);
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
        }),
        &preview_envelope(&envelope, "plan-cleanup"),
    )?;
    Ok(CleanupPlan {
        operation_id: operation.id,
        tool,
        deployment,
        memory: memory_meta,
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
    opts: &AdapterOptions,
) -> Result<CleanupResult> {
    let plan = ops::require_preview(store, operation_id, OperationKind::Cleanup)?;
    let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
    let result = run_cleanup(store, plan.tool, &request, opts).await;
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
    opts: &AdapterOptions,
) -> Result<CleanupResult> {
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.status == ToolStatus::Drift || envelope.recovery_required {
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

    let snapshot = if deployment.present || memory.as_ref().is_some_and(|(info, _)| info.bytes > 0)
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
    }

    let mut memory_cleared = false;
    if let Some((info, _)) = &memory {
        if info.bytes > 0 {
            let _lock = HomeLock::acquire(store.paths())?;
            truncate_file(Path::new(&info.path))?;
            memory_cleared = true;
        }
    }
    Ok(CleanupResult {
        snapshot_id: snapshot.map(|meta| meta.id),
        deactivated,
        memory_cleared,
    })
}

pub async fn plan_rollback(
    store: &Store,
    snapshot_id: &str,
    opts: &AdapterOptions,
) -> Result<RollbackPlan> {
    let snapshot = read_snapshot(store, snapshot_id)?;
    snapshot_memory(store, &snapshot)?;
    let tool = snapshot.tool;
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    let (deployment, _body, memory) = current_state(store, tool, &envelope, opts)?;
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
        saves_current: deployment.present || overwrites_memory,
        memory: memory_plan,
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

    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.status == ToolStatus::Drift || envelope.recovery_required {
        return Err(Error::command_failed(
            "the agent's setup was changed after the last deploy; repair it first",
        ));
    }
    let (deployment, body, memory) = current_state(store, tool, &envelope, opts)?;

    // The deploy is planned first; if the adapter would refuse it, nothing has been touched.
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

    // What is there now is kept, so this rollback can be undone as well.
    let current_differs = match (&snapshot.memory, &memory) {
        (Some(saved), Some((info, _))) => info.sha256 != saved.sha256 && info.bytes > 0,
        _ => false,
    };
    let saved_snapshot_id = if deployment.present || current_differs {
        Some(
            create_snapshot(
                store,
                tool,
                "before-rollback",
                deployment.clone(),
                body.as_deref(),
                memory
                    .as_ref()
                    .map(|(info, bytes)| (info.clone(), bytes.as_slice())),
            )?
            .id,
        )
    } else {
        None
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
                let home = opts.home.clone().or_else(dirs::home_dir)?;
                let expected = home.join(".claude").join("CLAUDE.md");
                (tool == ToolKind::Claude && expected.to_string_lossy() == saved.path)
                    .then_some(expected)
            })
            .ok_or_else(|| Error::invalid("the memory file could not be found"))?;
        let _lock = HomeLock::acquire(store.paths())?;
        restore_file(&path, bytes, saved.mode)?;
        memory_restored = true;
    }
    Ok(RollbackResult {
        saved_snapshot_id,
        redeployed,
        memory_restored,
    })
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
    fn ids_are_plain_and_short() {
        assert!(valid_id("20260101T000000Z-claude-abc123"));
        for bad in ["", "..", "../x", "a/b", "a b", "a\\b", &"x".repeat(65)] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }
}
