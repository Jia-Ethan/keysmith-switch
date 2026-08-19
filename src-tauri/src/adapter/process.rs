use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::time::timeout;

use crate::error::{Error, Result};
use crate::models::ToolKind;
use crate::redact::redact_text;

use super::AdapterOptions;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(15);
pub const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ResolvedCli {
    pub python: PathBuf,
    pub script: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Captured {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
    pub truncated: bool,
    pub argv: Vec<String>,
}

struct StreamCapture {
    bytes: Vec<u8>,
    truncated: bool,
    error: Option<String>,
}

pub fn resolve_python() -> Result<PathBuf> {
    which::which("python3")
        .or_else(|_| which::which("python"))
        .map_err(|_| Error::cli_missing("python3 is not available on PATH"))
}

pub fn resolve_cli(tool: ToolKind, opts: &AdapterOptions) -> Result<ResolvedCli> {
    let python = resolve_python()?;
    if let Some(path) = &opts.cli_override {
        return existing_script(python, path);
    }
    let env_key = tool.env_cli_key();
    if let Some(path) = opts.extra_env.get(env_key) {
        return existing_script(python, Path::new(path));
    }
    if let Ok(path) = std::env::var(env_key) {
        if !path.trim().is_empty() {
            return existing_script(python, Path::new(&path));
        }
    }
    if let Some(script) = find_vendored_script(tool) {
        return existing_script(python, &script);
    }
    if let Ok(path) = which::which(script_basename(tool)) {
        return existing_script(python, &path);
    }
    Err(Error::cli_missing(format!(
        "{} CLI not found (set {}, vendored script, or PATH)",
        tool.as_str(),
        env_key
    )))
}

fn existing_script(python: PathBuf, path: &Path) -> Result<ResolvedCli> {
    if !path.exists() {
        return Err(Error::cli_missing(format!(
            "CLI script does not exist: {}",
            path.display()
        )));
    }
    Ok(ResolvedCli {
        python,
        script: path.to_path_buf(),
    })
}

fn script_basename(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Claude => "claude-instruct.py",
        ToolKind::Codex => "codex-instruct.py",
        ToolKind::Grok => "grok-keysmith.py",
        ToolKind::Zcode => "zcode-keysmith.py",
    }
}

pub fn find_vendored_script(tool: ToolKind) -> Option<PathBuf> {
    let rel = tool.vendored_rel();
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
    for root in roots {
        let candidate = root.join(rel);
        if candidate.is_file() {
            return candidate.canonicalize().ok().or(Some(candidate));
        }
    }
    None
}

pub async fn invoke(
    cli: &ResolvedCli,
    args: &[String],
    opts: &AdapterOptions,
    limit: Duration,
) -> Result<Captured> {
    let mut argv = vec![
        cli.python.to_string_lossy().into_owned(),
        cli.script.to_string_lossy().into_owned(),
    ];
    argv.extend(args.iter().cloned());

    let mut command = Command::new(&cli.python);
    command.arg(&cli.script);
    command.args(args);
    configure_process_tree(&mut command);
    command.kill_on_drop(true);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if let Some(home) = &opts.home {
        command.env("HOME", home);
        command.env("USERPROFILE", home);
        command.env("CLAUDE_KEYSMITH_HOME", home);
    }
    for (key, value) in &opts.extra_env {
        command.env(key, value);
    }

    let mut child = command.spawn().map_err(|error| {
        Error::cli_missing(format!(
            "failed to start {} {}: {error}",
            cli.python.display(),
            cli.script.display()
        ))
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::command_failed("CLI stdout pipe missing"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::command_failed("CLI stderr pipe missing"))?;
    let read_task =
        tokio::spawn(async move { tokio::join!(read_capped(stdout), read_capped(stderr)) });

    let wait = timeout(limit, child.wait()).await;
    match wait {
        Ok(Ok(status)) => {
            let (stdout_cap, stderr_cap) = join_output(read_task).await?;
            finish_capture(
                argv,
                status.code().unwrap_or(-1),
                false,
                stdout_cap,
                stderr_cap,
            )
        }
        Ok(Err(error)) => {
            terminate_process_tree(&mut child).await;
            let _ = read_task.await;
            Err(Error::command_failed(format!("wait failed: {error}")))
        }
        Err(_) => {
            terminate_process_tree(&mut child).await;
            let (stdout_cap, stderr_cap) = match read_task.await {
                Ok(pair) => pair,
                Err(_) => (StreamCapture::default(), StreamCapture::default()),
            };
            finish_capture(argv, -1, true, stdout_cap, stderr_cap)
        }
    }
}

async fn join_output(
    task: tokio::task::JoinHandle<(StreamCapture, StreamCapture)>,
) -> Result<(StreamCapture, StreamCapture)> {
    task.await
        .map_err(|error| Error::command_failed(format!("output task failed: {error}")))
}

fn finish_capture(
    argv: Vec<String>,
    exit_code: i32,
    timed_out: bool,
    stdout: StreamCapture,
    stderr: StreamCapture,
) -> Result<Captured> {
    if let Some(error) = stdout.error.as_ref().or(stderr.error.as_ref()) {
        return Err(Error::command_failed(error));
    }
    if stdout.truncated || stderr.truncated {
        return Err(Error::command_failed(format!(
            "CLI output exceeded {MAX_OUTPUT_BYTES} byte cap"
        )));
    }
    Ok(Captured {
        stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
        stderr: redact_text(&String::from_utf8_lossy(&stderr.bytes)),
        exit_code,
        timed_out,
        truncated: false,
        argv,
    })
}

async fn read_capped<R>(mut reader: R) -> StreamCapture
where
    R: AsyncRead + Unpin,
{
    let mut captured = StreamCapture::default();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) => {
                captured.error = Some(error.to_string());
                break;
            }
        };
        let remaining = MAX_OUTPUT_BYTES.saturating_sub(captured.bytes.len());
        if remaining > 0 {
            captured
                .bytes
                .extend_from_slice(&chunk[..read.min(remaining)]);
        }
        if read > remaining {
            captured.truncated = true;
        }
    }
    captured
}

impl Default for StreamCapture {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            truncated: false,
            error: None,
        }
    }
}

#[cfg(unix)]
fn configure_process_tree(command: &mut Command) {
    command.process_group(0);
}

#[cfg(windows)]
fn configure_process_tree(command: &mut Command) {
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn configure_process_tree(_command: &mut Command) {}

#[cfg(unix)]
async fn terminate_process_tree(child: &mut Child) {
    if let Some(pid) = child.id() {
        if let Ok(pid) = i32::try_from(pid) {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(windows)]
async fn terminate_process_tree(child: &mut Child) {
    if let Some(pid) = child.id() {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(not(any(unix, windows)))]
async fn terminate_process_tree(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}
