//! Import, export, backup, first-run scan, and clear-all-data.
//!
//! Native prompt files are scanned read-only. Import copies into the
//! Keysmith Switch library and never activates.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::ZipArchive;

use crate::db::Store;
use crate::error::{Error, Result};
use crate::models::{now_rfc3339, CreatePromptInput, ToolKind};
use crate::ops;
use crate::paths::AppPaths;
use crate::redact::redact_text;

pub const RECOVERY_MARKER: &str = "last-recovery.json";
pub const CLEAR_CONFIRM_PHRASE: &str = "CLEAR ALL DATA";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub id: String,
    pub tool: ToolKind,
    pub path: String,
    pub title: String,
    pub excerpt: String,
    pub already_imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FirstRunReport {
    pub first_run: bool,
    pub candidates: Vec<ImportCandidate>,
    pub recovery: Option<RecoveryMarker>,
    pub sidecar: SidecarReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SidecarReport {
    pub python_required: bool,
    pub tools: Vec<SidecarToolStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SidecarToolStatus {
    pub tool: ToolKind,
    pub frozen: bool,
    pub path: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryMarker {
    pub kind: String,
    pub quarantined: Option<String>,
    pub rebuilt: bool,
    pub at: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub id: String,
    pub path: String,
    pub created_at: String,
    pub kind: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClearPlan {
    pub home: String,
    pub categories: Vec<ClearCategory>,
    pub irreversible: bool,
    pub confirm_phrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClearCategory {
    pub name: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DataDirs {
    pub home: String,
    pub logs: String,
    pub backups: String,
    pub prompts: String,
}

pub fn write_recovery_marker(paths: &AppPaths, marker: &RecoveryMarker) -> Result<()> {
    paths.ensure()?;
    let path = paths.logs.join(RECOVERY_MARKER);
    let json = serde_json::to_string_pretty(marker)?;
    crate::paths::atomic_write(&path, &json)
}

pub fn read_recovery_marker(paths: &AppPaths) -> Option<RecoveryMarker> {
    let path = paths.logs.join(RECOVERY_MARKER);
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn acknowledge_recovery(paths: &AppPaths) -> Result<()> {
    let path = paths.logs.join(RECOVERY_MARKER);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn data_dirs(paths: &AppPaths) -> DataDirs {
    DataDirs {
        home: paths.home.display().to_string(),
        logs: paths.logs.display().to_string(),
        backups: paths.backups.display().to_string(),
        prompts: paths.prompts.display().to_string(),
    }
}

pub fn scan_import_candidates(store: &Store, user_home: &Path) -> Result<Vec<ImportCandidate>> {
    let mut out = Vec::new();
    for tool in ToolKind::ALL {
        if !tool.available_on_this_os() {
            continue;
        }
        for path in native_prompt_files(tool, user_home) {
            if let Some(candidate) = candidate_from_file(store, tool, &path)? {
                out.push(candidate);
            }
        }
    }
    Ok(out)
}

fn native_prompt_files(tool: ToolKind, home: &Path) -> Vec<PathBuf> {
    let roots = match tool {
        ToolKind::Claude => vec![home.join(".claude").join("keysmith")],
        ToolKind::Codex => vec![
            home.join(".codex").join("keysmith"),
            home.join(".codex").join("prompts"),
        ],
        ToolKind::Grok => vec![home.join(".grok").join("rules")],
        ToolKind::Zcode => vec![home.join(".zcode-keysmith")],
    };
    let mut files = Vec::new();
    for root in roots {
        collect_markdown(&root, &mut files);
    }
    files
}

fn collect_markdown(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
        }
    }
}

fn candidate_from_file(
    store: &Store,
    tool: ToolKind,
    path: &Path,
) -> Result<Option<ImportCandidate>> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(None);
    };
    if content.trim().is_empty() {
        return Ok(None);
    }
    let title = title_from_markdown(&content, path);
    let excerpt = excerpt(&content);
    let already = store
        .list_prompts(tool, None, None, crate::models::PromptSort::Title)?
        .iter()
        .any(|item| item.title == title);
    Ok(Some(ImportCandidate {
        id: format!("{}:{}", tool.as_str(), path.display()),
        tool,
        path: path.display().to_string(),
        title,
        excerpt,
        already_imported: already,
    }))
}

fn title_from_markdown(content: &str, path: &Path) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
        if !trimmed.is_empty() && !trimmed.starts_with("---") {
            return trimmed.chars().take(80).collect();
        }
    }
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "imported".into())
}

fn excerpt(content: &str) -> String {
    let line = content
        .lines()
        .find(|item| !item.trim().is_empty())
        .unwrap_or("")
        .trim();
    line.chars().take(120).collect()
}

pub fn import_candidates(store: &Store, paths: &[String]) -> Result<ImportResult> {
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    for raw in paths {
        let path = PathBuf::from(raw);
        let tool = infer_tool_from_path(&path).unwrap_or(ToolKind::Claude);
        match import_markdown_file(store, tool, &path) {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(error) => errors.push(redact_text(&error.to_string())),
        }
    }
    Ok(ImportResult {
        imported,
        skipped,
        errors,
    })
}

pub fn import_markdown_file(store: &Store, tool: ToolKind, path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(false);
    }
    let title = title_from_markdown(&content, path);
    ops::create_prompt(
        store,
        CreatePromptInput {
            tool,
            title,
            content,
            tags: vec!["imported".into()],
        },
    )?;
    Ok(true)
}

