use std::time::Duration;

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::models::now_rfc3339;
use crate::paths::AppPaths;

pub const SCHEMA_VERSION: i64 = 1;

const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS prompts (
    id TEXT PRIMARY KEY,
    tool TEXT NOT NULL,
    title TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    version INTEGER NOT NULL DEFAULT 1,
    content_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS prompt_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    prompt_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    title TEXT NOT NULL,
    tags TEXT NOT NULL,
    content TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(prompt_id, version)
);

CREATE TABLE IF NOT EXISTS activations (
    id TEXT PRIMARY KEY,
    prompt_id TEXT,
    tool TEXT NOT NULL,
    scope TEXT NOT NULL,
    project_dir TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    fingerprint TEXT,
    operation_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS operations (
    id TEXT PRIMARY KEY,
    tool TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    preview INTEGER NOT NULL,
    prompt_id TEXT,
    scope TEXT,
    project_dir TEXT,
    request_json TEXT NOT NULL,
    envelope_json TEXT,
    error TEXT,
    parent_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tool_state (
    tool TEXT NOT NULL,
    scope TEXT NOT NULL,
    project_dir TEXT NOT NULL DEFAULT '',
    status TEXT,
    fingerprint TEXT,
    last_operation_id TEXT,
    last_checked_at TEXT,
    PRIMARY KEY (tool, scope, project_dir)
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_prompts_tool ON prompts(tool);
CREATE INDEX IF NOT EXISTS idx_prompts_updated ON prompts(updated_at);
CREATE INDEX IF NOT EXISTS idx_prompt_versions_prompt ON prompt_versions(prompt_id);
CREATE INDEX IF NOT EXISTS idx_operations_tool ON operations(tool, created_at);
CREATE INDEX IF NOT EXISTS idx_activations_tool ON activations(tool);
"#;

pub fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

pub fn ensure_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub fn current_version(conn: &Connection) -> Result<i64> {
    let version: Option<i64> =
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    Ok(version.unwrap_or(0))
}

pub fn has_core_tables(conn: &Connection) -> bool {
    conn.prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='prompts'")
        .and_then(|mut stmt| stmt.exists([]))
        .unwrap_or(false)
}

pub fn integrity_ok(conn: &Connection) -> bool {
    conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map(|value| value.eq_ignore_ascii_case("ok"))
        .unwrap_or(false)
}

pub fn apply_pending(conn: &Connection, paths: &AppPaths) -> Result<Vec<i64>> {
    ensure_migrations_table(conn)?;
    let current = current_version(conn)?;
    if current >= SCHEMA_VERSION {
        return Ok(Vec::new());
    }
    if current > 0 || has_core_tables(conn) {
        backup_before_migrate(conn, paths, current)?;
    }
    let mut applied = Vec::new();
    if current < 1 {
        conn.execute_batch(MIGRATION_1)?;
        record_version(conn, 1)?;
        applied.push(1);
    }
    Ok(applied)
}

fn record_version(conn: &Connection, version: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        rusqlite::params![version, now_rfc3339()],
    )?;
    Ok(())
}

pub fn backup_before_migrate(conn: &Connection, paths: &AppPaths, from: i64) -> Result<()> {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let dest = paths.db_backup_path(&format!("pre-migrate-{from}-{stamp}"));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    backup_to(conn, &dest)?;
    Ok(())
}

pub fn backup_to(conn: &Connection, dest: &std::path::Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut dest_conn = Connection::open(dest)?;
    {
        let backup = rusqlite::backup::Backup::new(conn, &mut dest_conn)
            .map_err(|error| Error::Db(error.to_string()))?;
        backup
            .run_to_completion(64, Duration::from_millis(5), None)
            .map_err(|error| Error::Db(error.to_string()))?;
    }
    Ok(())
}

pub fn restore_from(source: &Connection, dest: &mut Connection) -> Result<()> {
    let backup = rusqlite::backup::Backup::new(source, dest)
        .map_err(|error| Error::Db(error.to_string()))?;
    backup
        .run_to_completion(64, Duration::from_millis(5), None)
        .map_err(|error| Error::Db(error.to_string()))?;
    Ok(())
}
