//! Prompt deployment helpers.
//!
//! Quick Deploy accepts content supplied by the user and then uses the normal
//! prompt-library activation lifecycle. The legacy harness entry points remain
//! for compatibility, but they never fetch a remote default prompt.

use serde::{Deserialize, Serialize};

use crate::adapter::AdapterOptions;
use crate::db::markdown::content_sha;
use crate::db::Store;
use crate::error::{Error, Result};
use crate::lock::HomeLock;
use crate::models::{
    PlanActivateInput, PlanDeactivateInput, PromptSort, Scope, ToolKind, ToolStatus,
};
use crate::ops::{self, confirm_activate, confirm_deactivate, plan_activate, plan_deactivate};

pub const MAX_PROMPT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessSource {
    pub tool: ToolKind,
    pub title: &'static str,
}

impl HarnessSource {
    pub fn title(self) -> &'static str {
        self.title
    }
}

/// Legacy labels retained for compatibility. They are not remote sources.
pub fn harness_source(tool: ToolKind) -> HarnessSource {
    let title = match tool {
        ToolKind::Claude => "Claude prompt",
        ToolKind::Codex => "Codex prompt",
        ToolKind::Grok => "Grok prompt",
        ToolKind::Zcode => "ZCode prompt",
    };
    HarnessSource { tool, title }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessOutcome {
    pub ok: bool,
    pub tool: ToolKind,
    pub action: HarnessAction,
    pub prompt_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessState {
    pub tool: ToolKind,
    pub deployed: bool,
    pub error: Option<String>,
    /// The library prompt the machine is running, when it can be identified.
    #[serde(default)]
    pub prompt_id: Option<String>,
    #[serde(default)]
    pub prompt_title: Option<String>,
}

/// Which library prompt is live on the machine for `tool`.
///
/// The activation record is authoritative. It can be missing when the machine
/// was deployed by an older version or the index was rebuilt from the markdown
/// library, so the adapter's content fingerprint is then matched against the
/// library instead of leaving a live deployment unnamed.
fn deployed_prompt(
    store: &Store,
    tool: ToolKind,
    fingerprint: Option<&str>,
) -> Option<(String, String)> {
    if let Ok(Some(activation)) = store.find_activation(tool, Scope::User, None) {
        if activation.status == ToolStatus::Active {
            // A prompt deleted from the library is not "the library prompt that
            // is live": the machine then runs text the library no longer has.
            let prompt = activation
                .prompt_id
                .as_deref()
                .and_then(|id| store.get_prompt(id).ok())
                .filter(|prompt| prompt.deleted_at.is_none());
            if let Some(prompt) = prompt {
                return Some((prompt.id, prompt.title));
            }
        }
    }
    let fingerprint = fingerprint.filter(|value| !value.is_empty())?;
    store
        .list_prompts(tool, None, None, PromptSort::Updated)
        .ok()?
        .into_iter()
        .find(|item| item.sha256 == fingerprint)
        .map(|item| (item.id, item.title))
}

pub async fn harness_state(
    store: &Store,
    tool: ToolKind,
    opts: &AdapterOptions,
) -> Result<HarnessState> {
    if let Some(reason) = tool.unavailable_reason() {
        return Ok(HarnessState {
            tool,
            deployed: false,
            error: Some(reason.to_string()),
            prompt_id: None,
            prompt_title: None,
        });
    }
    match ops::tool_status(store, tool, Scope::User, None, opts).await {
        Ok(envelope) => {
            let deployed = envelope.status == ToolStatus::Active;
            let prompt = if deployed {
                deployed_prompt(store, tool, envelope.current_fingerprint.as_deref())
            } else {
                None
            };
            let (prompt_id, prompt_title) = match prompt {
                Some((id, title)) => (Some(id), Some(title)),
                None => (None, None),
            };
            Ok(HarnessState {
                tool,
                deployed,
                error: None,
                prompt_id,
                prompt_title,
            })
        }
        Err(error) => Ok(HarnessState {
            tool,
            deployed: false,
            error: Some(error.to_string()),
            prompt_id: None,
            prompt_title: None,
        }),
    }
}

/// Roles the adapters use for the file that holds the live prompt itself.
/// Directories (`config-root`, `codex-dir`, `managed_dir`) and wrappers are not
/// prompt text and are never read.
const LIVE_PROMPT_ROLES: [&str; 4] = ["memory", "instruction", "rule", "system_file"];

/// Take the prompt that is live on the machine into the library so it can be
/// edited, versioned and redeployed.
///
/// The text is only accepted when it is provably the live prompt: a file the
/// adapter reported must hash to the same fingerprint the adapter reports for
/// the deployment. When no file matches, nothing is guessed and an error says
/// the prompt cannot be read back; the caller falls back to pasting it.
pub async fn adopt_live_prompt(
    store: &Store,
    tool: ToolKind,
    title: &str,
    opts: &AdapterOptions,
) -> Result<String> {
    if let Some(reason) = tool.unavailable_reason() {
        return Err(Error::unavailable(reason.to_string()));
    }
    let envelope = ops::tool_status(store, tool, Scope::User, None, opts).await?;
    if envelope.status != ToolStatus::Active {
        return Err(Error::invalid("no prompt is deployed for this tool"));
    }
    let fingerprint = envelope
        .current_fingerprint
        .as_deref()
        .map(|value| value.strip_prefix("sha256:").unwrap_or(value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::command_failed("the live prompt cannot be read back"))?;
    for target in &envelope.target_paths {
        if !target.exists || !LIVE_PROMPT_ROLES.contains(&target.role.as_str()) {
            continue;
        }
        let Some(body) = read_prompt_file(std::path::Path::new(&target.path)) else {
            continue;
        };
        if content_sha(&body) == fingerprint {
            return store_prompt_body(store, tool, title, &body, vec!["imported".to_string()]);
        }
    }
    Err(Error::command_failed("the live prompt cannot be read back"))
}

/// A regular, UTF-8, size-bounded file, or nothing.
fn read_prompt_file(path: &std::path::Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() as usize > MAX_PROMPT_BYTES {
        return None;
    }
    String::from_utf8(std::fs::read(path).ok()?).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarnessAction {
    Deploy,
    Remove,
}

pub fn accept_prompt_body(bytes: &[u8]) -> Result<String> {
    if bytes.is_empty() {
        return Err(Error::command_failed("prompt body is empty"));
    }
    if bytes.len() > MAX_PROMPT_BYTES {
        return Err(Error::command_failed(format!(
            "prompt body exceeds {MAX_PROMPT_BYTES} bytes"
        )));
    }
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::command_failed("prompt body is not utf-8"))?;
    if text.trim().is_empty() {
        return Err(Error::command_failed("prompt body is empty"));
    }
    Ok(text)
}

fn store_prompt_body(
    store: &Store,
    tool: ToolKind,
    title: &str,
    body: &str,
    tags: Vec<String>,
) -> Result<String> {
    let body = accept_prompt_body(body.as_bytes())?;
    let title = title.trim();
    if title.is_empty() {
        return Err(Error::invalid("prompt title is required"));
    }
    // The lookup and insertion must share the home lock. Acquiring it via
    // ops::create_prompt after the lookup would allow concurrent pastes to
    // create duplicate entries.
    let _lock = HomeLock::acquire(store.paths())?;
    let sha = content_sha(&body);
    if let Some(existing) = store
        .list_prompts(tool, None, None, PromptSort::Updated)?
        .into_iter()
        .find(|item| item.sha256 == sha)
    {
        // Content identity wins: reusing a library entry never renames or
        // retags it behind the user's back. The returned detail tells the UI
        // which existing entry will actually be deployed.
        return Ok(existing.id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    store.insert_prompt(&id, tool, title, &body, &tags, false)?;
    Ok(id)
}

pub fn store_pasted_prompt(
    store: &Store,
    tool: ToolKind,
    title: &str,
    body: &str,
) -> Result<String> {
    store_prompt_body(store, tool, title, body, vec!["pasted".to_string()])
}

pub fn store_harness_prompt(store: &Store, tool: ToolKind, body: &str) -> Result<String> {
    store_prompt_body(
        store,
        tool,
        harness_source(tool).title(),
        body,
        vec!["harness".to_string()],
    )
}

pub async fn deploy_harness(
    store: &Store,
    tool: ToolKind,
    opts: &AdapterOptions,
) -> Result<HarnessOutcome> {
    deploy_harness_with(store, tool, opts, None).await
}

pub async fn deploy_harness_with(
    store: &Store,
    tool: ToolKind,
    opts: &AdapterOptions,
    body: Option<String>,
) -> Result<HarnessOutcome> {
    if let Some(reason) = tool.unavailable_reason() {
        return Ok(failed(tool, HarnessAction::Deploy, reason));
    }
    let body = match body {
        Some(body) => accept_prompt_body(body.as_bytes())?,
        None => {
            return Ok(failed(
                tool,
                HarnessAction::Deploy,
                "prompt content is required; paste or import a prompt before deploying",
            ))
        }
    };
    let prompt_id = match store_harness_prompt(store, tool, &body) {
        Ok(id) => id,
        Err(error) => return Ok(failed(tool, HarnessAction::Deploy, error.to_string())),
    };
    let plan = match plan_activate(
        store,
        PlanActivateInput {
            prompt_id: prompt_id.clone(),
            scope: Scope::User,
            project_dir: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
        },
        opts,
    )
    .await
    {
        Ok(plan) => plan,
        Err(error) => return Ok(failed(tool, HarnessAction::Deploy, error.to_string())),
    };
    if !plan.envelope.ok || !plan.envelope.blockers.is_empty() {
        let reason = plan
            .envelope
            .error
            .clone()
            .filter(|item| !item.is_empty())
            .or_else(|| plan.envelope.blockers.first().cloned())
            .unwrap_or_else(|| "preview reported blockers".to_string());
        return Ok(HarnessOutcome {
            ok: false,
            tool,
            action: HarnessAction::Deploy,
            prompt_id: Some(prompt_id),
            error: Some(reason),
        });
    }
    match confirm_activate(store, &plan.operation_id, opts).await {
        Ok(result) if result.envelope.ok => Ok(HarnessOutcome {
            ok: true,
            tool,
            action: HarnessAction::Deploy,
            prompt_id: Some(prompt_id),
            error: None,
        }),
        Ok(result) => Ok(HarnessOutcome {
            ok: false,
            tool,
            action: HarnessAction::Deploy,
            prompt_id: Some(prompt_id),
            error: Some(
                result
                    .envelope
                    .error
                    .unwrap_or_else(|| "deploy failed".to_string()),
            ),
        }),
        Err(error) => Ok(HarnessOutcome {
            ok: false,
            tool,
            action: HarnessAction::Deploy,
            prompt_id: Some(prompt_id),
            error: Some(error.to_string()),
        }),
    }
}

pub async fn remove_harness(
    store: &Store,
    tool: ToolKind,
    opts: &AdapterOptions,
) -> Result<HarnessOutcome> {
    if let Some(reason) = tool.unavailable_reason() {
        return Ok(failed(tool, HarnessAction::Remove, reason));
    }
    let plan = match plan_deactivate(
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
    {
        Ok(plan) => plan,
        Err(error) => return Ok(failed(tool, HarnessAction::Remove, error.to_string())),
    };
    if !plan.envelope.ok || !plan.envelope.blockers.is_empty() || plan.envelope.recovery_required {
        let reason = plan
            .envelope
            .error
            .clone()
            .filter(|item| !item.is_empty())
            .or_else(|| plan.envelope.blockers.first().cloned())
            .unwrap_or_else(|| "preview reported blockers".to_string());
        return Ok(failed(tool, HarnessAction::Remove, reason));
    }
    match confirm_deactivate(store, &plan.operation_id, opts).await {
        Ok(result) if result.envelope.ok => Ok(HarnessOutcome {
            ok: true,
            tool,
            action: HarnessAction::Remove,
            prompt_id: None,
            error: None,
        }),
        Ok(result) => Ok(failed(
            tool,
            HarnessAction::Remove,
            result
                .envelope
                .error
                .unwrap_or_else(|| "remove failed".to_string()),
        )),
        Err(error) => Ok(failed(tool, HarnessAction::Remove, error.to_string())),
    }
}

fn failed(tool: ToolKind, action: HarnessAction, error: impl Into<String>) -> HarnessOutcome {
    HarnessOutcome {
        ok: false,
        tool,
        action,
        prompt_id: None,
        error: Some(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Activation;
    use crate::paths::AppPaths;

    fn store() -> (tempfile::TempDir, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_home(tmp.path().join("switch"));
        paths.ensure().unwrap();
        let store = Store::open(&paths).unwrap();
        (tmp, store)
    }

    fn activation(prompt_id: &str, status: ToolStatus) -> Activation {
        Activation {
            id: "user-grok".into(),
            prompt_id: Some(prompt_id.into()),
            tool: ToolKind::Grok,
            scope: Scope::User,
            project_dir: None,
            status,
            fingerprint: None,
            operation_id: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn live_prompt_is_named_by_the_machine_fingerprint_when_no_record_exists() {
        let (_tmp, store) = store();
        store
            .insert_prompt("p1", ToolKind::Grok, "Rules", "body\n", &[], false)
            .unwrap();
        let sha = content_sha("body\n");

        assert_eq!(
            deployed_prompt(&store, ToolKind::Grok, Some(&sha)),
            Some(("p1".to_string(), "Rules".to_string()))
        );
        assert_eq!(
            deployed_prompt(&store, ToolKind::Grok, Some("deadbeef")),
            None
        );
        assert_eq!(deployed_prompt(&store, ToolKind::Grok, Some("")), None);
        assert_eq!(deployed_prompt(&store, ToolKind::Grok, None), None);
        assert_eq!(
            deployed_prompt(&store, ToolKind::Claude, Some(&sha)),
            None,
            "another tool's library is never searched"
        );
    }

    #[test]
    fn an_active_record_wins_and_an_inactive_one_is_ignored() {
        let (_tmp, store) = store();
        store
            .insert_prompt("p1", ToolKind::Grok, "One", "one\n", &[], false)
            .unwrap();
        store
            .insert_prompt("p2", ToolKind::Grok, "Two", "two\n", &[], false)
            .unwrap();
        store
            .upsert_activation(&activation("p1", ToolStatus::Active))
            .unwrap();

        assert_eq!(
            deployed_prompt(&store, ToolKind::Grok, Some(&content_sha("two\n"))),
            Some(("p1".to_string(), "One".to_string())),
            "the recorded activation is authoritative"
        );

        store
            .upsert_activation(&activation("p1", ToolStatus::Inactive))
            .unwrap();
        assert_eq!(
            deployed_prompt(&store, ToolKind::Grok, Some(&content_sha("two\n"))),
            Some(("p2".to_string(), "Two".to_string())),
            "without an active record the fingerprint decides"
        );
    }
}
