pub mod argv;
pub mod envelope;
pub mod normalize;
pub mod process;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::models::{validate_scope, RuntimeTarget, Scope, ToolInfo, ToolStatus};

pub use crate::models::ToolKind;
pub use envelope::Envelope;

#[derive(Debug, Clone, Default)]
pub struct AdapterOptions {
    pub home: Option<PathBuf>,
    pub extra_env: BTreeMap<String, String>,
    pub timeout: Option<Duration>,
    pub cli_override: Option<PathBuf>,
    pub runtime_target: RuntimeTarget,
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
        /// Claude user scope only. Other tools ignore these.
        #[serde(default)]
        runtime: bool,
        #[serde(default)]
        append_file: Option<PathBuf>,
        #[serde(default)]
        max_tokens: Option<u64>,
    },
    Activate {
        file: PathBuf,
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        runtime: bool,
        #[serde(default)]
        append_file: Option<PathBuf>,
        #[serde(default)]
        max_tokens: Option<u64>,
        /// Grok / pi envelope `confirmation_token`. Unused by the other tools.
        #[serde(default)]
        expected_preview_token: Option<String>,
    },
    /// Claude `backups`. Read-only.
    Backups {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
    },
    /// Claude `restore --target --backup`. Preview until `execute`.
    Restore {
        target: PathBuf,
        backup: String,
        #[serde(default)]
        scope: Option<Scope>,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        execute: bool,
    },
    PlanDeactivate {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
        /// Grok only: uninstall keeping config.toml when its managed backup is lost.
        #[serde(default)]
        salvage_config: bool,
    },
    Deactivate {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        expected_preview_token: Option<String>,
        #[serde(default)]
        salvage_config: bool,
    },
    Doctor,
    Recover {
        scope: Scope,
        #[serde(default)]
        project_dir: Option<PathBuf>,
        #[serde(default)]
        execute: bool,
        #[serde(default)]
        expected_preview_token: Option<String>,
    },
    Version,
    /// Grok `--reconcile`. Preview until `execute`, which passes the envelope token.
    Reconcile {
        #[serde(default)]
        execute: bool,
        #[serde(default)]
        expected_preview_token: Option<String>,
    },
}

impl AdapterCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Status { .. } => "status",
            Self::Backups { .. } => "backups",
            Self::Restore { .. } => "restore",
            Self::PlanActivate { .. } => "plan-activate",
            Self::Activate { .. } => "activate",
            Self::PlanDeactivate { .. } => "plan-deactivate",
            Self::Deactivate { .. } => "deactivate",
            Self::Doctor => "doctor",
            Self::Recover { .. } => "recover",
            Self::Version => "version",
            Self::Reconcile { .. } => "reconcile",
        }
    }

    pub fn is_preview(&self) -> bool {
        match self {
            Self::Activate { .. } | Self::Deactivate { .. } => false,
            Self::Recover { execute, .. }
            | Self::Restore { execute, .. }
            | Self::Reconcile { execute, .. } => !execute,
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

    if matches!(opts.runtime_target, RuntimeTarget::Wsl { .. }) && tool == ToolKind::Zcode {
        envelope.available = false;
        envelope.status = ToolStatus::Unavailable;
        envelope.unavailable_reason =
            Some("ZCode does not provide a supported Linux/WSL runtime".into());
        envelope.error = envelope.unavailable_reason.clone();
        envelope
            .blockers
            .push("Select Windows local for ZCode; its adapter does not support Linux/WSL".into());
        envelope.ok = false;
        return Ok(envelope);
    }

    if let Some((scope, project_dir)) = command_scope(&command) {
        validate_scope(tool, scope, project_dir.as_deref())?;
    }

    let command = prepare_runtime_command(command, &opts.runtime_target).await?;
    let isolated_home = opts
        .runtime_target
        .is_native()
        .then_some(opts.home.as_deref())
        .flatten();
    let prepared = argv::prepare(tool, &command, isolated_home)?;
    envelope.preview = prepared.preview;

    let cli = process::resolve_cli_for_runtime(tool, opts).await?;
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

async fn prepare_runtime_command(
    mut command: AdapterCommand,
    target: &RuntimeTarget,
) -> Result<AdapterCommand> {
    let RuntimeTarget::Wsl { distribution } = target else {
        return Ok(command);
    };

    async fn convert(distribution: &str, path: &mut PathBuf) -> Result<()> {
        *path = crate::runtime::translate_path(distribution, path).await?;
        Ok(())
    }

    match &mut command {
        AdapterCommand::Status { project_dir, .. }
        | AdapterCommand::Backups { project_dir, .. }
        | AdapterCommand::PlanDeactivate { project_dir, .. }
        | AdapterCommand::Deactivate { project_dir, .. }
        | AdapterCommand::Recover { project_dir, .. } => {
            if let Some(path) = project_dir {
                convert(distribution, path).await?;
            }
        }
        AdapterCommand::PlanActivate {
            file,
            project_dir,
            append_file,
            ..
        }
        | AdapterCommand::Activate {
            file,
            project_dir,
            append_file,
            ..
        } => {
            convert(distribution, file).await?;
            if let Some(path) = project_dir {
                convert(distribution, path).await?;
            }
            if let Some(path) = append_file {
                convert(distribution, path).await?;
            }
        }
        AdapterCommand::Restore {
            target,
            project_dir,
            ..
        } => {
            convert(distribution, target).await?;
            if let Some(path) = project_dir {
                convert(distribution, path).await?;
            }
        }
        AdapterCommand::Doctor | AdapterCommand::Version | AdapterCommand::Reconcile { .. } => {}
    }
    Ok(command)
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
        }
        | AdapterCommand::Backups {
            scope, project_dir, ..
        } => Some((*scope, project_dir.clone())),
        AdapterCommand::Restore {
            scope, project_dir, ..
        } => scope.map(|scope| (scope, project_dir.clone())),
        AdapterCommand::Doctor | AdapterCommand::Version | AdapterCommand::Reconcile { .. } => None,
    }
}
