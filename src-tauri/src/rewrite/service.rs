//! Running the relay in the background, independent of this app.
//!
//! The relay binary is copied out of the app bundle into the data folder first, so moving,
//! updating or removing the app never pulls it from under Codex (and on Windows, so the
//! installer can replace the app's files while the relay runs). It then starts at sign-in:
//! a LaunchAgent on macOS, a `Run` registry value on Windows.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::adapter::process::find_bundled_binary;
use crate::error::{Error, Result};
use crate::nowindow::NoWindow;
use crate::paths::{atomic_write, AppPaths};

pub const RELAY_CONFIG_SCHEMA: u32 = 1;
const BINARY: &str = "keysmith-relay";

/// Must match `keysmith_relay::RelayConfig`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayConfig {
    pub schema: u32,
    pub port: u16,
    pub token: String,
    pub upstreams: std::collections::BTreeMap<String, String>,
    /// `<tool>/<upstream id>` → base URL, for agents other than Codex.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub routes: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub installed: bool,
    pub running: bool,
}

fn dir(paths: &AppPaths) -> PathBuf {
    super::rewrite_dir(paths)
}

fn config_path(paths: &AppPaths) -> PathBuf {
    dir(paths).join("relay.json")
}

pub fn installed_binary(paths: &AppPaths) -> PathBuf {
    let name = if cfg!(windows) {
        format!("{BINARY}.exe")
    } else {
        BINARY.to_string()
    };
    dir(paths).join("bin").join(name)
}

fn log_path(paths: &AppPaths) -> PathBuf {
    paths.logs.join("relay.log")
}

pub fn read_config(paths: &AppPaths) -> Option<RelayConfig> {
    std::fs::read(config_path(paths))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RelayConfig>(&bytes).ok())
        .filter(|config| config.schema == RELAY_CONFIG_SCHEMA && !config.token.is_empty())
}

/// The existing config, or a new one on a free loopback port with a fresh token.
pub fn ensure_config(paths: &AppPaths) -> Result<RelayConfig> {
    if let Some(config) = read_config(paths) {
        return Ok(config);
    }
    let port = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?
        .local_addr()?
        .port();
    let config = RelayConfig {
        schema: RELAY_CONFIG_SCHEMA,
        port,
        token: uuid::Uuid::new_v4().simple().to_string(),
        upstreams: Default::default(),
        routes: Default::default(),
    };
    write_config(paths, &config)?;
    Ok(config)
}

pub fn write_config(paths: &AppPaths, config: &RelayConfig) -> Result<()> {
    atomic_write(&config_path(paths), &serde_json::to_string_pretty(config)?)
}

pub fn base_url(config: &RelayConfig, provider: &str) -> String {
    format!(
        "http://127.0.0.1:{}/r/{}/{provider}",
        config.port, config.token
    )
}

/// The relay address for one agent's upstream: `/t/<token>/<tool>/<upstream>`.
pub fn route_url(config: &RelayConfig, tool: &str, upstream: &str) -> String {
    format!(
        "http://127.0.0.1:{}/t/{}/{tool}/{upstream}",
        config.port, config.token
    )
}

/// True when the relay answers on its port within a short wait.
pub fn healthy(config: &RelayConfig) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut head = [0u8; 16];
    stream
        .read(&mut head)
        .is_ok_and(|n| head[..n].starts_with(b"HTTP/1.1 200"))
}