fn infer_tool_from_path(path: &Path) -> Option<ToolKind> {
    let text = path.to_string_lossy().to_ascii_lowercase();
    if text.contains(".claude") || text.contains("/claude/") {
        Some(ToolKind::Claude)
    } else if text.contains(".codex") || text.contains("/codex/") {
        Some(ToolKind::Codex)
    } else if text.contains(".grok") || text.contains("/grok/") {
        Some(ToolKind::Grok)
    } else if text.contains("zcode") {
        Some(ToolKind::Zcode)
    } else {
        None
    }
}

pub fn export_zip(store: &Store, dest: &Path) -> Result<PathBuf> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let manifest = serde_json::json!({
        "app": "Keysmith Switch",
        "version": crate::models::APP_VERSION,
        "exportedAt": now_rfc3339(),
        "preview": true,
    });
    zip.start_file("manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    for tool in ToolKind::ALL {
        for prompt in store.list_prompts(tool, None, None, crate::models::PromptSort::Title)? {
            let detail = store.get_prompt(&prompt.id)?;
            let rel = format!("prompts/{}/{}.md", tool.as_str(), prompt.id);
            zip.start_file(&rel, options)?;
            zip.write_all(detail.content.as_bytes())?;
        }
    }
    zip.finish()?;
    Ok(dest.to_path_buf())
}

pub fn import_zip(store: &Store, zip_path: &Path) -> Result<ImportResult> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|error| Error::invalid(error.to_string()))?;
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(redact_text(&error.to_string()));
                continue;
            }
        };
        let name = entry.name().to_string();
        if !name.ends_with(".md") {
            continue;
        }
        let tool = name
            .split('/')
            .find_map(|part| part.parse::<ToolKind>().ok())
            .or_else(|| infer_tool_from_path(Path::new(&name)))
            .unwrap_or(ToolKind::Claude);
        let mut content = String::new();
        if let Err(error) = entry.read_to_string(&mut content) {
            errors.push(redact_text(&error.to_string()));
            continue;
        }
        let title = title_from_markdown(&content, Path::new(&name));
        match ops::create_prompt(
            store,
            CreatePromptInput {
                tool,
                title,
                content,
                tags: vec!["imported".into(), "zip".into()],
            },
        ) {
            Ok(_) => imported += 1,
            Err(error) => {
                skipped += 1;
                errors.push(redact_text(&error.to_string()));
            }
        }
    }
    Ok(ImportResult {
        imported,
        skipped,
        errors,
    })
}

pub fn create_named_backup(store: &Store, kind: &str) -> Result<BackupEntry> {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let id = format!("{kind}-{stamp}");
    let dest = store.paths().backups.join(format!("{id}.zip"));
    export_zip(store, &dest)?;
    let _ = store.backup_db(&format!("{kind}-{stamp}"));
    Ok(backup_meta(&dest, kind)?)
}

pub fn list_backups(paths: &AppPaths) -> Result<Vec<BackupEntry>> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&paths.backups) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("zip") {
            if let Ok(meta) = backup_meta(&path, "zip") {
                out.push(meta);
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

fn backup_meta(path: &Path, kind: &str) -> Result<BackupEntry> {
    let meta = fs::metadata(path)?;
    let created = meta
        .modified()
        .ok()
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339())
        .unwrap_or_else(now_rfc3339);
    Ok(BackupEntry {
        id: path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "backup".into()),
        path: path.display().to_string(),
        created_at: created,
        kind: kind.to_string(),
        bytes: meta.len(),
    })
}

pub fn restore_backup_zip(store: &Store, zip_path: &Path) -> Result<ImportResult> {
    import_zip(store, zip_path)
}

pub fn clear_plan(paths: &AppPaths) -> ClearPlan {
    ClearPlan {
        home: paths.home.display().to_string(),
        categories: vec![
            cat("database", &paths.db),
            cat("prompts", &paths.prompts),
            cat("backups", &paths.backups),
            cat("logs", &paths.logs),
        ],
        irreversible: true,
        confirm_phrase: CLEAR_CONFIRM_PHRASE.into(),
    }
}

fn cat(name: &str, path: &Path) -> ClearCategory {
    ClearCategory {
        name: name.into(),
        path: path.display().to_string(),
        exists: path.exists(),
    }
}

pub fn clear_all_data(paths: &AppPaths, phrase: &str, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(Error::user_cancel("confirmation required"));
    }
    if phrase.trim() != CLEAR_CONFIRM_PHRASE {
        return Err(Error::invalid("clear-all confirmation phrase mismatch"));
    }
    if paths.home.exists() {
        fs::remove_dir_all(&paths.home)?;
    }
    paths.ensure()?;
    Ok(())
}

pub fn sidecar_report() -> SidecarReport {
    use crate::adapter::process::resolve_cli;
    let mut python_required = false;
    let mut tools = Vec::new();
    for tool in ToolKind::ALL {
        match resolve_cli(tool, &crate::adapter::AdapterOptions::default()) {
            Ok(cli) => {
                if !cli.frozen {
                    python_required = true;
                }
                tools.push(SidecarToolStatus {
                    tool,
                    frozen: cli.frozen,
                    path: Some(cli.cli_path()),
                    available: true,
                });
            }
            Err(_) => {
                tools.push(SidecarToolStatus {
                    tool,
                    frozen: false,
                    path: None,
                    available: tool.available_on_this_os(),
                });
            }
        }
    }
    SidecarReport {
        python_required,
        tools,
    }
}
