use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::adapter::process::{find_vendored_script, resolve_cli, resolve_python};
use crate::adapter::{list_tools as adapter_list_tools, AdapterOptions, Envelope};
use crate::db::Store;
use crate::error::{Error, Result};
use crate::models::{
    CreatePromptInput, PlanActivateInput, PlanDeactivateInput, PromptSort, Scope, Settings,
    SettingsPatch, ToolKind, ToolStatus, UpdatePromptInput, APP_VERSION,
};
use crate::official::{
    confirm_official_action as official_confirm, plan_official_action as official_plan,
    OfficialAction, OfficialPlan, OfficialProduct, OfficialResult,
};
use crate::ops;
use crate::paths::AppPaths;
use crate::redact::redact_text;
use crate::updater::{
    check_update, install_update, InstallRequest, UpdateChannel, UpdateCheck, UpdateInstall,
    UpdateRequest,
};

pub struct AppState {
    pub store: Store,
}

impl AppState {
    pub fn open() -> Result<Self> {
        let paths = AppPaths::resolve()?;
        paths.ensure()?;
        Ok(Self {
            store: Store::open(&paths)?,
        })
    }
}

fn opts() -> AdapterOptions {
    AdapterOptions::default()
}

fn parse_sort(raw: Option<&str>) -> PromptSort {
    match raw.unwrap_or("updated") {
        "created" => PromptSort::Created,
        "title" => PromptSort::Title,
        _ => PromptSort::Updated,
    }
}

fn parse_scope(raw: Option<&str>) -> Result<Scope> {
    raw.unwrap_or("user").parse()
}

fn parse_tool(raw: &str) -> Result<ToolKind> {
    raw.parse()
}

fn excerpt(content: &str) -> Option<String> {
    let line = content.lines().find(|item| !item.trim().is_empty())?;
    let mut text = line.trim().to_string();
    if text.len() > 80 {
        text.truncate(80);
        text.push('…');
    }
    Some(text)
}

