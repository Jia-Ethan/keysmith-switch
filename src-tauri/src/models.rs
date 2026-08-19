use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::error::{Error, Result};

pub const ADAPTER_SCHEMA: &str = "keysmith-switch/adapter-v1";
pub const APP_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Claude,
    Codex,
    Grok,
    Zcode,
}

impl ToolKind {
    pub const ALL: [ToolKind; 4] = [
        ToolKind::Claude,
        ToolKind::Codex,
        ToolKind::Grok,
        ToolKind::Zcode,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::Zcode => "zcode",
        }
    }

    pub fn expected_version(self) -> &'static str {
        match self {
            Self::Claude => "7.1",
            Self::Codex => "0.3.8",
            Self::Grok => "0.4.1",
            Self::Zcode => "0.1.0",
        }
    }

    pub fn env_cli_key(self) -> &'static str {
        match self {
            Self::Claude => "KEYSMITH_SWITCH_CLAUDE_CLI",
            Self::Codex => "KEYSMITH_SWITCH_CODEX_CLI",
            Self::Grok => "KEYSMITH_SWITCH_GROK_CLI",
            Self::Zcode => "KEYSMITH_SWITCH_ZCODE_CLI",
        }
    }

    pub fn vendored_rel(self) -> &'static str {
        match self {
            Self::Claude => "claude/claude-instruct.py",
            Self::Codex => "codex/codex-instruct.py",
            Self::Grok => "grok/grok-keysmith.py",
            Self::Zcode => "zcode/zcode-keysmith.py",
        }
    }

    pub fn default_name(self) -> &'static str {
        match self {
            Self::Claude => "claude-project-rules",
            Self::Codex => "gpt-unrestricted",
            Self::Grok => "99-keysmith",
            Self::Zcode => "system-role",
        }
    }

    pub fn supports_project_scopes(self) -> bool {
        matches!(self, Self::Claude)
    }

    pub fn recover_supported(self) -> bool {
        !matches!(self, Self::Zcode)
    }

    pub fn available_on_this_os(self) -> bool {
        if matches!(self, Self::Zcode) && cfg!(windows) {
            return false;
        }
        true
    }

    pub fn unavailable_reason(self) -> Option<&'static str> {
        if matches!(self, Self::Zcode) && cfg!(windows) {
            return Some("ZCode Keysmith is not available on Windows.");
        }
        None
    }

    pub fn versions_compatible(self, actual: &str) -> bool {
        versions_compatible(self.expected_version(), actual)
    }
}