pub fn wait_healthy(config: &RelayConfig) -> bool {
    for _ in 0..20 {
        if healthy(config) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

/// Copy the bundled relay into the data folder when it is missing or differs.
/// Returns true when the copy changed.
fn place_binary(paths: &AppPaths) -> Result<bool> {
    let source = find_bundled_binary(BINARY).ok_or_else(|| {
        Error::cli_missing("bundled keysmith-relay is missing; reinstall Keysmith Switch")
    })?;
    let dest = installed_binary(paths);
    let fresh = std::fs::read(&source)?;
    if std::fs::read(&dest).is_ok_and(|current| current == fresh) {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("new");
    std::fs::write(&tmp, &fresh)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(true)
}

pub fn status(paths: &AppPaths) -> ServiceStatus {
    ServiceStatus {
        installed: platform::registered(paths),
        running: read_config(paths).is_some_and(|config| healthy(&config)),
    }
}

/// Copy the relay, register it to start at sign-in, and start it now.
pub fn install(paths: &AppPaths) -> Result<()> {
    let config = ensure_config(paths)?;
    if platform::registered(paths) {
        platform::stop(paths);
    }
    place_binary(paths)?;
    platform::register(paths)?;
    platform::start(paths)?;
    if !wait_healthy(&config) {
        return Err(Error::unavailable(format!(
            "keysmith-relay did not start on port {}; see {}",
            config.port,
            log_path(paths).display()
        )));
    }
    Ok(())
}

pub fn uninstall(paths: &AppPaths) -> Result<()> {
    platform::stop(paths);
    platform::unregister(paths)?;
    let _ = std::fs::remove_file(installed_binary(paths));
    Ok(())
}

/// At app start: bring an installed relay up to the bundled version and make sure it runs.
pub fn refresh(paths: &AppPaths) -> Result<()> {
    if !platform::registered(paths) {
        return Ok(());
    }
    let Some(config) = read_config(paths) else {
        return Ok(());
    };
    let differs = find_bundled_binary(BINARY)
        .and_then(|source| std::fs::read(source).ok())
        .is_some_and(|fresh| std::fs::read(installed_binary(paths)).ok() != Some(fresh));
    if differs {
        platform::stop(paths);
        place_binary(paths)?;
        platform::start(paths)?;
    } else if !healthy(&config) {
        platform::start(paths)?;
    }
    Ok(())
}

fn run(command: &mut Command) -> Result<std::process::Output> {
    command
        .no_window()
        .output()
        .map_err(|error| Error::command_failed(error.to_string()))
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    const LABEL: &str = "com.jia-ethan.keysmith-switch.relay";

    fn agents_dir() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("KEYSMITH_SWITCH_LAUNCH_AGENTS_DIR") {
            return Some(PathBuf::from(dir));
        }
        Some(dirs::home_dir()?.join("Library/LaunchAgents"))
    }

    fn label() -> String {
        match std::env::var("KEYSMITH_SWITCH_RELAY_LABEL") {
            Ok(label) if !label.trim().is_empty() => label.trim().to_string(),
            _ => LABEL.to_string(),
        }
    }

    fn plist_path() -> Option<PathBuf> {
        Some(agents_dir()?.join(format!("{}.plist", label())))
    }

    fn domain() -> String {
        format!("gui/{}", unsafe { libc::getuid() })
    }

    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    pub fn registered(_paths: &AppPaths) -> bool {
        plist_path().is_some_and(|path| path.is_file())
    }

    pub fn register(paths: &AppPaths) -> Result<()> {
        let path = plist_path().ok_or_else(|| Error::message("cannot resolve LaunchAgents"))?;
        let binary = escape(&installed_binary(paths).to_string_lossy());
        let home = escape(&paths.home.to_string_lossy());
        let log = escape(&log_path(paths).to_string_lossy());
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{label}</string>
  <key>ProgramArguments</key>
  <array><string>{binary}</string><string>--home</string><string>{home}</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>ProcessType</key><string>Background</string>
  <key>StandardErrorPath</key><string>{log}</string>
  <key>StandardOutPath</key><string>{log}</string>
</dict>
</plist>
"#,
            label = escape(&label()),
        );
        atomic_write(&path, &plist)
    }

    pub fn start(_paths: &AppPaths) -> Result<()> {
        let path = plist_path().ok_or_else(|| Error::message("cannot resolve LaunchAgents"))?;
        // Already loaded: kickstart restarts it; not loaded: bootstrap loads and runs it.
        let kick = run(Command::new("launchctl").args([
            "kickstart",
            "-k",
            &format!("{}/{}", domain(), label()),
        ]))?;
        if kick.status.success() {
            return Ok(());
        }
        let boot = run(Command::new("launchctl")
            .arg("bootstrap")
            .arg(domain())
            .arg(&path))?;
        if !boot.status.success() {
            return Err(Error::command_failed(format!(
                "launchctl bootstrap failed: {}",
                String::from_utf8_lossy(&boot.stderr).trim()
            )));
        }
        Ok(())
    }

    pub fn stop(_paths: &AppPaths) {
        let _ =
            run(Command::new("launchctl").args(["bootout", &format!("{}/{}", domain(), label())]));
    }

    pub fn unregister(_paths: &AppPaths) -> Result<()> {
        if let Some(path) = plist_path() {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod platform {
    use super::*;

    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "KeysmithSwitchRelay";

    fn command_line(paths: &AppPaths) -> String {
        format!(
            "\"{}\" --home \"{}\"",
            installed_binary(paths).display(),
            paths.home.display()
        )
    }

    pub fn registered(_paths: &AppPaths) -> bool {
        run(Command::new("reg").args(["query", RUN_KEY, "/v", VALUE]))
            .is_ok_and(|out| out.status.success())
    }

    pub fn register(paths: &AppPaths) -> Result<()> {
        let out = run(Command::new("reg").args([
            "add",
            RUN_KEY,
            "/v",
            VALUE,
            "/t",
            "REG_SZ",
            "/d",
            &command_line(paths),
            "/f",
        ]))?;
        if !out.status.success() {
            return Err(Error::command_failed(
                "could not register keysmith-relay at sign-in",
            ));
        }
        Ok(())
    }

    pub fn start(paths: &AppPaths) -> Result<()> {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path(paths))?;
        Command::new(installed_binary(paths))
            .arg("--home")
            .arg(&paths.home)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(log)
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .map_err(|error| Error::command_failed(error.to_string()))?;
        Ok(())
    }

    pub fn stop(_paths: &AppPaths) {
        let _ = run(Command::new("taskkill").args(["/IM", "keysmith-relay.exe", "/F"]));
        std::thread::sleep(Duration::from_millis(300));
    }

    pub fn unregister(_paths: &AppPaths) -> Result<()> {
        if registered(_paths) {
            run(Command::new("reg").args(["delete", RUN_KEY, "/v", VALUE, "/f"]))?;
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use super::*;

    pub fn registered(_paths: &AppPaths) -> bool {
        false
    }
    pub fn register(_paths: &AppPaths) -> Result<()> {
        Err(Error::unavailable(
            "input rewrite is not supported on this platform",
        ))
    }
    pub fn start(_paths: &AppPaths) -> Result<()> {
        Err(Error::unavailable(
            "input rewrite is not supported on this platform",
        ))
    }
    pub fn stop(_paths: &AppPaths) {}
    pub fn unregister(_paths: &AppPaths) -> Result<()> {
        Ok(())
    }
}
