use std::net::IpAddr;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Stdio;

use serde::Serialize;
use serde_json::Value;
#[cfg(windows)]
use tokio::process::Command;

use crate::error::{Error, Result};
use crate::models::RuntimeTarget;
#[cfg(windows)]
use crate::nowindow::NoWindow;

/// Join a path that may belong to a Linux runtime while this process itself runs on Windows.
/// `PathBuf::join` uses the host separator, which would create names containing backslashes in WSL.
pub(crate) fn join_path(base: &Path, child: impl AsRef<Path>) -> PathBuf {
    let base_text = base.to_string_lossy();
    if base_text.starts_with('/') {
        let child_text = child.as_ref().to_string_lossy().replace('\\', "/");
        return PathBuf::from(format!(
            "{}/{}",
            base_text.trim_end_matches('/'),
            child_text.trim_start_matches('/')
        ));
    }
    base.join(child)
}

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
    pub home: Option<String>,
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
        home: dirs::home_dir().map(|path| path.to_string_lossy().into_owned()),
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
        for distribution in parse_verbose_output(&decode_console_output(&output.stdout)) {
            if !distribution.available {
                runtimes.push(distribution);
                continue;
            }
            let RuntimeTarget::Wsl {
                distribution: name, ..
            } = &distribution.target
            else {
                continue;
            };
            match list_users(name).await {
                Ok(users) if !users.is_empty() => {
                    runtimes.extend(users.into_iter().map(|user| RuntimeInfo {
                        target: RuntimeTarget::Wsl {
                            distribution: name.clone(),
                            user: Some(user.name.clone()),
                        },
                        label: if distribution.is_default && user.is_default {
                            format!("WSL2 · {name} · {} (default)", user.name)
                        } else {
                            format!("WSL2 · {name} · {}", user.name)
                        },
                        is_default: distribution.is_default && user.is_default,
                        state: distribution.state.clone(),
                        wsl_version: distribution.wsl_version,
                        available: true,
                        unavailable_reason: None,
                        home: Some(user.home),
                    }));
                }
                _ => runtimes.push(distribution),
            }
        }
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
    let output = wsl_command()
        .args(["--list", "--verbose"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| {
            Error::unavailable(format!("failed to list WSL distributions: {error}"))
        })?;
    let found = parse_verbose_output(&decode_console_output(&output.stdout))
        .into_iter()
        .find(|item| {
            matches!(
                &item.target,
                RuntimeTarget::Wsl { distribution: name, .. } if name == distribution
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

#[cfg(windows)]
pub(crate) async fn validate_target(target: &RuntimeTarget) -> Result<()> {
    let RuntimeTarget::Wsl { distribution, user } = target else {
        return Ok(());
    };
    validate_wsl(distribution).await?;
    let Some(user) = user.as_deref() else {
        return Ok(());
    };
    let status = wsl_command()
        .args([
            "--distribution",
            distribution,
            "--user",
            user,
            "--exec",
            "id",
            "-u",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|error| Error::unavailable(format!("failed to validate WSL user: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::unavailable(format!(
            "WSL user does not exist in {distribution}: {user}"
        )))
    }
}

#[cfg(windows)]
pub(crate) async fn home(target: &RuntimeTarget) -> Result<PathBuf> {
    match target {
        RuntimeTarget::Native => {
            dirs::home_dir().ok_or_else(|| Error::invalid("the Windows home folder is unknown"))
        }
        RuntimeTarget::Wsl { distribution, user } => {
            validate_target(target).await?;
            let mut command = wsl_command();
            command.args(["--distribution", distribution]);
            if let Some(user) = user {
                command.args(["--user", user]);
            }
            let output = command
                .args(["--exec", "sh", "-c", "printf '%s' \"$HOME\""])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|error| {
                    Error::command_failed(format!("failed to resolve WSL home: {error}"))
                })?;
            if !output.status.success() {
                return Err(Error::command_failed(format!(
                    "failed to resolve the home for WSL user {} in {distribution}",
                    user.as_deref().unwrap_or("<default>")
                )));
            }
            let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !home.starts_with('/') || home.contains(['\r', '\n', '\0']) {
                return Err(Error::invalid("WSL returned an invalid Linux home path"));
            }
            Ok(PathBuf::from(home))
        }
    }
}

#[cfg(not(windows))]
pub(crate) async fn validate_target(target: &RuntimeTarget) -> Result<()> {
    match target {
        RuntimeTarget::Native => Ok(()),
        RuntimeTarget::Wsl { distribution, .. } => validate_wsl(distribution).await,
    }
}

#[cfg(not(windows))]
pub(crate) async fn home(target: &RuntimeTarget) -> Result<PathBuf> {
    match target {
        RuntimeTarget::Native => {
            dirs::home_dir().ok_or_else(|| Error::invalid("the home folder is unknown"))
        }
        RuntimeTarget::Wsl { distribution, .. } => Err(Error::unavailable(format!(
            "WSL runtime is only available on Windows: {distribution}"
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
pub(crate) async fn translate_path(target: &RuntimeTarget, path: &Path) -> Result<PathBuf> {
    validate_target(target).await?;
    let RuntimeTarget::Wsl { distribution, user } = target else {
        return Ok(path.to_path_buf());
    };
    let raw = path
        .to_str()
        .ok_or_else(|| Error::invalid("runtime path is not valid Unicode"))?;
    if let Some(path) = translate_unc_path(distribution, raw)? {
        return Ok(path);
    }
    let mut command = wsl_command();
    command.args(["--distribution", distribution]);
    if let Some(user) = user {
        command.args(["--user", user]);
    }
    let output = command
        .args(["--exec", "wslpath", "-a", "-u", raw])
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
pub(crate) async fn translate_path(target: &RuntimeTarget, _path: &Path) -> Result<PathBuf> {
    let distribution = match target {
        RuntimeTarget::Wsl { distribution, .. } => distribution,
        RuntimeTarget::Native => {
            return Err(Error::invalid("native path does not need translation"))
        }
    };
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(windows)]
pub(crate) async fn python_available(target: &RuntimeTarget) -> Result<()> {
    validate_target(target).await?;
    let RuntimeTarget::Wsl { distribution, user } = target else {
        return Ok(());
    };
    let mut command = wsl_command();
    command.args(["--distribution", distribution]);
    if let Some(user) = user {
        command.args(["--user", user]);
    }
    let status = command
        .args(["--exec", "python3", "--version"])
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
pub(crate) async fn ensure_codex_config(
    target: &RuntimeTarget,
    config_dir: Option<&Path>,
) -> Result<()> {
    python_available(target).await?;
    let RuntimeTarget::Wsl { distribution, user } = target else {
        return Ok(());
    };
    let script = "import sys; from pathlib import Path; d=Path(sys.argv[1]) if len(sys.argv)>1 else Path.home()/'.codex'; p=d/'config.toml'; p.touch(exist_ok=True) if d.is_dir() and not p.exists() else None";
    let mut command = wsl_command();
    command.args(["--distribution", distribution]);
    if let Some(user) = user {
        command.args(["--user", user]);
    }
    command.args(["--exec", "python3", "-c", script]);
    if let Some(path) = config_dir {
        command.arg(path);
    }
    let status = command
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
pub(crate) async fn ensure_codex_config(
    target: &RuntimeTarget,
    _config_dir: Option<&Path>,
) -> Result<()> {
    let distribution = match target {
        RuntimeTarget::Wsl { distribution, .. } => distribution,
        RuntimeTarget::Native => return Ok(()),
    };
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(not(windows))]
pub(crate) async fn python_available(target: &RuntimeTarget) -> Result<()> {
    let distribution = match target {
        RuntimeTarget::Wsl { distribution, .. } => distribution,
        RuntimeTarget::Native => return Ok(()),
    };
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
}

#[cfg(windows)]
pub(crate) fn program() -> PathBuf {
    PathBuf::from("wsl.exe")
}

#[cfg(windows)]
fn helper_path() -> Result<PathBuf> {
    let relative = Path::new("runtime").join("runtime-helper.py");
    let mut roots = Vec::new();
    if let Ok(root) = std::env::var("KEYSMITH_SWITCH_VENDOR_ROOT") {
        roots.push(PathBuf::from(root));
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../third_party/keysmith"));
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("third_party/keysmith"));
        roots.push(cwd.join("../third_party/keysmith"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("third_party/keysmith"));
            roots.push(dir.join("../third_party/keysmith"));
            roots.push(dir.join("../Resources/third_party/keysmith"));
        }
    }
    roots
        .into_iter()
        .map(|root| root.join(&relative))
        .find(|path| path.is_file())
        .ok_or_else(|| Error::cli_missing("bundled WSL runtime helper is missing"))
}

#[cfg(windows)]
pub(crate) fn invoke_helper(target: &RuntimeTarget, payload: &Value) -> Result<Value> {
    use std::io::Write;
    use std::process::Command as StdCommand;

    let RuntimeTarget::Wsl { distribution, user } = target else {
        return Err(Error::invalid("the WSL runtime helper needs a WSL target"));
    };
    let helper = helper_path()?;
    let raw = helper
        .to_str()
        .ok_or_else(|| Error::invalid("runtime helper path is not valid Unicode"))?;
    let mut translate = StdCommand::new("wsl.exe");
    translate.no_window();
    translate.args(["--distribution", distribution]);
    if let Some(user) = user {
        translate.args(["--user", user]);
    }
    let translated = translate
        .args(["--exec", "wslpath", "-a", "-u", raw])
        .output()
        .map_err(|error| Error::command_failed(format!("failed to locate WSL helper: {error}")))?;
    if !translated.status.success() {
        return Err(Error::command_failed(
            "failed to translate the WSL helper path",
        ));
    }
    let script = String::from_utf8_lossy(&translated.stdout)
        .trim()
        .to_string();
    let mut command = StdCommand::new("wsl.exe");
    command.no_window();
    command.args(["--distribution", distribution]);
    if let Some(user) = user {
        command.args(["--user", user]);
    }
    let mut child = command
        .args(["--exec", "python3", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::command_failed(format!("failed to start WSL helper: {error}")))?;
    let bytes = serde_json::to_vec(payload)?;
    child
        .stdin
        .take()
        .ok_or_else(|| Error::command_failed("WSL helper stdin is unavailable"))?
        .write_all(&bytes)?;
    let output = child
        .wait_with_output()
        .map_err(|error| Error::command_failed(format!("WSL helper failed: {error}")))?;
    if output.stdout.len() > 4 * 1024 * 1024 || output.stderr.len() > 1024 * 1024 {
        return Err(Error::command_failed("WSL helper output is too large"));
    }
    let response: Value = serde_json::from_slice(&output.stdout).map_err(|_| {
        Error::command_failed(format!(
            "WSL helper returned unreadable output: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    })?;
    if !output.status.success() || response.get("ok") != Some(&Value::Bool(true)) {
        let reason = response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("WSL helper operation failed");
        return Err(Error::command_failed(reason.to_string()));
    }
    Ok(response)
}

pub(crate) fn helper_home(target: &RuntimeTarget) -> Result<PathBuf> {
    let response = invoke_helper(target, &serde_json::json!({ "command": "environment" }))?;
    let home = response
        .get("home")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with('/') && !value.contains(['\0', '\r', '\n']))
        .ok_or_else(|| Error::invalid("WSL returned an invalid Linux home path"))?;
    Ok(PathBuf::from(home))
}

pub(crate) fn relay_address(target: &RuntimeTarget) -> Result<String> {
    let response = invoke_helper(target, &serde_json::json!({ "command": "relayAddress" }))?;
    let raw = response
        .get("address")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::unavailable("WSL did not return a Windows host address"))?;
    let address = raw
        .parse::<IpAddr>()
        .map_err(|_| Error::invalid("WSL returned an invalid Windows host address"))?;
    match address {
        IpAddr::V4(ip)
            if !ip.is_unspecified()
                && !ip.is_multicast()
                && (ip.is_private() || ip.is_loopback() || ip.is_link_local()) =>
        {
            Ok(ip.to_string())
        }
        IpAddr::V4(_) | IpAddr::V6(_) => Err(Error::unavailable(
            "WSL host address is not a private IPv4 interface",
        )),
    }
}

#[cfg(not(windows))]
pub(crate) fn invoke_helper(target: &RuntimeTarget, _payload: &Value) -> Result<Value> {
    let distribution = match target {
        RuntimeTarget::Wsl { distribution, .. } => distribution,
        RuntimeTarget::Native => return Err(Error::invalid("not a WSL target")),
    };
    Err(Error::unavailable(format!(
        "WSL runtime is only available on Windows: {distribution}"
    )))
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
                target: RuntimeTarget::Wsl {
                    distribution,
                    user: None,
                },
                is_default,
                state: state.to_ascii_lowercase(),
                wsl_version: Some(version),
                available,
                unavailable_reason: (!available)
                    .then(|| "Only WSL2 distributions are supported".to_string()),
                home: None,
            })
        })
        .collect()
}

#[cfg(windows)]
#[derive(Debug)]
struct WslUser {
    name: String,
    home: String,
    is_default: bool,
}

#[cfg(windows)]
async fn list_users(distribution: &str) -> Result<Vec<WslUser>> {
    let default_output = wsl_command()
        .args([
            "--distribution",
            distribution,
            "--exec",
            "sh",
            "-c",
            "printf '%s\\t%s\\n' \"$(id -un)\" \"$HOME\"",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|error| Error::command_failed(format!("failed to inspect WSL user: {error}")))?;
    let default_line = String::from_utf8_lossy(&default_output.stdout);
    let (default_user, default_home) = default_line.trim().split_once('\t').unwrap_or(("", ""));

    let output = wsl_command()
        .args([
            "--distribution",
            distribution,
            "--user",
            "root",
            "--exec",
            "sh",
            "-c",
            "getent passwd 2>/dev/null || cat /etc/passwd",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| Error::command_failed(format!("failed to list WSL users: {error}")))?;
    if !output.status.success() {
        return Err(Error::command_failed(format!(
            "failed to list users in WSL distribution {distribution}"
        )));
    }
    let mut users = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 7 {
            continue;
        }
        let Ok(uid) = fields[2].parse::<u32>() else {
            continue;
        };
        let name = fields[0];
        let home = fields[5];
        let shell = fields[6];
        let is_default = name == default_user;
        if !is_default
            && uid != 0
            && (uid < 1000
                || uid == 65_534
                || shell.ends_with("/false")
                || shell.ends_with("/nologin"))
        {
            continue;
        }
        if name.is_empty() || !home.starts_with('/') {
            continue;
        }
        users.push(WslUser {
            name: name.to_string(),
            home: if is_default && !default_home.is_empty() {
                default_home.to_string()
            } else {
                home.to_string()
            },
            is_default,
        });
    }
    users.sort_by_key(|user| (!user.is_default, user.name.clone()));
    Ok(users)
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

    #[test]
    fn runtime_storage_keys_keep_legacy_defaults_and_explicit_users_separate() {
        let legacy = RuntimeTarget::Wsl {
            distribution: "Ubuntu-24.04".into(),
            user: None,
        };
        let explicit = RuntimeTarget::Wsl {
            distribution: "Ubuntu-24.04".into(),
            user: Some("developer".into()),
        };
        assert_eq!(legacy.storage_key(), "wsl:Ubuntu-24.04");
        assert_eq!(
            RuntimeTarget::from_storage_key(&legacy.storage_key()).unwrap(),
            legacy
        );
        assert_eq!(
            RuntimeTarget::from_storage_key(&explicit.storage_key()).unwrap(),
            explicit
        );
        assert_ne!(legacy.storage_key(), explicit.storage_key());
    }
}
