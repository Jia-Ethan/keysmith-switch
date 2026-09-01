//! Official product catalog for Claude Code / Codex / Grok Build / ZCode.
//!
//! Plans expose current version, latest (if known), install state, resolved
//! executable, audited source URL, argv array, and destination. Install /
//! update is never executed unless `confirmed=true`.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::redact::redact_text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OfficialProduct {
    Claude,
    Codex,
    Grok,
    Zcode,
}

impl OfficialProduct {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::Zcode => "zcode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OfficialAction {
    Install,
    Update,
}

impl OfficialAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Update => "update",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPlan {
    pub plan_id: String,
    pub product: OfficialProduct,
    pub action: OfficialAction,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub installed: bool,
    pub executable_path: Option<String>,
    pub source: String,
    pub argv: Vec<String>,
    pub dest: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OfficialResult {
    pub ok: bool,
    pub product: OfficialProduct,
    pub action: OfficialAction,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DetectedOfficial {
    pub executable_path: Option<String>,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct OfficialHost {
    pub os: Option<String>,
    pub npm_available: Option<bool>,
    pub detected: HashMap<OfficialProduct, DetectedOfficial>,
    pub dest_override: HashMap<OfficialProduct, String>,
}

const CLAUDE_SOURCE: &str = "https://docs.anthropic.com/en/docs/claude-code";
const CLAUDE_PACKAGE: &str = "@anthropic-ai/claude-code";
const CODEX_SOURCE: &str = "https://www.npmjs.com/package/@openai/codex";
const CODEX_PACKAGE: &str = "@openai/codex";
const GROK_SOURCE: &str = "https://x.ai/build";
const ZCODE_SOURCE: &str = "https://zcode.z.ai/en/docs/install";
const ZCODE_APP: &str = "/Applications/ZCode.app";
const ZCODE_BIN: &str = "/Applications/ZCode.app/Contents/MacOS/ZCode";
const ZCODE_WINDOWS_REASON: &str =
    "ZCode is not available on Windows in Keysmith Switch; official desktop support is macOS-only";
const GROK_NO_FEED: &str =
    "no audited latest feed; local detection only, install is not auto-executed";
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const NPM_QUERY_TIMEOUT: Duration = Duration::from_secs(10);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);

fn plan_store() -> &'static Mutex<HashMap<String, OfficialPlan>> {
    static STORE: OnceLock<Mutex<HashMap<String, OfficialPlan>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn plan_official_action(product: OfficialProduct, action: OfficialAction) -> OfficialPlan {
    plan_official_action_on(product, action, &OfficialHost::default())
}

pub fn plan_official_action_on(
    product: OfficialProduct,
    action: OfficialAction,
    host: &OfficialHost,
) -> OfficialPlan {
    let os = host
        .os
        .clone()
        .unwrap_or_else(current_os)
        .to_ascii_lowercase();
    let npm_available = host
        .npm_available
        .unwrap_or_else(|| which::which("npm").is_ok());
    let detected = detect_product(product, host, &os, npm_available);
    let dest = host
        .dest_override
        .get(&product)
        .cloned()
        .unwrap_or_else(|| default_dest(product, &os, &detected, npm_available));
    let (source, mut argv, mut blockers) = planned_command(product, action, &os);
    if matches!(product, OfficialProduct::Claude | OfficialProduct::Codex) && !npm_available {
        blockers.push("npm is required to install or update this official CLI".to_string());
        argv.clear();
    }
    if product == OfficialProduct::Zcode && os == "windows" {
        blockers.clear();
        blockers.push(ZCODE_WINDOWS_REASON.to_string());
    }
    if product == OfficialProduct::Zcode && os == "linux" {
        blockers.push("ZCode is not a first-ship target on Linux".to_string());
    }
    if product == OfficialProduct::Grok {
        blockers.push(GROK_NO_FEED.to_string());
    }
    if !blockers.is_empty() {
        // Show the would-be command only when it is audited and executable.
        // Disabled products keep argv empty so confirm cannot run anything.
        if product == OfficialProduct::Zcode || product == OfficialProduct::Grok {
            // argv already empty
        }
    }
    let plan = OfficialPlan {
        plan_id: Uuid::new_v4().to_string(),
        product,
        action,
        current_version: detected.current_version,
        latest_version: detected.latest_version,
        installed: detected.executable_path.is_some(),
        executable_path: detected.executable_path,
        source: source.to_string(),
        argv,
        dest,
        blockers,
    };
    if let Ok(mut store) = plan_store().lock() {
        store.insert(plan.plan_id.clone(), plan.clone());
    }
    plan
}

pub fn confirm_official_action(plan_id: &str, confirmed: bool) -> OfficialResult {
    confirm_official_action_exec(plan_id, confirmed, default_exec)
}

pub fn confirm_official_action_cancellable<F>(
    plan_id: &str,
    confirmed: bool,
    cancelled: Arc<AtomicBool>,
    mut on_progress: F,
) -> OfficialResult
where
    F: FnMut(u64),
{
    confirm_official_action_exec(plan_id, confirmed, |argv| {
        default_exec_cancellable(argv, &cancelled, &mut on_progress)
    })
}

pub fn confirm_official_action_exec<F>(
    plan_id: &str,
    confirmed: bool,
    mut exec: F,
) -> OfficialResult
where
    F: FnMut(&[String]) -> Result<(), String>,
{
    let plan = match plan_store().lock() {
        Ok(store) => store.get(plan_id).cloned(),
        Err(_) => None,
    };
    let Some(plan) = plan else {
        return OfficialResult {
            ok: false,
            product: OfficialProduct::Claude,
            action: OfficialAction::Install,
            error: Some("unknown plan".to_string()),
        };
    };
    if !confirmed {
        return OfficialResult {
            ok: false,
            product: plan.product,
            action: plan.action,
            error: Some("confirmation required".to_string()),
        };
    }
    if !plan.blockers.is_empty() {
        return OfficialResult {
            ok: false,
            product: plan.product,
            action: plan.action,
            error: Some(plan.blockers.join("; ")),
        };
    }
    if plan.argv.is_empty() {
        return OfficialResult {
            ok: false,
            product: plan.product,
            action: plan.action,
            error: Some("no audited install command".to_string()),
        };
    }
    let result = match exec(&plan.argv) {
        Ok(()) => OfficialResult {
            ok: true,
            product: plan.product,
            action: plan.action,
            error: None,
        },
        Err(err) => OfficialResult {
            ok: false,
            product: plan.product,
            action: plan.action,
            error: Some(err),
        },
    };
    if let Ok(mut store) = plan_store().lock() {
        store.remove(plan_id);
    }
    result
}

fn planned_command(
    product: OfficialProduct,
    action: OfficialAction,
    os: &str,
) -> (&'static str, Vec<String>, Vec<String>) {
    match product {
        OfficialProduct::Claude => (CLAUDE_SOURCE, npm_argv(CLAUDE_PACKAGE, action), Vec::new()),
        OfficialProduct::Codex => (CODEX_SOURCE, npm_argv(CODEX_PACKAGE, action), Vec::new()),
        OfficialProduct::Grok => (GROK_SOURCE, Vec::new(), Vec::new()),
        OfficialProduct::Zcode => {
            let mut blockers = Vec::new();
            if os == "windows" {
                blockers.push(ZCODE_WINDOWS_REASON.to_string());
            } else if os != "macos" {
                blockers.push("ZCode official app path is documented for macOS only".to_string());
            } else {
                blockers.push(format!("ZCode must be installed manually into {ZCODE_APP}"));
            }
            (ZCODE_SOURCE, Vec::new(), blockers)
        }
    }
}

fn npm_argv(package: &str, action: OfficialAction) -> Vec<String> {
    match action {
        OfficialAction::Install => vec![
            "npm".to_string(),
            "install".to_string(),
            "-g".to_string(),
            package.to_string(),
        ],
        OfficialAction::Update => vec![
            "npm".to_string(),
            "install".to_string(),
            "-g".to_string(),
            format!("{package}@latest"),
        ],
    }
}

fn default_dest(
    product: OfficialProduct,
    os: &str,
    detected: &DetectedOfficial,
    npm_available: bool,
) -> String {
    match product {
        OfficialProduct::Claude => detected
            .executable_path
            .clone()
            .unwrap_or_else(|| npm_dest(CLAUDE_PACKAGE, npm_available)),
        OfficialProduct::Codex => detected
            .executable_path
            .clone()
            .unwrap_or_else(|| npm_dest(CODEX_PACKAGE, npm_available)),
        OfficialProduct::Grok => detected
            .executable_path
            .clone()
            .unwrap_or_else(|| "PATH:grok".to_string()),
        OfficialProduct::Zcode => {
            if os == "windows" {
                "unavailable-on-windows".to_string()
            } else {
                ZCODE_APP.to_string()
            }
        }
    }
}

fn detect_product(
    product: OfficialProduct,
    host: &OfficialHost,
    os: &str,
    npm_available: bool,
) -> DetectedOfficial {
    if let Some(detected) = host.detected.get(&product) {
        let mut detected = detected.clone();
        if product == OfficialProduct::Grok || product == OfficialProduct::Zcode {
            detected.latest_version = None;
        }
        return detected;
    }
    live_detect(product, os, npm_available)
}

fn live_detect(product: OfficialProduct, os: &str, npm_available: bool) -> DetectedOfficial {
    match product {
        OfficialProduct::Claude => detect_bin("claude", Some(CLAUDE_PACKAGE), npm_available),
        OfficialProduct::Codex => detect_bin("codex", Some(CODEX_PACKAGE), npm_available),
        OfficialProduct::Grok => {
            let mut d = detect_bin("grok", None, npm_available);
            d.latest_version = None;
            d
        }
        OfficialProduct::Zcode => detect_zcode(os),
    }
}

fn detect_bin(name: &str, npm_package: Option<&str>, npm_available: bool) -> DetectedOfficial {
    let executable_path = which::which(name)
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let current_version = executable_path
        .as_deref()
        .and_then(|p| run_version(Path::new(p)));
    let latest_version = npm_package
        .filter(|_| npm_available)
        .and_then(|package| npm_view_version(package));
    DetectedOfficial {
        executable_path,
        current_version,
        latest_version,
    }
}

fn detect_zcode(os: &str) -> DetectedOfficial {
    if os != "macos" {
        return DetectedOfficial::default();
    }
    let app = PathBuf::from(ZCODE_APP);
    if !app.exists() {
        return DetectedOfficial::default();
    }
    let bin = PathBuf::from(ZCODE_BIN);
    let executable_path = if bin.exists() {
        Some(ZCODE_BIN.to_string())
    } else {
        Some(ZCODE_APP.to_string())
    };
    let current_version = read_plist_version(&app.join("Contents/Info.plist"))
        .or_else(|| run_version(Path::new(ZCODE_BIN)));
    DetectedOfficial {
        executable_path,
        current_version,
        latest_version: None,
    }
}

fn run_version(path: &Path) -> Option<String> {
    let mut command = Command::new(path);
    command.arg("--version");
    let output = run_command_timeout(&mut command, VERSION_TIMEOUT).ok()?;
    if !output.success && output.stdout.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn npm_view_version(package: &str) -> Option<String> {
    let mut command = Command::new("npm");
    command.args(["view", package, "version"]);
    let output = run_command_timeout(&mut command, NPM_QUERY_TIMEOUT).ok()?;
    if !output.success {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() || text.contains('\n') && text.len() > 32 {
        None
    } else {
        Some(text)
    }
}

fn npm_dest(package: &str, npm_available: bool) -> String {
    if npm_available {
        let mut command = Command::new("npm");
        command.args(["root", "-g"]);
        if let Ok(output) = run_command_timeout(&mut command, NPM_QUERY_TIMEOUT) {
            if output.success {
                let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !root.is_empty() {
                    return format!("{root}/{package}");
                }
            }
        }
    }
    format!("npm-global:{package}")
}

fn read_plist_version(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let key = "<key>CFBundleShortVersionString</key>";
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")?;
    let ver = rest[start..start + end].trim();
    if ver.is_empty() {
        None
    } else {
        Some(ver.to_string())
    }
}

fn current_os() -> String {
    match std::env::consts::OS {
        "macos" => "macos".to_string(),
        "windows" => "windows".to_string(),
        "linux" => "linux".to_string(),
        other => other.to_string(),
    }
}

fn default_exec(argv: &[String]) -> Result<(), String> {
    let cancelled = AtomicBool::new(false);
    default_exec_cancellable(argv, &cancelled, &mut |_| {})
}

fn default_exec_cancellable<F>(
    argv: &[String],
    cancelled: &AtomicBool,
    on_progress: &mut F,
) -> Result<(), String>
where
    F: FnMut(u64),
{
    if std::env::var("KEYSMITH_SWITCH_OFFICIAL_DRY_RUN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return Ok(());
    }
    if argv.is_empty() {
        return Err("empty argv".to_string());
    }
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    let output =
        run_command_timeout_cancellable(&mut command, INSTALL_TIMEOUT, cancelled, on_progress)?;
    if output.success {
        Ok(())
    } else {
        let detail = first_safe_line(&output.stderr, "command failed");
        Err(format!("exit {}: {detail}", output.code.unwrap_or(-1)))
    }
}

struct CapturedOutput {
    success: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_command_timeout(command: &mut Command, timeout: Duration) -> Result<CapturedOutput, String> {
    let cancelled = AtomicBool::new(false);
    run_command_timeout_cancellable(command, timeout, &cancelled, &mut |_| {})
}

fn run_command_timeout_cancellable<F>(
    command: &mut Command,
    timeout: Duration,
    cancelled: &AtomicBool,
    on_progress: &mut F,
) -> Result<CapturedOutput, String>
where
    F: FnMut(u64),
{
    let id = Uuid::new_v4();
    let stdout_path = std::env::temp_dir().join(format!("keysmith-switch-{id}.stdout"));
    let stderr_path = std::env::temp_dir().join(format!("keysmith-switch-{id}.stderr"));
    let mut stdout_file = create_capture(&stdout_path)?;
    let mut stderr_file = match create_capture(&stderr_path) {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&stdout_path);
            return Err(error);
        }
    };
    let stdout_child = match stdout_file.try_clone() {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&stdout_path);
            let _ = std::fs::remove_file(&stderr_path);
            return Err(safe_io_error(&error));
        }
    };
    let stderr_child = match stderr_file.try_clone() {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&stdout_path);
            let _ = std::fs::remove_file(&stderr_path);
            return Err(safe_io_error(&error));
        }
    };
    command
        .stdout(Stdio::from(stdout_child))
        .stderr(Stdio::from(stderr_child));
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = std::fs::remove_file(&stdout_path);
            let _ = std::fs::remove_file(&stderr_path);
            return Err(safe_io_error(&error));
        }
    };
    let deadline = Instant::now() + timeout;
    let started = Instant::now();
    let mut last_progress = u64::MAX;

    let status = loop {
        let elapsed = started.elapsed().as_secs();
        if elapsed != last_progress {
            on_progress(elapsed);
            last_progress = elapsed;
        }
        if cancelled.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&stdout_path);
            let _ = std::fs::remove_file(&stderr_path);
            return Err("cancelled by user".to_string());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&stdout_path);
                let _ = std::fs::remove_file(&stderr_path);
                return Err(format!(
                    "command timed out after {} seconds",
                    timeout.as_secs()
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&stdout_path);
                let _ = std::fs::remove_file(&stderr_path);
                return Err(safe_io_error(&error));
            }
        }
    };

    let stdout = read_capture(&mut stdout_file, &stdout_path)?;
    let stderr = read_capture(&mut stderr_file, &stderr_path)?;
    Ok(CapturedOutput {
        success: status.success(),
        code: status.code(),
        stdout,
        stderr,
    })
}

fn create_capture(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| safe_io_error(&error))
}

fn read_capture(file: &mut File, path: &Path) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| safe_io_error(&error))?;
    let mut bytes = Vec::new();
    let result = file
        .read_to_end(&mut bytes)
        .map(|_| bytes)
        .map_err(|error| safe_io_error(&error));
    let _ = std::fs::remove_file(path);
    result
}

fn safe_io_error(error: &std::io::Error) -> String {
    first_safe_line(error.to_string().as_bytes(), "command failed")
}

fn first_safe_line(bytes: &[u8], fallback: &str) -> String {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().next().unwrap_or(fallback).trim();
    let line = if line.is_empty() { fallback } else { line };
    redact_text(line).chars().take(240).collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn cancellable_command_stops_the_child_process() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&cancelled);
        let setter = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal.store(true, Ordering::SeqCst);
        });
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 5"]);
        let started = Instant::now();
        let error = match run_command_timeout_cancellable(
            &mut command,
            Duration::from_secs(10),
            &cancelled,
            &mut |_| {},
        ) {
            Ok(_) => panic!("command was not cancelled"),
            Err(error) => error,
        };
        setter.join().unwrap();
        assert!(error.contains("cancelled"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
