use std::path::PathBuf;

use serde_json::json;

use crate::adapter::{
    check_adapter_version, run_adapter_with, AdapterCommand, AdapterOptions, Envelope,
};
use crate::db::Store;
use crate::diff::{change_summary, unified_diff};
use crate::error::{Error, Result};
use crate::lock::HomeLock;
use crate::models::{
    now_rfc3339, sha256_hex, validate_scope, Activation, CreatePromptInput, DiffResult, Operation,
    OperationKind, OperationResult, OperationStatus, PlanActivateInput, PlanDeactivateInput,
    PromptDetail, PromptVersion, RebuildReport, Scope, ToolKind, ToolStatus, UpdatePromptInput,
};

pub fn create_prompt(store: &Store, input: CreatePromptInput) -> Result<PromptDetail> {
    let _lock = HomeLock::acquire(store.paths())?;
    if input.title.trim().is_empty() {
        return Err(Error::invalid("title is required"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    store.insert_prompt(
        &id,
        input.tool,
        input.title.trim(),
        &input.content,
        &input.tags,
        false,
    )
}

pub fn update_prompt(store: &Store, input: UpdatePromptInput) -> Result<PromptDetail> {
    let _lock = HomeLock::acquire(store.paths())?;
    store.update_prompt(
        &input.id,
        input.title.as_deref(),
        input.content.as_deref(),
        input.tags.as_deref(),
    )
}

pub fn delete_prompt(store: &Store, id: &str) -> Result<()> {
    let _lock = HomeLock::acquire(store.paths())?;
    store.soft_delete_prompt(id)
}

pub fn copy_prompt(store: &Store, id: &str, target_tool: ToolKind) -> Result<PromptDetail> {
    let _lock = HomeLock::acquire(store.paths())?;
    let source = store.get_prompt(id)?;
    if source.deleted_at.is_some() {
        return Err(Error::invalid("cannot copy a deleted prompt"));
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    store.insert_prompt(
        &new_id,
        target_tool,
        &source.title,
        &source.content,
        &source.tags,
        false,
    )
}

pub fn restore_prompt_version(store: &Store, id: &str, version: i64) -> Result<PromptDetail> {
    let _lock = HomeLock::acquire(store.paths())?;
    store.restore_version(id, version)
}

pub fn rebuild_index(store: &Store) -> Result<RebuildReport> {
    let _lock = HomeLock::acquire(store.paths())?;
    store.rebuild_from_markdown()
}

pub fn prompt_history(store: &Store, id: &str) -> Result<Vec<PromptVersion>> {
    store.list_versions(id)
}

pub fn prompt_diff(
    store: &Store,
    id: &str,
    from_version: i64,
    to_version: i64,
) -> Result<DiffResult> {
    let from = store
        .get_version(id, from_version)?
        .ok_or_else(|| Error::invalid(format!("version {from_version} not found")))?;
    let to = store
        .get_version(id, to_version)?
        .ok_or_else(|| Error::invalid(format!("version {to_version} not found")))?;
    Ok(DiffResult {
        unified: unified_diff(
            &from.content,
            &to.content,
            &format!("v{from_version}"),
            &format!("v{to_version}"),
        ),
        summary: change_summary(&from.content, &to.content),
    })
}

pub async fn plan_activate(
    store: &Store,
    input: PlanActivateInput,
    opts: &AdapterOptions,
) -> Result<OperationResult> {
    let _lock = HomeLock::acquire(store.paths())?;
    let prompt = store.get_prompt(&input.prompt_id)?;
    if prompt.deleted_at.is_some() {
        return Err(Error::invalid("cannot activate a deleted prompt"));
    }
    validate_scope(prompt.tool, input.scope, input.project_dir.as_deref())?;
    let name = cli_name(&prompt);
    let command = AdapterCommand::PlanActivate {
        file: prompt.path.clone(),
        scope: input.scope,
        project_dir: input.project_dir.clone(),
        name: Some(name.clone()),
    };
    let envelope = run_adapter_with(prompt.tool, command, opts).await?;
    let request = json!({
        "promptId": prompt.id,
        "tool": prompt.tool,
        "scope": input.scope,
        "projectDir": input.project_dir,
        "name": name,
        "file": prompt.path,
        "fileSha256": prompt.sha256,
    });
    let operation = store_preview(
        store,
        prompt.tool,
        OperationKind::PlanActivate,
        Some(&prompt.id),
        input.scope,
        input.project_dir.as_ref(),
        request,
        &envelope,
    )?;
    Ok(OperationResult {
        operation_id: operation.id,
        envelope,
    })
}

pub async fn confirm_activate(
    store: &Store,
    operation_id: &str,
    opts: &AdapterOptions,
) -> Result<OperationResult> {
    let _lock = HomeLock::acquire(store.paths())?;
    let plan = require_preview(store, operation_id, OperationKind::PlanActivate)?;
    let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
    let prompt_id = request
        .get("promptId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| Error::user_cancel("activate without confirmed plan"))?;
    let prompt = store.get_prompt(prompt_id)?;
    let planned_sha = request
        .get("fileSha256")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if planned_sha != prompt.sha256 {
        return Err(Error::user_cancel(
            "prompt changed since preview; run plan-activate again",
        ));
    }
    if let Some(plan_env) = plan.envelope_json.as_deref() {
        if let Ok(preview) = serde_json::from_str::<Envelope>(plan_env) {
            if !preview.ok || !preview.blockers.is_empty() {
                return Err(Error::command_failed(
                    preview
                        .error
                        .unwrap_or_else(|| "preview reported blockers".into()),
                ));
            }
        }
    }
    if let Err(error) = check_adapter_version(plan.tool, opts).await {
        store.update_operation(
            operation_id,
            OperationStatus::Failed,
            None,
            Some(&error.to_string()),
        )?;
        return Err(error);
    }
    let scope = plan.scope.unwrap_or(Scope::User);
    let project_dir = plan.project_dir.as_ref().map(PathBuf::from);
    let name = request
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| cli_name(&prompt));
    let command = AdapterCommand::Activate {
        file: prompt.path.clone(),
        scope,
        project_dir: project_dir.clone(),
        name: Some(name),
    };
    let envelope = run_adapter_with(plan.tool, command, opts).await?;
    let execute_id = persist_execute(store, &plan, OperationKind::Activate, &envelope, request)?;
    if envelope.ok {
        record_activation(
            store,
            &prompt,
            scope,
            project_dir.as_ref(),
            &execute_id,
            &envelope,
        )?;
        store.update_operation(operation_id, OperationStatus::Succeeded, None, None)?;
    } else {
        store.update_operation(
            operation_id,
            OperationStatus::Failed,
            None,
            envelope.error.as_deref(),
        )?;
    }
    Ok(OperationResult {
        operation_id: execute_id,
        envelope,
    })
}

pub async fn plan_deactivate(
    store: &Store,
    input: PlanDeactivateInput,
    opts: &AdapterOptions,
) -> Result<OperationResult> {
    let _lock = HomeLock::acquire(store.paths())?;
    validate_scope(input.tool, input.scope, input.project_dir.as_deref())?;
    let name = match input.prompt_id.as_deref() {
        Some(id) => store.get_prompt(id).ok().map(|prompt| cli_name(&prompt)),
        None => Some(input.tool.default_name().to_string()),
    };
    let status = run_adapter_with(
        input.tool,
        AdapterCommand::Status {
            scope: input.scope,
            project_dir: input.project_dir.clone(),
            name: name.clone(),
        },
        opts,
    )
    .await?;
    let drifted = status.status == ToolStatus::Drift || status.recovery_required;
    let mut envelope = if drifted {
        let mut blocked = status;
        blocked.command = OperationKind::PlanDeactivate.as_str().to_string();
        blocked.ok = false;
        blocked.preview = true;
        blocked.recovery_required = true;
        blocked.status = if blocked.status == ToolStatus::Drift {
            ToolStatus::Drift
        } else {
            ToolStatus::RecoveryRequired
        };
        blocked
            .blockers
            .push("unmanaged edits detected; deactivate refused, recover instead".into());
        blocked.error = Some("drift; recovery required".into());
        blocked
    } else {
        run_adapter_with(
            input.tool,
            AdapterCommand::PlanDeactivate {
                scope: input.scope,
                project_dir: input.project_dir.clone(),
                name: name.clone(),
            },
            opts,
        )
        .await?
    };
    if drifted {
        envelope.recovery_required = true;
    }
    let request = json!({
        "promptId": input.prompt_id,
        "tool": input.tool,
        "scope": input.scope,
        "projectDir": input.project_dir,
        "name": name,
        "drift": drifted,
    });
    let operation = store_preview(
        store,
        input.tool,
        OperationKind::PlanDeactivate,
        input.prompt_id.as_deref(),
        input.scope,
        input.project_dir.as_ref(),
        request,
        &envelope,
    )?;
    Ok(OperationResult {
        operation_id: operation.id,
        envelope,
    })
}

pub async fn confirm_deactivate(
    store: &Store,
    operation_id: &str,
    opts: &AdapterOptions,
) -> Result<OperationResult> {
    let _lock = HomeLock::acquire(store.paths())?;
    let plan = require_preview(store, operation_id, OperationKind::PlanDeactivate)?;
    let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
    if request.get("drift").and_then(|value| value.as_bool()) == Some(true) {
        store.update_operation(
            operation_id,
            OperationStatus::Cancelled,
            None,
            Some("drift; recovery required"),
        )?;
        return Err(Error::recovery_required(
            "deactivate refused because unmanaged edits were detected",
        ));
    }
    if let Some(plan_env) = plan.envelope_json.as_deref() {
        if let Ok(preview) = serde_json::from_str::<Envelope>(plan_env) {
            if preview.recovery_required || preview.status == ToolStatus::Drift {
                return Err(Error::recovery_required(
                    "deactivate refused because unmanaged edits were detected",
                ));
            }
            if !preview.ok || !preview.blockers.is_empty() {
                return Err(Error::command_failed(
                    preview
                        .error
                        .unwrap_or_else(|| "preview reported blockers".into()),
                ));
            }
        }
    }
    let scope = plan.scope.unwrap_or(Scope::User);
    let project_dir = plan.project_dir.as_ref().map(PathBuf::from);
    let name = request
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let envelope = run_adapter_with(
        plan.tool,
        AdapterCommand::Deactivate {
            scope,
            project_dir: project_dir.clone(),
            name,
        },
        opts,
    )
    .await?;
    let execute_id = persist_execute(store, &plan, OperationKind::Deactivate, &envelope, request)?;
    if envelope.ok {
        if let Some(mut existing) = store.find_activation(
            plan.tool,
            scope,
            project_dir.as_ref().and_then(|p| p.to_str()),
        )? {
            existing.status = ToolStatus::Inactive;
            existing.operation_id = Some(execute_id.clone());
            existing.updated_at = now_rfc3339();
            store.upsert_activation(&existing)?;
        }
        store.upsert_tool_state(
            plan.tool,
            scope,
            project_dir.as_ref().and_then(|p| p.to_str()),
            ToolStatus::Inactive,
            envelope.current_fingerprint.as_deref(),
            Some(&execute_id),
        )?;
        store.update_operation(operation_id, OperationStatus::Succeeded, None, None)?;
    } else {
        store.update_operation(
            operation_id,
            OperationStatus::Failed,
            None,
            envelope.error.as_deref(),
        )?;
    }
    Ok(OperationResult {
        operation_id: execute_id,
        envelope,
    })
}

pub async fn plan_recover(
    store: &Store,
    tool: ToolKind,
    scope: Scope,
    project_dir: Option<PathBuf>,
    opts: &AdapterOptions,
) -> Result<OperationResult> {
    let _lock = HomeLock::acquire(store.paths())?;
    if !tool.recover_supported() {
        let mut envelope = Envelope::new(tool, "recover");
        envelope.ok = false;
        envelope.available = tool.available_on_this_os();
        envelope.status = ToolStatus::Unavailable;
        envelope.unavailable_reason = Some("recover is not supported for ZCode".into());
        envelope.error = Some("recover is not supported for ZCode".into());
        let operation = store_preview(
            store,
            tool,
            OperationKind::Recover,
            None,
            scope,
            project_dir.as_ref(),
            json!({"tool": tool, "scope": scope}),
            &envelope,
        )?;
        return Ok(OperationResult {
            operation_id: operation.id,
            envelope,
        });
    }
    validate_scope(tool, scope, project_dir.as_deref())?;
    let preview = run_adapter_with(
        tool,
        AdapterCommand::Recover {
            scope,
            project_dir: project_dir.clone(),
            execute: false,
        },
        opts,
    )
    .await?;
    let preview_op = store_preview(
        store,
        tool,
        OperationKind::Recover,
        None,
        scope,
        project_dir.as_ref(),
        json!({"tool": tool, "scope": scope, "projectDir": project_dir}),
        &preview,
    )?;
    if !preview.ok {
        return Ok(OperationResult {
            operation_id: preview_op.id,
            envelope: preview,
        });
    }
    Ok(OperationResult {
        operation_id: preview_op.id,
        envelope: preview,
    })
}

pub async fn confirm_recover(
    store: &Store,
    operation_id: &str,
    opts: &AdapterOptions,
) -> Result<OperationResult> {
    let _lock = HomeLock::acquire(store.paths())?;
    let plan = require_preview(store, operation_id, OperationKind::Recover)?;
    if let Some(plan_env) = plan.envelope_json.as_deref() {
        if let Ok(preview) = serde_json::from_str::<Envelope>(plan_env) {
            if !preview.ok || !preview.blockers.is_empty() {
                return Err(Error::command_failed(
                    preview
                        .error
                        .unwrap_or_else(|| "recovery preview reported blockers".into()),
                ));
            }
        }
    }
    let request: serde_json::Value = serde_json::from_str(&plan.request_json)?;
    let scope = plan.scope.unwrap_or(Scope::User);
    let project_dir = plan.project_dir.as_ref().map(PathBuf::from);
    let envelope = run_adapter_with(
        plan.tool,
        AdapterCommand::Recover {
            scope,
            project_dir: project_dir.clone(),
            execute: true,
        },
        opts,
    )
    .await?;
    let execute_id = persist_execute(store, &plan, OperationKind::Recover, &envelope, request)?;
    store.upsert_tool_state(
        plan.tool,
        scope,
        project_dir.as_ref().and_then(|p| p.to_str()),
        envelope.status,
        envelope.current_fingerprint.as_deref(),
        Some(&execute_id),
    )?;
    Ok(OperationResult {
        operation_id: execute_id,
        envelope,
    })
}

pub async fn tool_status(
    store: &Store,
    tool: ToolKind,
    scope: Scope,
    project_dir: Option<PathBuf>,
    opts: &AdapterOptions,
) -> Result<Envelope> {
    validate_scope(tool, scope, project_dir.as_deref())?;
    let envelope = run_adapter_with(
        tool,
        AdapterCommand::Status {
            scope,
            project_dir: project_dir.clone(),
            name: Some(tool.default_name().to_string()),
        },
        opts,
    )
    .await?;
    store.upsert_tool_state(
        tool,
        scope,
        project_dir.as_ref().and_then(|p| p.to_str()),
        envelope.status,
        envelope.current_fingerprint.as_deref(),
        None,
    )?;
    Ok(envelope)
}

pub async fn doctor_tool(tool: ToolKind, opts: &AdapterOptions) -> Result<Envelope> {
    run_adapter_with(tool, AdapterCommand::Doctor, opts).await
}

fn require_preview(
    store: &Store,
    operation_id: &str,
    expected: OperationKind,
) -> Result<Operation> {
    let operation = store
        .get_operation(operation_id)?
        .ok_or_else(|| Error::user_cancel("activate without confirmed plan"))?;
    if operation.kind != expected {
        return Err(Error::user_cancel("activate without confirmed plan"));
    }
    if !matches!(
        operation.status,
        OperationStatus::Preview | OperationStatus::Ready
    ) {
        return Err(Error::user_cancel("activate without confirmed plan"));
    }
    if !operation.preview {
        return Err(Error::user_cancel("activate without confirmed plan"));
    }
    Ok(operation)
}

fn store_preview(
    store: &Store,
    tool: ToolKind,
    kind: OperationKind,
    prompt_id: Option<&str>,
    scope: Scope,
    project_dir: Option<&PathBuf>,
    request: serde_json::Value,
    envelope: &Envelope,
) -> Result<Operation> {
    let now = now_rfc3339();
    let operation = Operation {
        id: uuid::Uuid::new_v4().to_string(),
        tool,
        kind,
        status: if envelope.ok {
            OperationStatus::Preview
        } else {
            OperationStatus::Failed
        },
        preview: true,
        prompt_id: prompt_id.map(str::to_string),
        scope: Some(scope),
        project_dir: project_dir.map(|path| path.to_string_lossy().into_owned()),
        request_json: request.to_string(),
        envelope_json: Some(serde_json::to_string(envelope)?),
        error: envelope.error.clone(),
        parent_id: None,
        created_at: now.clone(),
        updated_at: now,
    };
    store.insert_operation(&operation)?;
    Ok(operation)
}

fn persist_execute(
    store: &Store,
    parent: &Operation,
    kind: OperationKind,
    envelope: &Envelope,
    request: serde_json::Value,
) -> Result<String> {
    let now = now_rfc3339();
    let operation = Operation {
        id: uuid::Uuid::new_v4().to_string(),
        tool: parent.tool,
        kind,
        status: if envelope.ok {
            OperationStatus::Succeeded
        } else {
            OperationStatus::Failed
        },
        preview: false,
        prompt_id: parent.prompt_id.clone(),
        scope: parent.scope,
        project_dir: parent.project_dir.clone(),
        request_json: request.to_string(),
        envelope_json: Some(serde_json::to_string(envelope)?),
        error: envelope.error.clone(),
        parent_id: Some(parent.id.clone()),
        created_at: now.clone(),
        updated_at: now,
    };
    store.insert_operation(&operation)?;
    Ok(operation.id)
}

fn record_activation(
    store: &Store,
    prompt: &PromptDetail,
    scope: Scope,
    project_dir: Option<&PathBuf>,
    operation_id: &str,
    envelope: &Envelope,
) -> Result<()> {
    let now = now_rfc3339();
    let project = project_dir.map(|path| path.to_string_lossy().into_owned());
    let activation = Activation {
        id: sha256_hex(format!(
            "{}:{}:{}",
            prompt.tool.as_str(),
            scope.as_str(),
            project.clone().unwrap_or_default()
        )),
        prompt_id: Some(prompt.id.clone()),
        tool: prompt.tool,
        scope,
        project_dir: project.clone(),
        status: if envelope.ok {
            ToolStatus::Active
        } else {
            envelope.status
        },
        fingerprint: envelope
            .target_fingerprint
            .clone()
            .or(envelope.current_fingerprint.clone())
            .or(Some(prompt.sha256.clone())),
        operation_id: Some(operation_id.to_string()),
        created_at: now.clone(),
        updated_at: now,
    };
    store.upsert_activation(&activation)?;
    store.upsert_tool_state(
        prompt.tool,
        scope,
        project.as_deref(),
        activation.status,
        activation.fingerprint.as_deref(),
        Some(operation_id),
    )?;
    Ok(())
}

fn cli_name(prompt: &PromptDetail) -> String {
    prompt.tool.default_name().to_string()
}
