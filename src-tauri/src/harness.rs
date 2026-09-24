//! Tool-level deploy and remove for the four Keysmith harnesses.
//!
//! Deploy fetches the current default prompt from that tool's upstream
//! repository, stores it with the existing prompt library, then runs the
//! existing plan/confirm activate path. Remove runs the existing
//! plan/confirm deactivate path at user scope. Sidecar argv is unchanged.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::adapter::AdapterOptions;
use crate::db::markdown::content_sha;
use crate::db::Store;
use crate::error::{Error, Result};
use crate::models::{
    CreatePromptInput, PlanActivateInput, PlanDeactivateInput, PromptSort, Scope, ToolKind,
    ToolStatus,
};
use crate::ops::{self, confirm_activate, confirm_deactivate, plan_activate, plan_deactivate};

pub const MAX_PROMPT_BYTES: usize = 512 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessSource {
    pub repo: &'static str,
    pub branch: &'static str,
    pub path: &'static str,
}

impl HarnessSource {
    pub fn url(self) -> String {
        format!(
            "https://raw.githubusercontent.com/Jia-Ethan/{}/{}/{}",
            self.repo, self.branch, self.path
        )
    }

    pub fn title(self) -> &'static str {
        self.path.rsplit('/').next().unwrap_or(self.path)
    }
}

pub fn harness_source(tool: ToolKind) -> HarnessSource {
    match tool {
        ToolKind::Claude => HarnessSource {
            repo: "claude-keysmith",
            branch: "main",
            path: "examples/claude-project-rules.md",
        },
        ToolKind::Codex => HarnessSource {
            repo: "codex-keysmith",
            branch: "main",
            path: "examples/gpt-overlay.md",
        },
        ToolKind::Grok => HarnessSource {
            repo: "grok-keysmith",
            branch: "main",
            path: "examples/grok-unrestricted.md",
        },
        ToolKind::Zcode => HarnessSource {
            repo: "zcode-keysmith",
            branch: "master",
            path: "examples/system-role.md",
        },
    }
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

/// Machine state for one tool. `deployed` is the only fact the first screen
/// uses to choose between the deploy button and the remove button.
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

pub async fn fetch_harness_prompt(tool: ToolKind) -> Result<String> {
    let url = harness_source(tool).url();
    fetch_prompt_url(&url).await
}

pub async fn fetch_prompt_url(url: &str) -> Result<String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::invalid("harness prompt url must be http(s)"));
    }
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|err| Error::message(format!("harness fetch client: {err}")))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| Error::command_failed(format!("harness prompt fetch failed: {err}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::command_failed(format!(
            "harness prompt fetch returned {status}"
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| Error::command_failed(format!("harness prompt body: {err}")))?;
    accept_prompt_body(&bytes)
}

pub fn accept_prompt_body(bytes: &[u8]) -> Result<String> {
    if bytes.is_empty() {
        return Err(Error::command_failed("harness prompt body is empty"));
    }
    if bytes.len() > MAX_PROMPT_BYTES {
        return Err(Error::command_failed(format!(
            "harness prompt body exceeds {MAX_PROMPT_BYTES} bytes"
        )));
    }
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::command_failed("harness prompt body is not utf-8"))?;
    if text.trim().is_empty() {
        return Err(Error::command_failed("harness prompt body is empty"));
    }
    Ok(text)
}

/// Store `body` for `tool`. Reuse the newest live prompt with the same content
/// sha instead of inserting another copy.
pub fn store_harness_prompt(store: &Store, tool: ToolKind, body: &str) -> Result<String> {
    let sha = content_sha(body);
    if let Some(existing) = store
        .list_prompts(tool, None, None, PromptSort::Updated)?
        .into_iter()
        .find(|item| item.sha256 == sha)
    {
        return Ok(existing.id);
    }
    let created = ops::create_prompt(
        store,
        CreatePromptInput {
            tool,
            title: harness_source(tool).title().to_string(),
            content: body.to_string(),
            tags: vec!["harness".to_string()],
        },
    )?;
    Ok(created.id)
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
        None => match fetch_harness_prompt(tool).await {
            Ok(body) => body,
            Err(error) => return Ok(failed(tool, HarnessAction::Deploy, error.to_string())),
        },
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