impl fmt::Display for ToolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ToolKind {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "grok" => Ok(Self::Grok),
            "zcode" => Ok(Self::Zcode),
            other => Err(Error::invalid(format!("unknown tool: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Project,
    Local,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }

    pub fn requires_project_dir(self) -> bool {
        matches!(self, Self::Project | Self::Local)
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Scope {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "user" => Ok(Self::User),
            "project" => Ok(Self::Project),
            "local" => Ok(Self::Local),
            other => Err(Error::invalid(format!("unknown scope: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolStatus {
    NotInstalled,
    Inactive,
    Active,
    Drift,
    Conflict,
    RecoveryRequired,
    Unavailable,
}

impl ToolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not-installed",
            Self::Inactive => "inactive",
            Self::Active => "active",
            Self::Drift => "drift",
            Self::Conflict => "conflict",
            Self::RecoveryRequired => "recovery-required",
            Self::Unavailable => "unavailable",
        }
    }
}

impl fmt::Display for ToolStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ToolStatus {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "not-installed" | "not_installed" => Ok(Self::NotInstalled),
            "inactive" | "inactive-by-config" => Ok(Self::Inactive),
            "active" | "active-aligned" => Ok(Self::Active),
            "drift" => Ok(Self::Drift),
            "conflict" => Ok(Self::Conflict),
            "recovery" | "recovery-required" => Ok(Self::RecoveryRequired),
            "unavailable" => Ok(Self::Unavailable),
            other => Err(Error::invalid(format!("unknown status: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationKind {
    PlanActivate,
    Activate,
    PlanDeactivate,
    Deactivate,
    Recover,
    Doctor,
    Version,
    Status,
}

impl OperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlanActivate => "plan-activate",
            Self::Activate => "activate",
            Self::PlanDeactivate => "plan-deactivate",
            Self::Deactivate => "deactivate",
            Self::Recover => "recover",
            Self::Doctor => "doctor",
            Self::Version => "version",
            Self::Status => "status",
        }
    }
}

impl FromStr for OperationKind {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim() {
            "plan-activate" => Ok(Self::PlanActivate),
            "activate" => Ok(Self::Activate),
            "plan-deactivate" => Ok(Self::PlanDeactivate),
            "deactivate" => Ok(Self::Deactivate),
            "recover" => Ok(Self::Recover),
            "doctor" => Ok(Self::Doctor),
            "version" => Ok(Self::Version),
            "status" => Ok(Self::Status),
            other => Err(Error::invalid(format!("unknown operation: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationStatus {
    Preview,
    Ready,
    Succeeded,
    Failed,
    Cancelled,
}

impl OperationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Ready => "ready",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl FromStr for OperationStatus {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim() {
            "preview" => Ok(Self::Preview),
            "ready" => Ok(Self::Ready),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(Error::invalid(format!("unknown operation status: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromptSort {
    Updated,
    Created,
    Title,
}

impl Default for PromptSort {
    fn default() -> Self {
        Self::Updated
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScopeInfo {
    pub id: Scope,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: ToolKind,
    pub adapter_version: String,
    pub expected_version: String,
    pub scopes: Vec<ScopeInfo>,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub recover_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptSummary {
    pub id: String,
    pub tool: ToolKind,
    pub title: String,
    pub tags: Vec<String>,
    pub version: i64,
    pub sha256: String,
    pub updated_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptDetail {
    pub id: String,
    pub tool: ToolKind,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub version: i64,
    pub sha256: String,
    pub path: PathBuf,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptVersion {
    pub prompt_id: String,
    pub version: i64,
    pub title: String,
    pub tags: Vec<String>,
    pub content: String,
    pub sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Activation {
    pub id: String,
    pub prompt_id: Option<String>,
    pub tool: ToolKind,
    pub scope: Scope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    pub status: ToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub tool: ToolKind,
    pub kind: OperationKind,
    pub status: OperationStatus,
    pub preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    pub request_json: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub language: String,
    pub update_channel: String,
    pub advanced_tools_enabled: bool,
    pub default_claude_scope: Scope,
    pub recent_project_dirs: Vec<String>,
    pub updater_endpoint_override: Option<String>,
    pub close_to_tray: bool,
    pub auto_launch: bool,
    pub silent_start: bool,
    pub auto_check_updates: bool,
    pub theme: String,
    pub first_run_completed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "zh-CN".to_string(),
            update_channel: "stable".to_string(),
            advanced_tools_enabled: false,
            default_claude_scope: Scope::User,
            recent_project_dirs: Vec::new(),
            updater_endpoint_override: None,
            close_to_tray: true,
            auto_launch: false,
            silent_start: false,
            auto_check_updates: true,
            theme: "system".to_string(),
            first_run_completed: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub language: Option<String>,
    pub update_channel: Option<String>,
    pub advanced_tools_enabled: Option<bool>,
    pub default_claude_scope: Option<Scope>,
    pub recent_project_dirs: Option<Vec<String>>,
    pub updater_endpoint_override: Option<Option<String>>,
    pub close_to_tray: Option<bool>,
    pub auto_launch: Option<bool>,
    pub silent_start: Option<bool>,
    pub auto_check_updates: Option<bool>,
    pub theme: Option<String>,
    pub first_run_completed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreatePromptInput {
    pub tool: ToolKind,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePromptInput {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanActivateInput {
    pub prompt_id: String,
    pub scope: Scope,
    pub project_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanDeactivateInput {
    pub prompt_id: Option<String>,
    pub tool: ToolKind,
    pub scope: Scope,
    pub project_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    pub scanned: usize,
    pub imported: usize,
    pub updated: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub unified: String,
    pub summary: crate::diff::ChangeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub operation_id: String,
    pub envelope: crate::adapter::Envelope,
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes.as_ref());
    hex::encode(digest)
}

pub fn normalize_version(raw: &str) -> String {
    raw.trim()
        .trim_start_matches(|c: char| c == 'v' || c == 'V')
        .split_whitespace()
        .last()
        .unwrap_or(raw)
        .trim_start_matches(|c: char| c == 'v' || c == 'V')
        .to_string()
}

pub fn versions_compatible(expected: &str, actual: &str) -> bool {
    let expected = normalize_version(expected);
    let actual = normalize_version(actual);
    expected == actual || actual.ends_with(&expected) || expected.ends_with(&actual)
}

pub fn validate_scope(tool: ToolKind, scope: Scope, project_dir: Option<&Path>) -> Result<()> {
    if !tool.available_on_this_os() {
        return Err(Error::unavailable(
            tool.unavailable_reason()
                .unwrap_or("tool is unavailable on this platform"),
        ));
    }
    if !tool.supports_project_scopes() && scope != Scope::User {
        return Err(Error::invalid(format!(
            "{} only supports user scope",
            tool.as_str()
        )));
    }
    if scope.requires_project_dir() {
        let dir = project_dir
            .ok_or_else(|| Error::invalid("project/local scope requires an absolute projectDir"))?;
        if !dir.is_absolute() {
            return Err(Error::invalid(
                "project/local scope requires an absolute projectDir",
            ));
        }
    }
    Ok(())
}

pub fn sanitize_cli_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
        } else if ch == ' ' {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('.').trim_matches('-');
    if trimmed.is_empty() {
        "prompt".to_string()
    } else {
        trimmed.to_string()
    }
}
