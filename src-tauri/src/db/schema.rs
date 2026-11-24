use std::time::Duration;

use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::models::now_rfc3339;
use crate::paths::AppPaths;

pub const SCHEMA_VERSION: i64 = 6;
/// The oldest schema a backup may carry. Restoring one migrates it forward.
pub const MIN_RESTORABLE_SCHEMA: i64 = 1;

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

/// Input rewrite rules. `pack_*` columns describe a table an extension pack installed;
/// `pending_json` holds a pack update that waits for the person to accept it.
const MIGRATION_2: &str = r#"
CREATE TABLE IF NOT EXISTS rule_tables (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    pack_id TEXT,
    pack_version TEXT,
    pending_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_id TEXT NOT NULL REFERENCES rule_tables(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    from_text TEXT NOT NULL,
    to_text TEXT NOT NULL,
    UNIQUE(table_id, from_text)
);

CREATE INDEX IF NOT EXISTS idx_rules_table ON rules(table_id, position);
"#;

/// Which agents a rule table applies to: a JSON array of tool ids, or NULL for every agent.
const MIGRATION_3: &str = r#"
ALTER TABLE rule_tables ADD COLUMN tools_json TEXT;
"#;

/// An opaque pack table keeps its rules out of the interface.
const MIGRATION_4: &str = r#"
ALTER TABLE rule_tables ADD COLUMN opaque INTEGER NOT NULL DEFAULT 0;
"#;

const MIGRATION_5_TOOL_STATE: &str = r#"
CREATE TABLE tool_state_v5 (
    tool TEXT NOT NULL,
    runtime_target TEXT NOT NULL DEFAULT 'native',
    scope TEXT NOT NULL,
    project_dir TEXT NOT NULL DEFAULT '',
    status TEXT,
    fingerprint TEXT,
    last_operation_id TEXT,
    last_checked_at TEXT,
    PRIMARY KEY (tool, runtime_target, scope, project_dir)
);
INSERT INTO tool_state_v5 (
    tool, runtime_target, scope, project_dir, status, fingerprint, last_operation_id, last_checked_at
)
SELECT tool, 'native', scope, project_dir, status, fingerprint, last_operation_id, last_checked_at
FROM tool_state;
DROP TABLE tool_state;
ALTER TABLE tool_state_v5 RENAME TO tool_state;
"#;

const MIGRATION_6_REWRITE_RUNTIME: &str = r#"
CREATE TABLE rule_tables_v6 (
    id TEXT NOT NULL,
    runtime_target TEXT NOT NULL DEFAULT 'native',
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    pack_id TEXT,
    pack_version TEXT,
    pending_json TEXT,
    tools_json TEXT,
    opaque INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (id, runtime_target)
);

INSERT INTO rule_tables_v6 (
    id, runtime_target, kind, title, enabled, priority, pack_id, pack_version,
    pending_json, tools_json, opaque, created_at, updated_at
)
SELECT id, 'native', kind, title, enabled, priority, pack_id, pack_version,
       pending_json, tools_json, opaque, created_at, updated_at
FROM rule_tables;

CREATE TABLE rules_v6 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_id TEXT NOT NULL,
    runtime_target TEXT NOT NULL DEFAULT 'native',
    position INTEGER NOT NULL,
    from_text TEXT NOT NULL,
    to_text TEXT NOT NULL,
    FOREIGN KEY (table_id, runtime_target)
        REFERENCES rule_tables_v6(id, runtime_target) ON DELETE CASCADE,
    UNIQUE(table_id, runtime_target, from_text)
);

INSERT INTO rules_v6 (id, table_id, runtime_target, position, from_text, to_text)
SELECT id, table_id, 'native', position, from_text, to_text FROM rules;

DROP TABLE rules;
DROP TABLE rule_tables;
ALTER TABLE rule_tables_v6 RENAME TO rule_tables;
ALTER TABLE rules_v6 RENAME TO rules;
CREATE INDEX idx_rules_table ON rules(table_id, runtime_target, position);
CREATE INDEX idx_rule_tables_runtime ON rule_tables(runtime_target, kind, priority);

CREATE TABLE IF NOT EXISTS rewrite_profiles (
    runtime_target TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 0,
    codex_enabled INTEGER NOT NULL DEFAULT 1,
    claude_enabled INTEGER NOT NULL DEFAULT 1,
    grok_enabled INTEGER NOT NULL DEFAULT 1,
    zcode_enabled INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO rewrite_profiles (
    runtime_target, enabled, codex_enabled, claude_enabled, grok_enabled,
    zcode_enabled, updated_at
)
SELECT
    'native',
    COALESCE((SELECT value IN ('true', '1', 'yes') FROM settings WHERE key = 'rewriteEnabled'), 0),
    COALESCE((SELECT value IN ('true', '1', 'yes') FROM settings WHERE key = 'rewriteCodexEnabled'), 1),
    COALESCE((SELECT value IN ('true', '1', 'yes') FROM settings WHERE key = 'rewriteClaudeEnabled'), 1),
    COALESCE((SELECT value IN ('true', '1', 'yes') FROM settings WHERE key = 'rewriteGrokEnabled'), 1),
    COALESCE((SELECT value IN ('true', '1', 'yes') FROM settings WHERE key = 'rewriteZcodeEnabled'), 1),
    CURRENT_TIMESTAMP;
"#;

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn apply_migration_5(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "activations", "runtime_target")? {
        conn.execute_batch(
            "ALTER TABLE activations ADD COLUMN runtime_target TEXT NOT NULL DEFAULT 'native';",
        )?;
    }
    if !column_exists(conn, "operations", "runtime_target")? {
        conn.execute_batch(
            "ALTER TABLE operations ADD COLUMN runtime_target TEXT NOT NULL DEFAULT 'native';",
        )?;
    }
    if !column_exists(conn, "tool_state", "runtime_target")? {
        conn.execute_batch(MIGRATION_5_TOOL_STATE)?;
    }
    Ok(())
}

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
    if current < 2 {
        conn.execute_batch(MIGRATION_2)?;
        record_version(conn, 2)?;
        applied.push(2);
    }
    if current < 3 {
        conn.execute_batch(MIGRATION_3)?;
        record_version(conn, 3)?;
        applied.push(3);
    }
    if current < 4 {
        conn.execute_batch(MIGRATION_4)?;
        record_version(conn, 4)?;
        applied.push(4);
    }
    if current < 5 {
        apply_migration_5(conn)?;
        record_version(conn, 5)?;
        applied.push(5);
    }
    if current < 6 {
        conn.execute_batch(MIGRATION_6_REWRITE_RUNTIME)?;
        record_version(conn, 6)?;
        applied.push(6);
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
