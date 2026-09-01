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
use crate::lock::HomeLock;
use crate::models::{now_rfc3339, sha256_hex, CreatePromptInput, ToolKind};
use crate::ops;
use crate::paths::AppPaths;
use crate::redact::redact_text;

pub const RECOVERY_MARKER: &str = "last-recovery.json";
pub const CLEAR_CONFIRM_PHRASE: &str = "CLEAR ALL DATA";
const BACKUP_FORMAT: &str = "keysmith-switch-backup-v2";
const BACKUP_DB_PATH: &str = "data/keysmith-switch.db";
const MAX_BACKUP_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_BACKUP_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_BACKUP_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format: String,
    app_version: String,
    schema_version: i64,
    exported_at: String,
    database: BackupFile,
    prompts: Vec<BackupFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    path: String,
    sha256: String,
    bytes: u64,
}

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
pub struct ArchiveInspection {
    pub mode: String,
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
    let _lock = HomeLock::acquire(store.paths())?;
    if dest == store.paths().db || dest.starts_with(&store.paths().prompts) {
        return Err(Error::invalid(
            "backup destination must be outside active data files",
        ));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let work = tempfile::tempdir_in(&store.paths().home)?;
    let db_snapshot = work.path().join("keysmith-switch.db");
    store.backup_db_to(&db_snapshot)?;
    let db_bytes = fs::read(&db_snapshot)?;
    let database = backup_file(BACKUP_DB_PATH, &db_bytes);
    let mut prompt_files = Vec::new();
    for entry in walkdir::WalkDir::new(&store.paths().prompts) {
        let entry = entry.map_err(|error| Error::invalid(error.to_string()))?;
        if entry.file_type().is_symlink() {
            return Err(Error::invalid("prompt library contains a symlink"));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&store.paths().home)
            .map_err(|_| Error::invalid("prompt path is outside the data directory"))?
            .to_string_lossy()
            .replace('\\', "/");
        if !rel.ends_with(".md") {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        prompt_files.push((backup_file(&rel, &bytes), bytes));
    }
    prompt_files.sort_by(|a, b| a.0.path.cmp(&b.0.path));
    let manifest = BackupManifest {
        format: BACKUP_FORMAT.into(),
        app_version: crate::models::APP_VERSION.into(),
        schema_version: crate::db::schema::SCHEMA_VERSION,
        exported_at: now_rfc3339(),
        database,
        prompts: prompt_files
            .iter()
            .map(|(m, _)| BackupFile {
                path: m.path.clone(),
                sha256: m.sha256.clone(),
                bytes: m.bytes,
            })
            .collect(),
    };
    let temp_dest = dest.with_file_name(format!(
        ".{}.keysmith-switch-tmp-{}",
        dest.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("backup.zip"),
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| -> Result<()> {
        let mut zip = zip::ZipWriter::new(File::create(&temp_dest)?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
        zip.start_file(BACKUP_DB_PATH, options)?;
        zip.write_all(&db_bytes)?;
        for (meta, bytes) in &prompt_files {
            zip.start_file(&meta.path, options)?;
            zip.write_all(bytes)?;
        }
        zip.finish()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temp_dest);
        return Err(error);
    }
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    fs::rename(&temp_dest, dest)?;
    Ok(dest.to_path_buf())
}

pub fn import_zip(store: &Store, zip_path: &Path) -> Result<ImportResult> {
    match read_backup_archive(zip_path) {
        Ok(archive) => {
            let _lock = HomeLock::acquire(store.paths())?;
            Ok(ImportResult {
                imported: restore_archive(store, archive)?,
                skipped: 0,
                errors: Vec::new(),
            })
        }
        Err(_error) if is_legacy_archive(zip_path) => import_legacy_zip(store, zip_path),
        Err(error) => Err(error),
    }
}

pub fn inspect_zip(zip_path: &Path) -> Result<ArchiveInspection> {
    match read_backup_archive(zip_path) {
        Ok(_) => Ok(ArchiveInspection {
            mode: "restore".to_string(),
        }),
        Err(_error) if is_legacy_archive(zip_path) => Ok(ArchiveInspection {
            mode: "import".to_string(),
        }),
        Err(error) => Err(error),
    }
}

struct ExtractedBackup {
    work: tempfile::TempDir,
    manifest: BackupManifest,
}

fn read_backup_archive(zip_path: &Path) -> Result<ExtractedBackup> {
    let mut archive =
        ZipArchive::new(File::open(zip_path)?).map_err(|e| Error::invalid(e.to_string()))?;
    let mut manifest_text = String::new();
    let mut manifest_entry = archive
        .by_name("manifest.json")
        .map_err(|_| Error::invalid("backup manifest is missing"))?;
    if manifest_entry.size() > MAX_BACKUP_MANIFEST_BYTES {
        return Err(Error::invalid("backup manifest exceeds the size limit"));
    }
    manifest_entry.read_to_string(&mut manifest_text)?;
    drop(manifest_entry);
    let manifest: BackupManifest = serde_json::from_str(&manifest_text)
        .map_err(|_| Error::invalid("backup manifest is invalid"))?;
    if manifest.format != BACKUP_FORMAT
        || manifest.schema_version != crate::db::schema::SCHEMA_VERSION
        || manifest.database.path != BACKUP_DB_PATH
    {
        return Err(Error::invalid("backup format or schema is not supported"));
    }
    let work = tempfile::tempdir()?;
    let mut total = 0_u64;
    let mut seen = std::collections::HashSet::new();
    for expected in std::iter::once(&manifest.database).chain(manifest.prompts.iter()) {
        if !seen.insert(expected.path.clone()) {
            return Err(Error::invalid("backup manifest contains duplicate paths"));
        }
        validate_archive_path(&expected.path)?;
        if expected.bytes > MAX_BACKUP_ENTRY_BYTES {
            return Err(Error::invalid("backup entry exceeds the size limit"));
        }
        total = total
            .checked_add(expected.bytes)
            .ok_or_else(|| Error::invalid("backup size overflow"))?;
        if total > MAX_BACKUP_TOTAL_BYTES {
            return Err(Error::invalid("backup exceeds the total size limit"));
        }
        let mut entry = archive
            .by_name(&expected.path)
            .map_err(|_| Error::invalid("backup entry is missing"))?;
        if entry.is_dir() || entry.size() != expected.bytes {
            return Err(Error::invalid("backup entry size does not match manifest"));
        }
        let mut bytes = Vec::with_capacity(expected.bytes as usize);
        entry.read_to_end(&mut bytes)?;
        if sha256_hex(&bytes) != expected.sha256 {
            return Err(Error::invalid("backup entry checksum mismatch"));
        }
        let target = work.path().join(&expected.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
    }
    let mut archive_seen = std::collections::HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if !archive_seen.insert(entry.name().to_string()) {
            return Err(Error::invalid("backup contains duplicate entries"));
        }
        if entry.name() != "manifest.json" && !seen.contains(entry.name()) && !entry.is_dir() {
            return Err(Error::invalid("backup contains an undeclared file"));
        }
    }
    Ok(ExtractedBackup { work, manifest })
}

fn restore_archive(store: &Store, archive: ExtractedBackup) -> Result<usize> {
    let db = archive.work.path().join(&archive.manifest.database.path);
    let prompts = archive.work.path().join("prompts");
    fs::create_dir_all(&prompts)?;
    store.restore_snapshot(&db, &prompts)
}

fn backup_file(path: &str, bytes: &[u8]) -> BackupFile {
    BackupFile {
        path: path.into(),
        sha256: sha256_hex(bytes),
        bytes: bytes.len() as u64,
    }
}

fn validate_archive_path(raw: &str) -> Result<()> {
    let path = Path::new(raw);
    if path.is_absolute()
        || raw.contains('\\')
        || path
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        || (raw != BACKUP_DB_PATH && !(raw.starts_with("prompts/") && raw.ends_with(".md")))
    {
        return Err(Error::invalid("backup contains an unsafe path"));
    }
    Ok(())
}

fn is_legacy_archive(zip_path: &Path) -> bool {
    let Ok(file) = File::open(zip_path) else {
        return false;
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return false;
    };
    let Ok(mut entry) = archive.by_name("manifest.json") else {
        return false;
    };
    let mut text = String::new();
    entry.read_to_string(&mut text).is_ok()
        && serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("app").and_then(|v| v.as_str()).map(str::to_owned))
            .as_deref()
            == Some("Keysmith Switch")
}

fn import_legacy_zip(store: &Store, zip_path: &Path) -> Result<ImportResult> {
    let mut archive =
        ZipArchive::new(File::open(zip_path)?).map_err(|e| Error::invalid(e.to_string()))?;
    let mut result = ImportResult {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        if !name.starts_with("prompts/") || !name.ends_with(".md") {
            continue;
        }
        validate_archive_path(&name)?;
        if entry.size() > MAX_BACKUP_ENTRY_BYTES {
            return Err(Error::invalid(
                "legacy archive entry exceeds the size limit",
            ));
        }
        let tool = name
            .split('/')
            .find_map(|part| part.parse::<ToolKind>().ok())
            .unwrap_or(ToolKind::Claude);
        let mut content = String::new();
        entry.read_to_string(&mut content)?;
        match ops::create_prompt(
            store,
            CreatePromptInput {
                tool,
                title: title_from_markdown(&content, Path::new(&name)),
                content,
                tags: vec!["imported".into(), "legacy-zip".into()],
            },
        ) {
            Ok(_) => result.imported += 1,
            Err(error) => {
                result.skipped += 1;
                result.errors.push(redact_text(&error.to_string()));
            }
        }
    }
    Ok(result)
}

pub fn create_named_backup(store: &Store, kind: &str) -> Result<BackupEntry> {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let id = format!("{kind}-{stamp}");
    let dest = store.paths().backups.join(format!("{id}.zip"));
    export_zip(store, &dest)?;
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

pub fn clear_all_data(store: &Store, phrase: &str, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(Error::user_cancel("confirmation required"));
    }
    if phrase.trim() != CLEAR_CONFIRM_PHRASE {
        return Err(Error::invalid("clear-all confirmation phrase mismatch"));
    }
    let _lock = HomeLock::acquire(store.paths())?;
    store.clear_all()
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
