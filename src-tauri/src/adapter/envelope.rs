use serde::{Deserialize, Serialize};

use crate::models::{ScopeInfo, ToolKind, ToolStatus, ADAPTER_SCHEMA};
use crate::redact::redact_text;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TargetPath {
    pub path: String,
    pub role: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    pub path: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
    pub planned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub schema: String,
    pub tool: ToolKind,
    pub command: String,
    pub ok: bool,
    pub preview: bool,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_path: Option<String>,
    pub argv: Vec<String>,
    pub exit_code: i32,
    pub status: ToolStatus,
    pub recovery_required: bool,
    pub scopes: Vec<ScopeInfo>,
    pub target_paths: Vec<TargetPath>,
    pub planned_files: Vec<PlannedFile>,
    pub backups: Vec<BackupEntry>,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_fingerprint: Option<String>,
    pub doctor: DoctorReport,
    pub reload_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reload_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub redacted_stderr: String,
}

impl Envelope {
    pub fn new(tool: ToolKind, command: &str) -> Self {
        Self {
            schema: ADAPTER_SCHEMA.to_string(),
            tool,
            command: command.to_string(),
            ok: true,
            preview: true,
            available: tool.available_on_this_os(),
            unavailable_reason: tool.unavailable_reason().map(str::to_string),
            adapter_version: Some(tool.expected_version().to_string()),
            cli_path: None,
            argv: Vec::new(),
            exit_code: 0,
            status: if tool.available_on_this_os() {
                ToolStatus::Inactive
            } else {
                ToolStatus::Unavailable
            },
            recovery_required: false,
            scopes: default_scopes(tool),
            target_paths: Vec::new(),
            planned_files: Vec::new(),
            backups: Vec::new(),
            conflicts: Vec::new(),
            warnings: Vec::new(),
            blockers: Vec::new(),
            current_fingerprint: None,
            target_fingerprint: None,
            doctor: DoctorReport::default(),
            reload_required: false,
            reload_hint: None,
            error: None,
            redacted_stderr: String::new(),
        }
    }

    pub fn fail(mut self, error: impl Into<String>) -> Self {
        let error = redact_text(&error.into());
        self.ok = false;
        if !self.blockers.iter().any(|item| item == &error) {
            self.blockers.push(error.clone());
        }
        self.error = Some(error);
        if self.exit_code == 0 {
            self.exit_code = 1;
        }
        self
    }

    pub fn redact_inplace(&mut self) {
        self.warnings = self.warnings.iter().map(|item| redact_text(item)).collect();
        self.blockers = self.blockers.iter().map(|item| redact_text(item)).collect();
        self.conflicts = self
            .conflicts
            .iter()
            .map(|item| redact_text(item))
            .collect();
        if let Some(error) = &self.error {
            self.error = Some(redact_text(error));
        }
        self.redacted_stderr = redact_text(&self.redacted_stderr);
        for file in &mut self.planned_files {
            file.detail = redact_text(&file.detail);
        }
        for check in &mut self.doctor.checks {
            if let Some(detail) = &check.detail {
                check.detail = Some(redact_text(detail));
            }
        }
    }
}

pub fn default_scopes(tool: ToolKind) -> Vec<ScopeInfo> {
    use crate::models::Scope;
    if tool.supports_project_scopes() {
        vec![
            ScopeInfo {
                id: Scope::User,
                supported: true,
                reason: None,
            },
            ScopeInfo {
                id: Scope::Project,
                supported: true,
                reason: None,
            },
            ScopeInfo {
                id: Scope::Local,
                supported: true,
                reason: None,
            },
        ]
    } else {
        vec![
            ScopeInfo {
                id: Scope::User,
                supported: true,
                reason: None,
            },
            ScopeInfo {
                id: Scope::Project,
                supported: false,
                reason: Some(format!("{} does not isolate project scope", tool.as_str())),
            },
            ScopeInfo {
                id: Scope::Local,
                supported: false,
                reason: Some(format!("{} does not isolate local scope", tool.as_str())),
            },
        ]
    }
}
