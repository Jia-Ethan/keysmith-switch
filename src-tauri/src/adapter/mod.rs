pub mod argv;
pub mod envelope;
pub mod normalize;
pub mod process;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::models::{validate_scope, Scope, ToolInfo, ToolStatus};

pub use crate::models::ToolKind;
pub use envelope::Envelope;

#[derive(Debug, Clone, Default)]
pub struct AdapterOptions {
    pub home: Option<PathBuf>,
    pub extra_env: BTreeMap<String, String>,
    pub timeout: Option<Duration>,
    pub cli_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AdapterCommand {
    Status {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
    },
    PlanActivate {
        file: PathBuf,
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
    },
    Activate {
        file: PathBuf,
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
    },
    PlanDeactivate {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
    },
    Deactivate {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
    },
    Doctor,
    Recover {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        execute: bool,
    },
    Version,
}

impl AdapterCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Status { .. } => "status",
            Self::PlanActivate { .. } => "plan-activate",
            Self::Activate { .. } => "activate",
            Self::PlanDeactivate { .. } => "plan-deactivate",
            Self::Deactivate { .. } => "deactivate",
            Self::Doctor => "doctor",
            Self::Recover { .. } => "recover",
            Self::Version => "version",
        }
    }

    pub fn is_preview(&self) -> bool {
        match self {
            Self::Activate { .. } | Self::Deactivate { .. } => false,
            Self::Recover { execute, .. } => !execute,
            _ => true,
        }
    }
}

pub fn list_tools() -> Vec<ToolInfo> {
    ToolKind::ALL.iter().copied().map(tool_info).collect()
}

pub fn tool_info(tool: ToolKind) -> ToolInfo {
    ToolInfo {
        id: tool,
        adapter_version: tool.expected_version().to_string(),
        expected_version: tool.expected_version().to_string(),
        scopes: envelope::default_scopes(tool),
        available: tool.available_on_this_os(),
        unavailable_reason: tool.unavailable_reason().map(str::to_string),
        recover_supported: tool.recover_supported(),
    }
}

pub async fn run_adapter(tool: ToolKind, command: AdapterCommand) -> Result<Envelope> {
    run_adapter_with(tool, command, &AdapterOptions::default()).await
}

pub async fn run_adapter_with(
    tool: ToolKind,
    command: AdapterCommand,
    opts: &AdapterOptions,
) -> Result<Envelope> {
    let mut envelope = Envelope::new(tool, command.name());
    envelope.preview = command.is_preview();

    if !tool.available_on_this_os() {
        envelope.available = false;
        envelope.status = ToolStatus::Unavailable;
        envelope.unavailable_reason = tool.unavailable_reason().map(str::to_string);
        envelope.ok = false;
        return Ok(envelope);
    }

    if matches!(tool, ToolKind::Zcode) && matches!(command, AdapterCommand::Recover { .. }) {
        envelope.available = true;
        envelope.status = ToolStatus::Unavailable;
        envelope.unavailable_reason = Some("recover is not supported for ZCode".into());
        envelope.ok = false;
        envelope.error = Some("recover is not supported for ZCode".into());
        envelope
            .blockers
            .push("recover is not supported for ZCode".into());
        return Ok(envelope);
    }

    if let Some((scope, project_dir)) = command_scope(&command) {
        validate_scope(tool, scope, project_dir.as_deref())?;
    }

    let prepared = argv::prepare(tool, &command, opts.home.as_deref())?;
    envelope.preview = prepared.preview;

    let cli = process::resolve_cli(tool, opts)?;
    envelope.cli_path = Some(cli.cli_path());

    let limit = opts
        .timeout
        .unwrap_or(if matches!(command, AdapterCommand::Version) {
            process::VERSION_TIMEOUT
        } else {
            process::DEFAULT_TIMEOUT
        });
    let captured = process::invoke(&cli, &prepared.args, opts, limit).await?;
    let mut envelope = normalize::normalize(tool, &command, &captured, envelope);
    if matches!(command, AdapterCommand::Version) {
        if let Some(actual) = &envelope.adapter_version {
            if !tool.versions_compatible(actual) {
                envelope.warnings.push(format!(
                    "version mismatch: expected {}, got {actual}",
                    tool.expected_version()
                ));
            }
        }
    }
    envelope.redact_inplace();
    Ok(envelope)
}

pub async fn check_adapter_version(tool: ToolKind, opts: &AdapterOptions) -> Result<String> {
    let envelope = run_adapter_with(tool, AdapterCommand::Version, opts).await?;
    let actual = envelope
        .adapter_version
        .unwrap_or_else(|| "unknown".to_string());
    if !tool.versions_compatible(&actual) {
        return Err(Error::version_mismatch(tool.expected_version(), actual));
    }
    Ok(actual)
}

fn command_scope(command: &AdapterCommand) -> Option<(Scope, Option<PathBuf>)> {
    match command {
        AdapterCommand::Status {
            scope, project_dir, ..
        }
        | AdapterCommand::PlanActivate {
            scope, project_dir, ..
        }
        | AdapterCommand::Activate {
            scope, project_dir, ..
        }
        | AdapterCommand::PlanDeactivate {
            scope, project_dir, ..
        }
        | AdapterCommand::Deactivate {
            scope, project_dir, ..
        }
        | AdapterCommand::Recover {
            scope, project_dir, ..
        } => Some((*scope, project_dir.clone())),
        AdapterCommand::Doctor | AdapterCommand::Version => None,
    }
}
