use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Stdio;

use serde::Serialize;
#[cfg(windows)]
use tokio::process::Command;

use crate::error::{Error, Result};
use crate::models::RuntimeTarget;
#[cfg(windows)]
use crate::nowindow::NoWindow;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub target: RuntimeTarget,
    pub label: String,
    pub is_default: bool,
    pub state: String,
    pub wsl_version: Option<u8>,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

pub async fn list() -> Result<Vec<RuntimeInfo>> {
    #[allow(unused_mut)]
    let mut runtimes = vec![RuntimeInfo {
        target: RuntimeTarget::Native,
        label: native_label().to_string(),
        is_default: true,
        state: "running".to_string(),
        wsl_version: None,
        available: true,
        unavailable_reason: None,
    }];
    #[cfg(windows)]
    {
        let output = match wsl_command()
            .args(["--list", "--verbose"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
        {
            Ok(output) => output,
            Err(_) => return Ok(runtimes),
        };
        if !output.status.success() {
            return Ok(runtimes);
        }
        runtimes.extend(parse_verbose_output(&decode_console_output(&output.stdout)));
    }
    Ok(runtimes)
}

#[cfg(not(windows))]
fn native_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS local"
    } else {
        "Local"
    }
}

#[cfg(windows)]
fn native_label() -> &'static str {
    "Windows local"
}

#[cfg(windows)]
fn wsl_command() -> Command {
    let mut command = Command::new("wsl.exe");
    command.no_window();
    command
}