fn tool_display_name(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Claude => "Claude Code",
        ToolKind::Codex => "Codex",
        ToolKind::Grok => "Grok Build",
        ToolKind::Zcode => "ZCode",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConflict {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiEnvelope {
    pub schema: String,
    pub tool: ToolKind,
    pub command: String,
    pub ok: bool,
    pub preview: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub adapter_version: String,
    pub cli_path: Option<String>,
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub status: ToolStatus,
    pub recovery_required: bool,
    pub scopes: Vec<crate::models::ScopeInfo>,
    pub target_paths: Vec<crate::adapter::envelope::TargetPath>,
    pub planned_files: Vec<crate::adapter::envelope::PlannedFile>,
    pub backups: Vec<crate::adapter::envelope::BackupEntry>,
    pub conflicts: Vec<UiConflict>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    pub current_fingerprint: Option<String>,
    pub target_fingerprint: Option<String>,
    pub doctor: crate::adapter::envelope::DoctorReport,
    pub reload_required: bool,
    pub reload_hint: Option<String>,
    pub error: Option<String>,
    pub redacted_stderr: String,
}

impl From<Envelope> for UiEnvelope {
    fn from(value: Envelope) -> Self {
        let conflicts = value
            .conflicts
            .into_iter()
            .map(|item| {
                if let Some((path, reason)) = item.split_once(':') {
                    UiConflict {
                        path: path.trim().to_string(),
                        reason: reason.trim().to_string(),
                    }
                } else {
                    UiConflict {
                        path: item.clone(),
                        reason: item,
                    }
                }
            })
            .collect();
        Self {
            schema: value.schema,
            tool: value.tool,
            command: value.command,
            ok: value.ok,
            preview: value.preview,
            available: value.available,
            unavailable_reason: value.unavailable_reason,
            adapter_version: value
                .adapter_version
                .unwrap_or_else(|| value.tool.expected_version().to_string()),
            cli_path: value.cli_path,
            argv: value.argv,
            exit_code: value.exit_code,
            status: value.status,
            recovery_required: value.recovery_required,
            scopes: value.scopes,
            target_paths: value.target_paths,
            planned_files: value.planned_files,
            backups: value.backups,
            conflicts,
            warnings: value.warnings,
            blockers: value.blockers,
            current_fingerprint: value.current_fingerprint,
            target_fingerprint: value.target_fingerprint,
            doctor: value.doctor,
            reload_required: value.reload_required,
            reload_hint: value.reload_hint,
            error: value.error,
            redacted_stderr: value.redacted_stderr,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiToolInfo {
    pub id: ToolKind,
    pub name: String,
    pub adapter_version: String,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub supported_scopes: Vec<Scope>,
    pub cli_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPromptSummary {
    pub id: String,
    pub tool: ToolKind,
    pub title: String,
    pub tags: Vec<String>,
    pub active: bool,
    pub last_used_at: Option<String>,
    pub updated_at: String,
    pub created_at: String,
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPromptDetail {
    #[serde(flatten)]
    pub summary: UiPromptSummary,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPromptVersion {
    pub version: i64,
    pub created_at: String,
    pub title: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiActivation {
    pub id: String,
    pub tool: ToolKind,
    pub prompt_id: String,
    pub prompt_title: Option<String>,
    pub scope: Scope,
    pub project_dir: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiOperation {
    pub id: String,
    pub tool: ToolKind,
    pub kind: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub recover_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPlanResult {
    pub operation_id: String,
    pub envelope: UiEnvelope,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub app: AboutApp,
    pub adapters: Vec<AdapterVersionInfo>,
    pub official: Vec<OfficialCard>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutApp {
    pub name: String,
    pub version: String,
    pub channel: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterVersionInfo {
    pub tool: ToolKind,
    pub version: String,
    pub bundled: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialCard {
    pub product: OfficialProduct,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub installed: bool,
    pub executable_path: Option<String>,
    pub source: String,
    pub argv: Vec<String>,
    pub dest: String,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

fn map_plan(result: crate::models::OperationResult) -> UiPlanResult {
    UiPlanResult {
        operation_id: result.operation_id,
        envelope: result.envelope.into(),
    }
}

fn summarize_prompt(
    store: &Store,
    prompt: &crate::models::PromptSummary,
    content: Option<&str>,
) -> Result<UiPromptSummary> {
    let activations = store.list_activations(prompt.tool)?;
    let hit = activations.iter().find(|item| {
        item.prompt_id.as_deref() == Some(prompt.id.as_str()) && item.status == ToolStatus::Active
    });
    Ok(UiPromptSummary {
        id: prompt.id.clone(),
        tool: prompt.tool,
        title: prompt.title.clone(),
        tags: prompt.tags.clone(),
        active: hit.is_some(),
        last_used_at: hit.map(|item| item.updated_at.clone()),
        updated_at: prompt.updated_at.clone(),
        created_at: prompt.created_at.clone(),
        excerpt: content.and_then(excerpt),
    })
}

fn detail_from_store(store: &Store, id: &str) -> Result<UiPromptDetail> {
    let prompt = store.get_prompt(id)?;
    let summary = crate::models::PromptSummary {
        id: prompt.id.clone(),
        tool: prompt.tool,
        title: prompt.title.clone(),
        tags: prompt.tags.clone(),
        version: prompt.version,
        sha256: prompt.sha256.clone(),
        updated_at: prompt.updated_at.clone(),
        created_at: prompt.created_at.clone(),
    };
    Ok(UiPromptDetail {
        summary: summarize_prompt(store, &summary, Some(&prompt.content))?,
        content: prompt.content,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_tools() -> Result<serde_json::Value> {
    let tools: Vec<UiToolInfo> = adapter_list_tools()
        .into_iter()
        .map(|info| {
            let cli_path = resolve_cli(info.id, &opts())
                .ok()
                .map(|cli| cli.script.display().to_string());
            UiToolInfo {
                id: info.id,
                name: tool_display_name(info.id).to_string(),
                adapter_version: info.adapter_version,
                available: info.available,
                unavailable_reason: info.unavailable_reason,
                supported_scopes: info
                    .scopes
                    .into_iter()
                    .filter(|scope| scope.supported)
                    .map(|scope| scope.id)
                    .collect(),
                cli_path,
            }
        })
        .collect();
    Ok(serde_json::json!({ "tools": tools }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_prompts(
    state: State<'_, AppState>,
    tool: String,
    query: Option<String>,
    tag: Option<String>,
    sort: Option<String>,
) -> Result<serde_json::Value> {
    let tool = parse_tool(&tool)?;
    let sort = parse_sort(sort.as_deref());
    let items = state
        .store
        .list_prompts(tool, query.as_deref(), tag.as_deref(), sort)?;
    let mut prompts = Vec::new();
    for item in items {
        let content = state
            .store
            .get_prompt(&item.id)
            .ok()
            .map(|detail| detail.content);
        prompts.push(summarize_prompt(&state.store, &item, content.as_deref())?);
    }
    Ok(serde_json::json!({ "prompts": prompts }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_prompt(state: State<'_, AppState>, id: String) -> Result<UiPromptDetail> {
    detail_from_store(&state.store, &id)
}

#[tauri::command(rename_all = "camelCase")]
pub fn create_prompt(
    state: State<'_, AppState>,
    tool: String,
    title: String,
    content: String,
    tags: Vec<String>,
) -> Result<UiPromptDetail> {
    let created = ops::create_prompt(
        &state.store,
        CreatePromptInput {
            tool: parse_tool(&tool)?,
            title,
            content,
            tags,
        },
    )?;
    detail_from_store(&state.store, &created.id)
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_prompt(
    state: State<'_, AppState>,
    id: String,
    title: Option<String>,
    content: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<UiPromptDetail> {
    ops::update_prompt(
        &state.store,
        UpdatePromptInput {
            id: id.clone(),
            title,
            content,
            tags,
        },
    )?;
    detail_from_store(&state.store, &id)
}

#[tauri::command(rename_all = "camelCase")]
pub fn delete_prompt(state: State<'_, AppState>, id: String) -> Result<serde_json::Value> {
    ops::delete_prompt(&state.store, &id)?;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn copy_prompt(
    state: State<'_, AppState>,
    id: String,
    target_tool: String,
) -> Result<UiPromptDetail> {
    let copied = ops::copy_prompt(&state.store, &id, parse_tool(&target_tool)?)?;
    detail_from_store(&state.store, &copied.id)
}

#[tauri::command(rename_all = "camelCase")]
pub fn prompt_history(state: State<'_, AppState>, id: String) -> Result<serde_json::Value> {
    let versions = ops::prompt_history(&state.store, &id)?
        .into_iter()
        .map(|item| UiPromptVersion {
            version: item.version,
            created_at: item.created_at,
            title: item.title,
            summary: excerpt(&item.content),
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "versions": versions }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn prompt_diff(
    state: State<'_, AppState>,
    id: String,
    from_version: i64,
    to_version: i64,
) -> Result<serde_json::Value> {
    let diff = ops::prompt_diff(&state.store, &id, from_version, to_version)?;
    let summary = format!(
        "+{} / -{}, {} hunks",
        diff.summary.added_lines, diff.summary.removed_lines, diff.summary.hunks
    );
    Ok(serde_json::json!({
        "unified": diff.unified,
        "summary": summary,
    }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn restore_prompt_version(
    state: State<'_, AppState>,
    id: String,
    version: i64,
) -> Result<UiPromptDetail> {
    ops::restore_prompt_version(&state.store, &id, version)?;
    detail_from_store(&state.store, &id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn tool_status(
    state: State<'_, AppState>,
    tool: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<UiEnvelope> {
    let envelope = ops::tool_status(
        &state.store,
        parse_tool(&tool)?,
        parse_scope(scope.as_deref())?,
        project_dir.map(PathBuf::from),
        &opts(),
    )
    .await?;
    Ok(envelope.into())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn plan_activate(
    state: State<'_, AppState>,
    prompt_id: String,
    scope: String,
    project_dir: Option<String>,
) -> Result<UiPlanResult> {
    let result = ops::plan_activate(
        &state.store,
        PlanActivateInput {
            prompt_id,
            scope: scope.parse()?,
            project_dir: project_dir.map(PathBuf::from),
        },
        &opts(),
    )
    .await?;
    Ok(map_plan(result))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn activate(state: State<'_, AppState>, operation_id: String) -> Result<UiPlanResult> {
    let result = ops::confirm_activate(&state.store, &operation_id, &opts()).await?;
    Ok(map_plan(result))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn plan_deactivate(
    state: State<'_, AppState>,
    prompt_id: Option<String>,
    tool: String,
    scope: String,
    project_dir: Option<String>,
) -> Result<UiPlanResult> {
    let result = ops::plan_deactivate(
        &state.store,
        PlanDeactivateInput {
            prompt_id,
            tool: parse_tool(&tool)?,
            scope: scope.parse()?,
            project_dir: project_dir.map(PathBuf::from),
        },
        &opts(),
    )
    .await?;
    Ok(map_plan(result))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn deactivate(state: State<'_, AppState>, operation_id: String) -> Result<UiPlanResult> {
    let result = ops::confirm_deactivate(&state.store, &operation_id, &opts()).await?;
    Ok(map_plan(result))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn recover_tool(
    state: State<'_, AppState>,
    tool: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<UiPlanResult> {
    let result = ops::recover_tool(
        &state.store,
        parse_tool(&tool)?,
        parse_scope(scope.as_deref())?,
        project_dir.map(PathBuf::from),
        &opts(),
    )
    .await?;
    Ok(map_plan(result))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn doctor(tool: String) -> Result<UiEnvelope> {
    let envelope = ops::doctor_tool(parse_tool(&tool)?, &opts()).await?;
    Ok(envelope.into())
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_activations(state: State<'_, AppState>, tool: String) -> Result<serde_json::Value> {
    let tool = parse_tool(&tool)?;
    let mut activations = Vec::new();
    for item in state.store.list_activations(tool)? {
        let prompt_title = item
            .prompt_id
            .as_deref()
            .and_then(|id| state.store.get_prompt(id).ok())
            .map(|prompt| prompt.title);
        activations.push(UiActivation {
            id: item.id,
            tool: item.tool,
            prompt_id: item.prompt_id.unwrap_or_default(),
            prompt_title,
            scope: item.scope,
            project_dir: item.project_dir,
            active: item.status == ToolStatus::Active,
            created_at: item.created_at,
            fingerprint: item.fingerprint,
        });
    }
    Ok(serde_json::json!({ "activations": activations }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_operations(
    state: State<'_, AppState>,
    tool: Option<String>,
) -> Result<serde_json::Value> {
    let tool = tool.as_deref().map(parse_tool).transpose()?;
    let operations = state
        .store
        .list_operations(tool)?
        .into_iter()
        .map(|item| UiOperation {
            recover_available: item.kind == crate::models::OperationKind::Recover
                || item.status == crate::models::OperationStatus::Failed,
            id: item.id,
            tool: item.tool,
            kind: item.kind.as_str().to_string(),
            status: item.status.as_str().to_string(),
            error: item.error,
            created_at: item.created_at,
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "operations": operations }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    state.store.get_settings()
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_settings(
    state: State<'_, AppState>,
    language: Option<String>,
    update_channel: Option<String>,
    advanced_tools_enabled: Option<bool>,
    default_claude_scope: Option<Scope>,
    recent_project_dirs: Option<Vec<String>>,
    updater_endpoint_override: Option<Option<String>>,
) -> Result<Settings> {
    state.store.update_settings(SettingsPatch {
        language,
        update_channel,
        advanced_tools_enabled,
        default_claude_scope,
        recent_project_dirs,
        updater_endpoint_override,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_about(state: State<'_, AppState>) -> Result<AboutInfo> {
    let settings = state.store.get_settings()?;
    let adapters = ToolKind::ALL
        .into_iter()
        .map(|tool| {
            let path = resolve_cli(tool, &opts())
                .ok()
                .map(|cli| cli.script.display().to_string())
                .or_else(|| find_vendored_script(tool).map(|path| path.display().to_string()));
            AdapterVersionInfo {
                tool,
                version: tool.expected_version().to_string(),
                bundled: path.is_some(),
                path,
            }
        })
        .collect();
    let official = [
        OfficialProduct::Claude,
        OfficialProduct::Codex,
        OfficialProduct::Grok,
        OfficialProduct::Zcode,
    ]
    .into_iter()
    .map(|product| {
        let plan = official_plan(product, OfficialAction::Install);
        let unavailable = plan.blockers.first().cloned();
        OfficialCard {
            product,
            current_version: plan.current_version,
            latest_version: plan.latest_version,
            installed: plan.installed,
            executable_path: plan.executable_path,
            source: plan.source,
            argv: plan.argv,
            dest: plan.dest,
            available: plan.blockers.is_empty(),
            unavailable_reason: unavailable,
        }
    })
    .collect();
    Ok(AboutInfo {
        app: AboutApp {
            name: "Keysmith Switch".into(),
            version: APP_VERSION.into(),
            channel: settings.update_channel,
        },
        adapters,
        official,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn check_app_update(
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<UpdateCheck> {
    let settings = state.store.get_settings()?;
    let req = UpdateRequest {
        channel: channel
            .as_deref()
            .and_then(UpdateChannel::parse)
            .or_else(|| UpdateChannel::parse(&settings.update_channel)),
        settings_channel: UpdateChannel::parse(&settings.update_channel),
        settings_endpoint_override: settings.updater_endpoint_override.clone(),
        endpoint: settings.updater_endpoint_override.clone(),
        ..UpdateRequest::default()
    };
    Ok(check_update(&req))
}

#[tauri::command(rename_all = "camelCase")]
pub fn install_app_update(
    state: State<'_, AppState>,
    confirmed: bool,
    channel: Option<String>,
) -> Result<UpdateInstall> {
    if !confirmed {
        return Ok(UpdateInstall {
            ok: false,
            restart_required: false,
            error: Some("confirmation required".into()),
            release_page: crate::updater::RELEASE_PAGE.into(),
        });
    }
    let settings = state.store.get_settings()?;
    let check = UpdateRequest {
        channel: channel
            .as_deref()
            .and_then(UpdateChannel::parse)
            .or_else(|| UpdateChannel::parse(&settings.update_channel)),
        settings_channel: UpdateChannel::parse(&settings.update_channel),
        settings_endpoint_override: settings.updater_endpoint_override.clone(),
        endpoint: settings.updater_endpoint_override.clone(),
        ..UpdateRequest::default()
    };
    Ok(install_update(&InstallRequest {
        confirmed: true,
        check,
    }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn plan_official_action(product: String, action: String) -> Result<OfficialPlan> {
    let product = parse_official(&product)?;
    let action = parse_official_action(&action)?;
    Ok(official_plan(product, action))
}

#[tauri::command(rename_all = "camelCase")]
pub fn confirm_official_action(plan_id: String, confirmed: bool) -> Result<OfficialResult> {
    Ok(official_confirm(&plan_id, confirmed))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedToolInfo {
    pub kind: String,
    pub name: String,
    pub description: String,
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_advanced_tools(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let enabled = state.store.get_settings()?.advanced_tools_enabled;
    let tools = vec![
        AdvancedToolInfo {
            kind: "scenario".into(),
            name: "Scenario evaluation".into(),
            description: "Read-only Codex scenario list via vendored CLI".into(),
        },
        AdvancedToolInfo {
            kind: "grokRun".into(),
            name: "Grok Run".into(),
            description: "Vendored grok_keysmith_runner.py".into(),
        },
        AdvancedToolInfo {
            kind: "grokBreaktest".into(),
            name: "Grok Breaktest".into(),
            description: "Vendored grok_keysmith_breaktest.py".into(),
        },
    ];
    Ok(serde_json::json!({ "tools": tools, "enabled": enabled }))
}

#[tauri::command(rename_all = "camelCase")]
pub fn run_advanced(
    state: State<'_, AppState>,
    kind: String,
    args: Option<std::collections::BTreeMap<String, String>>,
) -> Result<serde_json::Value> {
    if !state.store.get_settings()?.advanced_tools_enabled {
        return Ok(serde_json::json!({
            "ok": false,
            "kind": kind,
            "output": "",
            "error": "Advanced Tools are hidden until enabled in Settings",
        }));
    }
    let extra = args.unwrap_or_default();
    let input = extra.get("input").cloned();
    let (script, argv) = match kind.as_str() {
        "scenario" => {
            let script = find_vendored_script(ToolKind::Codex)
                .ok_or_else(|| Error::cli_missing("vendored Codex CLI missing"))?;
            (
                script,
                vec!["--scenario-list".to_string(), "--lang".into(), "en".into()],
            )
        }
        "grokRun" => {
            let script = vendored_grok_helper("grok_keysmith_runner.py")?;
            let mut argv = Vec::new();
            if let Some(prompt) = input {
                argv.extend(["--prompt".into(), prompt]);
            } else {
                argv.push("--help".into());
            }
            (script, argv)
        }
        "grokBreaktest" => {
            let script = vendored_grok_helper("grok_keysmith_breaktest.py")?;
            (script, vec!["--help".into()])
        }
        other => {
            return Ok(serde_json::json!({
                "ok": false,
                "kind": other,
                "output": "",
                "error": format!("unknown advanced tool: {other}"),
            }));
        }
    };
    let output = run_python(&script, &argv)?;
    Ok(serde_json::json!({
        "ok": output.status,
        "kind": kind,
        "output": redact_text(&output.stdout),
        "error": if output.status { serde_json::Value::Null } else { serde_json::Value::String(redact_text(&output.stderr)) },
    }))
}

struct ProcOut {
    status: bool,
    stdout: String,
    stderr: String,
}

fn run_python(script: &std::path::Path, argv: &[String]) -> Result<ProcOut> {
    let python = resolve_python()?;
    let mut cmd = Command::new(python);
    cmd.arg(script);
    cmd.args(argv);
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONNOUSERSITE", "1");
    let output = cmd
        .output()
        .map_err(|error| Error::command_failed(error.to_string()))?;
    let stdout = truncate_utf8(&output.stdout);
    let stderr = truncate_utf8(&output.stderr);
    Ok(ProcOut {
        status: output.status.success(),
        stdout,
        stderr,
    })
}

fn truncate_utf8(bytes: &[u8]) -> String {
    const LIMIT: usize = 32 * 1024;
    let slice = if bytes.len() > LIMIT {
        &bytes[..LIMIT]
    } else {
        bytes
    };
    String::from_utf8_lossy(slice).into_owned()
}

fn vendored_grok_helper(name: &str) -> Result<PathBuf> {
    if let Some(main) = find_vendored_script(ToolKind::Grok) {
        if let Some(dir) = main.parent() {
            let helper = dir.join(name);
            if helper.is_file() {
                return Ok(helper);
            }
        }
    }
    Err(Error::cli_missing(format!("vendored {name} missing")))
}

fn parse_official(raw: &str) -> Result<OfficialProduct> {
    match raw {
        "claude" => Ok(OfficialProduct::Claude),
        "codex" => Ok(OfficialProduct::Codex),
        "grok" => Ok(OfficialProduct::Grok),
        "zcode" => Ok(OfficialProduct::Zcode),
        other => Err(Error::invalid(format!("unknown official product: {other}"))),
    }
}

fn parse_official_action(raw: &str) -> Result<OfficialAction> {
    match raw {
        "install" => Ok(OfficialAction::Install),
        "update" => Ok(OfficialAction::Update),
        other => Err(Error::invalid(format!("unknown official action: {other}"))),
    }
}

#[allow(dead_code)]
const _KEEP_DURATION: Duration = Duration::from_secs(1);
