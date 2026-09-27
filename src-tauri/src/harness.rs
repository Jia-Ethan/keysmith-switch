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
        });
    }
    match ops::tool_status(store, tool, Scope::User, None, opts).await {
        Ok(envelope) => Ok(HarnessState {
            tool,
            deployed: envelope.status == ToolStatus::Active,
            error: None,
        }),
        Err(error) => Ok(HarnessState {
            tool,
            deployed: false,
            error: Some(error.to_string()),
        }),
    }
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
