use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic;
use std::sync::Mutex;

use crate::error::Result;
use crate::paths::AppPaths;
use crate::redact::redact_text;

static LOG_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

pub fn init(paths: &AppPaths) -> Result<()> {
    paths.ensure()?;
    let path = paths.logs.join("keysmith-switch.log");
    if let Ok(mut guard) = LOG_PATH.lock() {
        *guard = Some(path.clone());
    }
    append_raw(
        &path,
        &format!(
            "=== Keysmith Switch {} start {} ===",
            crate::models::APP_VERSION,
            chrono::Utc::now().to_rfc3339()
        ),
    );
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "panic".to_string()
        };
        let location = info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown".into());
        let _ = write_line("panic", &format!("{location} {payload}"));
        default_hook(info);
    }));
    Ok(())
}

pub fn write_line(kind: &str, message: &str) -> Result<()> {
    let path = LOG_PATH
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .ok_or_else(|| crate::error::Error::message("log path not initialized"))?;
    let line = format!(
        "{} [{}] {}",
        chrono::Utc::now().to_rfc3339(),
        kind,
        redact_text(message)
    );
    append_raw(&path, &line);
    Ok(())
}

pub fn frontend_error(message: &str, stack: Option<&str>) -> Result<()> {
    let mut body = message.to_string();
    if let Some(stack) = stack {
        body.push('\n');
        body.push_str(stack);
    }
    write_line("frontend", &body)
}

fn append_raw(path: &std::path::Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
}
