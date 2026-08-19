use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::models::{validate_scope, Scope, ToolKind};

use super::AdapterCommand;

#[derive(Debug, Clone)]
pub struct PreparedCommand {
    pub args: Vec<String>,
    pub preview: bool,
}

pub fn prepare(
    tool: ToolKind,
    command: &AdapterCommand,
    isolated_home: Option<&Path>,
) -> Result<PreparedCommand> {
    match tool {
        ToolKind::Claude => claude_args(command),
        ToolKind::Codex => codex_args(command, isolated_home),
        ToolKind::Grok => grok_args(command, isolated_home),
        ToolKind::Zcode => zcode_args(command, isolated_home),
    }
}

fn claude_args(command: &AdapterCommand) -> Result<PreparedCommand> {
    match command {
        AdapterCommand::Version => Ok(PreparedCommand {
            args: vec!["--version".into()],
            preview: true,
        }),
        AdapterCommand::Doctor => Ok(PreparedCommand {
            args: vec!["doctor".into(), "--json".into()],
            preview: true,
        }),
        AdapterCommand::Status {
            scope,
            project_dir,
            name,
        } => {
            let mut args = vec!["status".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            push_name(&mut args, name.as_deref());
            if *scope == Scope::User {
                args.push("--runtime".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::PlanActivate {
            file,
            scope,
            project_dir,
            name,
        }
        | AdapterCommand::Activate {
            file,
            scope,
            project_dir,
            name,
        } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let preview = matches!(command, AdapterCommand::PlanActivate { .. });
            let mut args = vec![
                "install".into(),
                "--scope".into(),
                scope.as_str().into(),
                "--file".into(),
                abs(file)?,
            ];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            push_name(&mut args, name.as_deref());
            if !preview {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand { args, preview })
        }
        AdapterCommand::PlanDeactivate {
            scope,
            project_dir,
            name,
        }
        | AdapterCommand::Deactivate {
            scope,
            project_dir,
            name,
        } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let preview = matches!(command, AdapterCommand::PlanDeactivate { .. });
            let mut args = vec!["uninstall".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            push_name(&mut args, name.as_deref());
            if !preview {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand { args, preview })
        }
        AdapterCommand::Recover {
            scope,
            project_dir,
            execute,
        } => {
            validate_scope(ToolKind::Claude, *scope, project_dir.as_deref())?;
            let mut args = vec!["recover".into(), "--scope".into(), scope.as_str().into()];
            push_project_dir(&mut args, *scope, project_dir.as_deref())?;
            if *execute {
                args.push("--yes".into());
            }
            args.push("--json".into());
            Ok(PreparedCommand {
                args,
                preview: !*execute,
            })
        }
    }
}

fn codex_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let mut args = Vec::new();
    let mut preview = true;
    match command {
        AdapterCommand::Version => args.push("--version".into()),
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => args.push("--status".into()),
        AdapterCommand::PlanActivate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--dry-run".into());
        }
        AdapterCommand::Activate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::PlanDeactivate { .. } => args.push("--uninstall".into()),
        AdapterCommand::Deactivate { .. } => {
            args.push("--uninstall".into());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::Recover { execute, .. } => {
            args.push("--recover".into());
            if *execute {
                args.push("--yes".into());
                preview = false;
            }
        }
    }
    if !matches!(command, AdapterCommand::Version) {
        args.push("--lang".into());
        args.push("en".into());
        if let Some(dir) = tool_dir(home, ".codex") {
            args.push("--codex-dir".into());
            args.push(dir);
        }
    }
    Ok(PreparedCommand { args, preview })
}

fn grok_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let mut args = vec!["--json".into()];
    let mut preview = true;
    match command {
        AdapterCommand::Version => args.push("--version".into()),
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => args.push("--status".into()),
        AdapterCommand::PlanActivate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--dry-run".into());
        }
        AdapterCommand::Activate { file, name, .. } => {
            args.push("--file".into());
            args.push(abs(file)?);
            push_flag_name(&mut args, name.as_deref());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::PlanDeactivate { .. } => args.push("--uninstall".into()),
        AdapterCommand::Deactivate { .. } => {
            args.push("--uninstall".into());
            args.push("--yes".into());
            preview = false;
        }
        AdapterCommand::Recover { execute, .. } => {
            args.push("--recover".into());
            if *execute {
                args.push("--yes".into());
                preview = false;
            }
        }
    }
    if let Some(dir) = tool_dir(home, ".grok") {
        args.push("--grok-dir".into());
        args.push(dir);
    }
    Ok(PreparedCommand { args, preview })
}

fn zcode_args(command: &AdapterCommand, home: Option<&Path>) -> Result<PreparedCommand> {
    let managed = tool_dir(home, ".zcode-keysmith");
    match command {
        AdapterCommand::Version => Ok(PreparedCommand {
            args: vec!["--version".into()],
            preview: true,
        }),
        AdapterCommand::Doctor | AdapterCommand::Status { .. } => {
            let mut args = vec!["doctor".into()];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::PlanActivate { file, .. } => {
            let mut args = vec![
                "install".into(),
                "--system-file".into(),
                abs(file)?,
                "--dry-run".into(),
            ];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::Activate { file, .. } => {
            let mut args = vec![
                "install".into(),
                "--system-file".into(),
                abs(file)?,
                "--yes".into(),
            ];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: false,
            })
        }
        AdapterCommand::PlanDeactivate { .. } => {
            let mut args = vec!["uninstall".into(), "--dry-run".into()];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: true,
            })
        }
        AdapterCommand::Deactivate { .. } => {
            let mut args = vec!["uninstall".into(), "--yes".into()];
            if let Some(dir) = managed {
                args.push("--managed-dir".into());
                args.push(dir);
            }
            Ok(PreparedCommand {
                args,
                preview: false,
            })
        }
        AdapterCommand::Recover { .. } => {
            Err(Error::unavailable("recover is not supported for ZCode"))
        }
    }
}

fn push_project_dir(
    args: &mut Vec<String>,
    scope: Scope,
    project_dir: Option<&Path>,
) -> Result<()> {
    if scope.requires_project_dir() {
        let dir = project_dir
            .ok_or_else(|| Error::invalid("project/local scope requires an absolute projectDir"))?;
        if !dir.is_absolute() {
            return Err(Error::invalid(
                "project/local scope requires an absolute projectDir",
            ));
        }
        args.push("--project-dir".into());
        args.push(dir.to_string_lossy().into_owned());
    }
    Ok(())
}

fn push_name(args: &mut Vec<String>, name: Option<&str>) {
    if let Some(name) = name {
        if !name.is_empty() {
            args.push("--name".into());
            args.push(name.to_string());
        }
    }
}

fn push_flag_name(args: &mut Vec<String>, name: Option<&str>) {
    if let Some(name) = name {
        if !name.is_empty() {
            args.push("--name".into());
            args.push(name.to_string());
        }
    }
}

fn abs(path: &Path) -> Result<String> {
    if path.is_absolute() {
        return Ok(path.to_string_lossy().into_owned());
    }
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(path).to_string_lossy().into_owned())
}

fn tool_dir(home: Option<&Path>, name: &str) -> Option<String> {
    home.map(|home| {
        let path = if home.is_absolute() {
            home.join(name)
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(home)
                .join(name)
        };
        path.to_string_lossy().into_owned()
    })
}