#[cfg(windows)]
pub(crate) async fn validate_wsl(distribution: &str) -> Result<()> {
    let found = list().await?.into_iter().find(|item| {
        matches!(
            &item.target,
            RuntimeTarget::Wsl { distribution: name } if name == distribution
        )
    });
    match found {
        Some(info) if info.available => Ok(()),
        Some(info) => Err(Error::unavailable(info.unavailable_reason.unwrap_or_else(
            || format!("WSL distribution {distribution} is unavailable"),
        ))),
        None => Err(Error::unavailable(format!(
            "WSL distribution is not installed: {distribution}"
        ))),
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub(crate) async fn validate_wsl(distribution: &str) -> Result<()> {
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(windows)]
pub(crate) async fn translate_path(distribution: &str, path: &Path) -> Result<PathBuf> {
    validate_wsl(distribution).await?;
    let raw = path
        .to_str()
        .ok_or_else(|| Error::invalid("runtime path is not valid Unicode"))?;
    if let Some(path) = translate_unc_path(distribution, raw)? {
        return Ok(path);
    }
    let output = wsl_command()
        .args([
            "--distribution",
            distribution,
            "--exec",
            "wslpath",
            "-a",
            "-u",
            raw,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| Error::command_failed(format!("failed to translate WSL path: {error}")))?;
    if !output.status.success() {
        return Err(Error::invalid(format!(
            "path is not accessible in WSL distribution {distribution}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let translated = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !translated.starts_with('/') {
        return Err(Error::invalid(
            "wslpath did not return a Linux absolute path",
        ));
    }
    Ok(PathBuf::from(translated))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn translate_unc_path(distribution: &str, raw: &str) -> Result<Option<PathBuf>> {
    let normalized = raw.replace('/', "\\");
    let lower = normalized.to_ascii_lowercase();
    let prefix = if lower.starts_with(r"\\wsl.localhost\") {
        r"\\wsl.localhost\"
    } else if lower.starts_with(r"\\wsl$\") {
        r"\\wsl$\"
    } else {
        return Ok(None);
    };
    let remainder = &normalized[prefix.len()..];
    let mut parts = remainder.split('\\').filter(|part| !part.is_empty());
    let selected = parts
        .next()
        .ok_or_else(|| Error::invalid("WSL UNC path is missing a distribution"))?;
    if !selected.eq_ignore_ascii_case(distribution) {
        return Err(Error::invalid(format!(
            "project path belongs to WSL distribution {selected}, not {distribution}"
        )));
    }
    let mut linux_parts = Vec::new();
    for part in parts {
        if matches!(part, "." | "..") {
            return Err(Error::invalid(
                "WSL UNC path cannot contain . or .. segments",
            ));
        }
        linux_parts.push(part);
    }
    Ok(Some(PathBuf::from(format!("/{}", linux_parts.join("/")))))
}

#[cfg(not(windows))]
pub(crate) async fn translate_path(distribution: &str, _path: &Path) -> Result<PathBuf> {
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(windows)]
pub(crate) async fn python_available(distribution: &str) -> Result<()> {
    validate_wsl(distribution).await?;
    let status = wsl_command()
        .args([
            "--distribution",
            distribution,
            "--exec",
            "python3",
            "--version",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| Error::cli_missing(format!("failed to start WSL Python: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::cli_missing(format!(
            "python3 is required in WSL distribution {distribution}"
        )))
    }
}

#[cfg(windows)]
pub(crate) async fn ensure_codex_config(distribution: &str) -> Result<()> {
    python_available(distribution).await?;
    let script = "from pathlib import Path; d=Path.home()/'.codex'; p=d/'config.toml'; p.touch(exist_ok=True) if d.is_dir() and not p.exists() else None";
    let status = wsl_command()
        .args([
            "--distribution",
            distribution,
            "--exec",
            "python3",
            "-c",
            script,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| {
            Error::command_failed(format!("failed to prepare Codex config in WSL: {error}"))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::command_failed(format!(
            "failed to prepare Codex config in WSL distribution {distribution}"
        )))
    }
}

#[cfg(not(windows))]
pub(crate) async fn ensure_codex_config(distribution: &str) -> Result<()> {
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(not(windows))]
pub(crate) async fn python_available(distribution: &str) -> Result<()> {
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(windows)]
pub(crate) fn program() -> PathBuf {
    PathBuf::from("wsl.exe")
}

#[cfg(not(windows))]
pub(crate) fn program() -> PathBuf {
    PathBuf::from("wsl.exe")
}

#[cfg_attr(not(windows), allow(dead_code))]
fn decode_console_output(bytes: &[u8]) -> String {
    if bytes.chunks_exact(2).take(64).any(|pair| pair[1] == 0) {
        let words: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&words)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn parse_verbose_output(raw: &str) -> Vec<RuntimeInfo> {
    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim_matches(|ch: char| ch.is_whitespace() || ch == '\0');
            let is_default = trimmed.starts_with('*');
            let row = trimmed.trim_start_matches('*').trim();
            let mut fields: Vec<&str> = row.split_whitespace().collect();
            let version = fields.pop()?.parse::<u8>().ok()?;
            let state = fields.pop()?.to_string();
            let distribution = fields.join(" ");
            if distribution.is_empty()
                || distribution
                    .to_ascii_lowercase()
                    .starts_with("docker-desktop")
            {
                return None;
            }
            let available = version == 2;
            Some(RuntimeInfo {
                label: if is_default {
                    format!("WSL{version} · {distribution} (default)")
                } else {
                    format!("WSL{version} · {distribution}")
                },
                target: RuntimeTarget::Wsl { distribution },
                is_default,
                state: state.to_ascii_lowercase(),
                wsl_version: Some(version),
                available,
                unavailable_reason: (!available)
                    .then(|| "Only WSL2 distributions are supported".to_string()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verbose_rows_and_filters_internal_distributions() {
        let parsed = parse_verbose_output(
            "  NAME                   STATE           VERSION\n* Ubuntu-24.04           Running         2\n  Debian Dev             Stopped         2\n  docker-desktop         Running         2\n  Legacy                 Stopped         1\n",
        );
        assert_eq!(parsed.len(), 3);
        assert!(parsed[0].is_default);
        assert_eq!(parsed[1].state, "stopped");
        assert!(!parsed[2].available);
    }

    #[test]
    fn decodes_redirected_utf16_output() {
        let bytes: Vec<u8> = "* Ubuntu Running 2\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert!(decode_console_output(&bytes).contains("Ubuntu"));
    }

    #[test]
    fn converts_only_unc_paths_for_the_selected_distribution() {
        assert_eq!(
            translate_unc_path("Ubuntu", r"\\wsl.localhost\Ubuntu\home\sam\project").unwrap(),
            Some(PathBuf::from("/home/sam/project"))
        );
        assert!(translate_unc_path("Debian", r"\\wsl$\Ubuntu\home\sam").is_err());
        assert!(translate_unc_path("Ubuntu", r"\\wsl$\Ubuntu\home\..\root").is_err());
    }
}
