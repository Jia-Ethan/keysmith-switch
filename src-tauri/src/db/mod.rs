pub mod markdown;
pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::models::{
    now_rfc3339, Activation, Operation, OperationKind, OperationStatus, PromptDetail, PromptSort,
    PromptSummary, PromptVersion, RebuildReport, Scope, Settings, SettingsPatch, ToolKind,
    ToolStatus,
};
use crate::paths::{atomic_write, AppPaths};
use crate::redact::redact_text;

use self::markdown::{content_sha, parse_file, render_markdown, FrontMatter, MarkdownPrompt};

pub struct Store {
    conn: Mutex<Connection>,
    paths: AppPaths,
}

impl Store {
    pub fn open(paths: &AppPaths) -> Result<Self> {
        paths.ensure()?;
        if paths.db.exists() {
            match try_open(paths) {
                Ok(store) => return Ok(store),
                Err(error) => {
                    quarantine_db(&paths.db, paths)?;
                    let _ = error;
                }
            }
        }
        let conn = open_connection(&paths.db)?;
        schema::apply_pending(&conn, paths)?;
        seed_settings(&conn)?;
        let store = Self {
            conn: Mutex::new(conn),
            paths: paths.clone(),
        };
        store.rebuild_from_markdown()?;
        Ok(store)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.conn()?;
        schema::current_version(&conn)
    }

    pub fn clear_schema_versions_for_test(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM schema_migrations", [])?;
        Ok(())
    }

    pub fn backup_db(&self, label: &str) -> Result<PathBuf> {
        let dest = self.paths.db_backup_path(label);
        let conn = self.conn()?;
        schema::backup_to(&conn, &dest)?;
        Ok(dest)
    }

    pub fn list_prompts(
        &self,
        tool: ToolKind,
        query: Option<&str>,
        tag: Option<&str>,
        sort: PromptSort,
    ) -> Result<Vec<PromptSummary>> {
        let order = match sort {
            PromptSort::Updated => "updated_at DESC, title COLLATE NOCASE ASC",
            PromptSort::Created => "created_at DESC, title COLLATE NOCASE ASC",
            PromptSort::Title => "title COLLATE NOCASE ASC, updated_at DESC",
        };
        let sql = format!(
            "SELECT id, tool, title, tags, version, sha256, created_at, updated_at
             FROM prompts
             WHERE tool = ?1 AND deleted_at IS NULL
             ORDER BY {order}"
        );
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![tool.as_str()], |row| {
            Ok(PromptSummary {
                id: row.get(0)?,
                tool: parse_tool(row.get::<_, String>(1)?)?,
                title: row.get(2)?,
                tags: decode_tags(row.get::<_, String>(3)?),
                version: row.get(4)?,
                sha256: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        let query = query.map(|value| value.trim().to_ascii_lowercase());
        let tag = tag.map(|value| value.trim().to_ascii_lowercase());
        let mut out = Vec::new();
        for row in rows {
            let item = row?;
            if let Some(query) = &query {
                let title_hit = item.title.to_ascii_lowercase().contains(query);
                let tag_hit = item
                    .tags
                    .iter()
                    .any(|tag| tag.to_ascii_lowercase().contains(query));
                if !title_hit && !tag_hit {
                    continue;
                }
            }
            if let Some(tag) = &tag {
                if !item
                    .tags
                    .iter()
                    .any(|item_tag| item_tag.to_ascii_lowercase() == *tag)
                {
                    continue;
                }
            }
            out.push(item);
        }
        Ok(out)
    }

    pub fn get_prompt(&self, id: &str) -> Result<PromptDetail> {
        let conn = self.conn()?;
        let row = conn
            .query_row(
                "SELECT id, tool, title, tags, version, content_path, sha256, created_at, updated_at, deleted_at
                 FROM prompts WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| Error::invalid(format!("prompt not found: {id}")))?;
        let tool = parse_tool(row.1)?;
        let path = self.paths.home.join(&row.5);
        let content = match markdown::parse_file(&path) {
            Ok(parsed) => parsed.content,
            Err(_) => self.latest_version_content(id)?.unwrap_or_default(),
        };
        Ok(PromptDetail {
            id: row.0,
            tool,
            title: row.2,
            content,
            tags: decode_tags(row.3),
            version: row.4,
            sha256: row.6,
            path,
            created_at: row.7,
            updated_at: row.8,
            deleted_at: row.9,
        })
    }

    pub fn insert_prompt(
        &self,
        id: &str,
        tool: ToolKind,
        title: &str,
        content: &str,
        tags: &[String],
        deleted: bool,
    ) -> Result<PromptDetail> {
        let now = now_rfc3339();
        let sha = content_sha(content);
        let rel = format!("prompts/{}/{id}.md", tool.as_str());
        let path = self.paths.prompt_file(tool, id);
        let front = FrontMatter {
            id: id.to_string(),
            tool,
            title: title.to_string(),
            tags: tags.to_vec(),
            version: 1,
            deleted,
        };
        atomic_write(&path, &render_markdown(&front, content))?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO prompts (id, tool, title, tags, version, content_path, sha256, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?7, ?8)",
            params![
                id,
                tool.as_str(),
                title,
                encode_tags(tags),
                rel,
                sha,
                now,
                deleted.then(|| now.clone())
            ],
        )?;
        conn.execute(
            "INSERT INTO prompt_versions (prompt_id, version, title, tags, content, sha256, created_at)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6)",
            params![id, title, encode_tags(tags), content, sha, now],
        )?;
        drop(conn);
        self.get_prompt(id)
    }

    pub fn update_prompt(
        &self,
        id: &str,
        title: Option<&str>,
        content: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<PromptDetail> {
        let current = self.get_prompt(id)?;
        if current.deleted_at.is_some() {
            return Err(Error::invalid(format!("prompt is deleted: {id}")));
        }
        let title = title.unwrap_or(&current.title);
        let body = content.unwrap_or(&current.content);
        let tags = tags.unwrap_or(&current.tags).to_vec();
        let next_version = current.version + 1;
        let now = now_rfc3339();
        let sha = content_sha(body);
        let path = self.paths.prompt_file(current.tool, id);
        let front = FrontMatter {
            id: id.to_string(),
            tool: current.tool,
            title: title.to_string(),
            tags: tags.clone(),
            version: next_version,
            deleted: false,
        };
        atomic_write(&path, &render_markdown(&front, body))?;
        let conn = self.conn()?;
        conn.execute(
            "UPDATE prompts SET title = ?1, tags = ?2, version = ?3, sha256 = ?4, updated_at = ?5
             WHERE id = ?6",
            params![title, encode_tags(&tags), next_version, sha, now, id],
        )?;
        conn.execute(
            "INSERT INTO prompt_versions (prompt_id, version, title, tags, content, sha256, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, next_version, title, encode_tags(&tags), body, sha, now],
        )?;
        drop(conn);
        self.get_prompt(id)
    }

    pub fn soft_delete_prompt(&self, id: &str) -> Result<()> {
        let current = self.get_prompt(id)?;
        if current.deleted_at.is_some() {
            return Ok(());
        }
        let now = now_rfc3339();
        let path = self.paths.prompt_file(current.tool, id);
        let front = FrontMatter {
            id: current.id.clone(),
            tool: current.tool,
            title: current.title.clone(),
            tags: current.tags.clone(),
            version: current.version,
            deleted: true,
        };
        atomic_write(&path, &render_markdown(&front, &current.content))?;
        let conn = self.conn()?;
        conn.execute(
            "UPDATE prompts SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn restore_version(&self, id: &str, version: i64) -> Result<PromptDetail> {
        let current = self.get_prompt(id)?;
        let snapshot = self
            .get_version(id, version)?
            .ok_or_else(|| Error::invalid(format!("version {version} not found for {id}")))?;
        self.update_prompt(
            id,
            Some(&snapshot.title),
            Some(&snapshot.content),
            Some(&snapshot.tags),
        )?;
        let _ = current;
        self.get_prompt(id)
    }

    pub fn list_versions(&self, id: &str) -> Result<Vec<PromptVersion>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT prompt_id, version, title, tags, content, sha256, created_at
             FROM prompt_versions WHERE prompt_id = ?1 ORDER BY version ASC",
        )?;
        let rows = stmt.query_map(params![id], |row| {
            Ok(PromptVersion {
                prompt_id: row.get(0)?,
                version: row.get(1)?,
                title: row.get(2)?,
                tags: decode_tags(row.get::<_, String>(3)?),
                content: row.get(4)?,
                sha256: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn get_version(&self, id: &str, version: i64) -> Result<Option<PromptVersion>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT prompt_id, version, title, tags, content, sha256, created_at
             FROM prompt_versions WHERE prompt_id = ?1 AND version = ?2",
            params![id, version],
            |row| {
                Ok(PromptVersion {
                    prompt_id: row.get(0)?,
                    version: row.get(1)?,
                    title: row.get(2)?,
                    tags: decode_tags(row.get::<_, String>(3)?),
                    content: row.get(4)?,
                    sha256: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    pub fn insert_operation(&self, operation: &Operation) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO operations (
                id, tool, kind, status, preview, prompt_id, scope, project_dir,
                request_json, envelope_json, error, parent_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                operation.id,
                operation.tool.as_str(),
                operation.kind.as_str(),
                operation.status.as_str(),
                operation.preview as i64,
                operation.prompt_id,
                operation.scope.map(|scope| scope.as_str().to_string()),
                operation.project_dir,
                redact_text(&operation.request_json),
                operation.envelope_json.as_deref().map(redact_text),
                operation.error.as_deref().map(redact_text),
                operation.parent_id,
                operation.created_at,
                operation.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn update_operation(
        &self,
        id: &str,
        status: OperationStatus,
        envelope_json: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE operations
             SET status = ?1, envelope_json = ?2, error = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                status.as_str(),
                envelope_json.map(redact_text),
                error.map(redact_text),
                now_rfc3339(),
                id
            ],
        )?;
        Ok(())
    }

    pub fn get_operation(&self, id: &str) -> Result<Option<Operation>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, tool, kind, status, preview, prompt_id, scope, project_dir,
                    request_json, envelope_json, error, parent_id, created_at, updated_at
             FROM operations WHERE id = ?1",
            params![id],
            map_operation,
        )
        .optional()
        .map_err(Error::from)
    }

    pub fn list_operations(&self, tool: Option<ToolKind>) -> Result<Vec<Operation>> {
        let conn = self.conn()?;
        if let Some(tool) = tool {
            let mut stmt = conn.prepare(
                "SELECT id, tool, kind, status, preview, prompt_id, scope, project_dir,
                        request_json, envelope_json, error, parent_id, created_at, updated_at
                 FROM operations WHERE tool = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![tool.as_str()], map_operation)?;
            return rows
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Error::from);
        }
        let mut stmt = conn.prepare(
            "SELECT id, tool, kind, status, preview, prompt_id, scope, project_dir,
                    request_json, envelope_json, error, parent_id, created_at, updated_at
             FROM operations ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_operation)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn upsert_activation(&self, activation: &Activation) -> Result<()> {
        let conn = self.conn()?;
        let project_dir = activation.project_dir.clone().unwrap_or_default();
        conn.execute(
            "INSERT INTO activations (
                id, prompt_id, tool, scope, project_dir, status, fingerprint, operation_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                prompt_id = excluded.prompt_id,
                status = excluded.status,
                fingerprint = excluded.fingerprint,
                operation_id = excluded.operation_id,
                updated_at = excluded.updated_at",
            params![
                activation.id,
                activation.prompt_id,
                activation.tool.as_str(),
                activation.scope.as_str(),
                project_dir,
                activation.status.as_str(),
                activation.fingerprint,
                activation.operation_id,
                activation.created_at,
                activation.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn find_activation(
        &self,
        tool: ToolKind,
        scope: Scope,
        project_dir: Option<&str>,
    ) -> Result<Option<Activation>> {
        let conn = self.conn()?;
        let project_dir = project_dir.unwrap_or("");
        conn.query_row(
            "SELECT id, prompt_id, tool, scope, project_dir, status, fingerprint, operation_id, created_at, updated_at
             FROM activations
             WHERE tool = ?1 AND scope = ?2 AND project_dir = ?3
             ORDER BY updated_at DESC LIMIT 1",
            params![tool.as_str(), scope.as_str(), project_dir],
            map_activation,
        )
        .optional()
        .map_err(Error::from)
    }

    pub fn list_activations(&self, tool: ToolKind) -> Result<Vec<Activation>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, prompt_id, tool, scope, project_dir, status, fingerprint, operation_id, created_at, updated_at
             FROM activations WHERE tool = ?1 ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![tool.as_str()], map_activation)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn upsert_tool_state(
        &self,
        tool: ToolKind,
        scope: Scope,
        project_dir: Option<&str>,
        status: ToolStatus,
        fingerprint: Option<&str>,
        operation_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        let project_dir = project_dir.unwrap_or("");
        conn.execute(
            "INSERT INTO tool_state (tool, scope, project_dir, status, fingerprint, last_operation_id, last_checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(tool, scope, project_dir) DO UPDATE SET
                status = excluded.status,
                fingerprint = excluded.fingerprint,
                last_operation_id = excluded.last_operation_id,
                last_checked_at = excluded.last_checked_at",
            params![
                tool.as_str(),
                scope.as_str(),
                project_dir,
                status.as_str(),
                fingerprint,
                operation_id,
                now_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_settings(&self) -> Result<Settings> {
        let conn = self.conn()?;
        let mut settings = Settings::default();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            apply_setting(&mut settings, &key, &value);
        }
        Ok(settings)
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Result<Settings> {
        let mut settings = self.get_settings()?;
        if let Some(language) = patch.language {
            settings.language = language;
        }
        if let Some(channel) = patch.update_channel {
            settings.update_channel = channel;
        }
        if let Some(enabled) = patch.advanced_tools_enabled {
            settings.advanced_tools_enabled = enabled;
        }
        if let Some(scope) = patch.default_claude_scope {
            settings.default_claude_scope = scope;
        }
        if let Some(dirs) = patch.recent_project_dirs {
            settings.recent_project_dirs = dirs;
        }
        if let Some(endpoint) = patch.updater_endpoint_override {
            settings.updater_endpoint_override = endpoint;
        }
        let conn = self.conn()?;
        write_settings(&conn, &settings)?;
        Ok(settings)
    }

    pub fn rebuild_from_markdown(&self) -> Result<RebuildReport> {
        let mut report = RebuildReport {
            scanned: 0,
            imported: 0,
            updated: 0,
            deleted: 0,
        };
        if !self.paths.prompts.exists() {
            return Ok(report);
        }
        let mut seen = Vec::new();
        for entry in WalkDir::new(&self.paths.prompts).into_iter().flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            report.scanned += 1;
            let parsed = match parse_file(path) {
                Ok(parsed) => parsed,
                Err(_) => continue,
            };
            seen.push(parsed.front.id.clone());
            match self.upsert_from_markdown(path, &parsed)? {
                UpsertKind::Imported => report.imported += 1,
                UpsertKind::Updated => report.updated += 1,
                UpsertKind::Unchanged => {}
            }
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id FROM prompts WHERE deleted_at IS NULL")?;
        let live = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let now = now_rfc3339();
        for id in live {
            if !seen.iter().any(|item| item == &id) {
                conn.execute(
                    "UPDATE prompts SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                    params![now, id],
                )?;
                report.deleted += 1;
            }
        }
        Ok(report)
    }

    fn upsert_from_markdown(&self, path: &Path, parsed: &MarkdownPrompt) -> Result<UpsertKind> {
        let rel = path
            .strip_prefix(&self.paths.home)
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
        let sha = content_sha(&parsed.content);
        let conn = self.conn()?;
        let existing = conn
            .query_row(
                "SELECT version, sha256, deleted_at FROM prompts WHERE id = ?1",
                params![parsed.front.id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let now = now_rfc3339();
        let deleted_at = parsed.front.deleted.then(|| now.clone());
        match existing {
            None => {
                conn.execute(
                    "INSERT INTO prompts (id, tool, title, tags, version, content_path, sha256, created_at, updated_at, deleted_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)",
                    params![
                        parsed.front.id,
                        parsed.front.tool.as_str(),
                        parsed.front.title,
                        encode_tags(&parsed.front.tags),
                        parsed.front.version,
                        rel,
                        sha,
                        now,
                        deleted_at
                    ],
                )?;
                conn.execute(
                    "INSERT INTO prompt_versions (prompt_id, version, title, tags, content, sha256, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        parsed.front.id,
                        parsed.front.version,
                        parsed.front.title,
                        encode_tags(&parsed.front.tags),
                        parsed.content,
                        sha,
                        now
                    ],
                )?;
                Ok(UpsertKind::Imported)
            }
            Some((version, existing_sha, existing_deleted)) => {
                if existing_sha == sha
                    && existing_deleted.is_some() == parsed.front.deleted
                    && version == parsed.front.version
                {
                    return Ok(UpsertKind::Unchanged);
                }
                let next_version = parsed.front.version.max(version);
                let history_version = if existing_sha == sha {
                    next_version
                } else {
                    next_version.max(version + 1)
                };
                conn.execute(
                    "UPDATE prompts
                     SET tool = ?1, title = ?2, tags = ?3, version = ?4, content_path = ?5,
                         sha256 = ?6, updated_at = ?7, deleted_at = ?8
                     WHERE id = ?9",
                    params![
                        parsed.front.tool.as_str(),
                        parsed.front.title,
                        encode_tags(&parsed.front.tags),
                        history_version,
                        rel,
                        sha,
                        now,
                        deleted_at,
                        parsed.front.id
                    ],
                )?;
                if existing_sha != sha {
                    conn.execute(
                        "INSERT OR IGNORE INTO prompt_versions (prompt_id, version, title, tags, content, sha256, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            parsed.front.id,
                            history_version,
                            parsed.front.title,
                            encode_tags(&parsed.front.tags),
                            parsed.content,
                            sha,
                            now
                        ],
                    )?;
                }
                Ok(UpsertKind::Updated)
            }
        }
    }

    fn latest_version_content(&self, id: &str) -> Result<Option<String>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT content FROM prompt_versions WHERE prompt_id = ?1 ORDER BY version DESC LIMIT 1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Error::from)
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| Error::db("sqlite connection lock poisoned"))
    }
}

enum UpsertKind {
    Imported,
    Updated,
    Unchanged,
}

impl Error {
    fn db(msg: impl Into<String>) -> Self {
        Error::Db(msg.into())
    }
}

fn try_open(paths: &AppPaths) -> Result<Store> {
    let conn = open_connection(&paths.db)?;
    if !schema::integrity_ok(&conn) {
        return Err(Error::db("sqlite integrity check failed"));
    }
    schema::apply_pending(&conn, paths)?;
    if !schema::has_core_tables(&conn) {
        return Err(Error::db("sqlite schema is incomplete"));
    }
    seed_settings(&conn)?;
    Ok(Store {
        conn: Mutex::new(conn),
        paths: paths.clone(),
    })
}

fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    schema::configure(&conn)?;
    Ok(conn)
}

fn quarantine_db(path: &Path, paths: &AppPaths) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let dest = paths.db_backup_path(&format!("corrupt-{stamp}"));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(path, &dest).or_else(|_| {
        fs::copy(path, &dest)?;
        fs::remove_file(path)
    })?;
    let _ = fs::remove_file(path.with_extension("db-wal"));
    let _ = fs::remove_file(path.with_extension("db-shm"));
    Ok(())
}

fn seed_settings(conn: &Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))?;
    if count == 0 {
        write_settings(conn, &Settings::default())?;
    }
    Ok(())
}

fn write_settings(conn: &Connection, settings: &Settings) -> Result<()> {
    let pairs = [
        ("language", settings.language.clone()),
        ("updateChannel", settings.update_channel.clone()),
        (
            "advancedToolsEnabled",
            settings.advanced_tools_enabled.to_string(),
        ),
        (
            "defaultClaudeScope",
            settings.default_claude_scope.as_str().to_string(),
        ),
        (
            "recentProjectDirs",
            serde_json::to_string(&settings.recent_project_dirs)?,
        ),
        (
            "updaterEndpointOverride",
            settings
                .updater_endpoint_override
                .clone()
                .unwrap_or_default(),
        ),
    ];
    for (key, value) in pairs {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
    }
    Ok(())
}

fn apply_setting(settings: &mut Settings, key: &str, value: &str) {
    match key {
        "language" => settings.language = value.to_string(),
        "updateChannel" => settings.update_channel = value.to_string(),
        "advancedToolsEnabled" => {
            settings.advanced_tools_enabled = matches!(value, "true" | "1" | "yes")
        }
        "defaultClaudeScope" => {
            if let Ok(scope) = value.parse() {
                settings.default_claude_scope = scope;
            }
        }
        "recentProjectDirs" => {
            if let Ok(dirs) = serde_json::from_str::<Vec<String>>(value) {
                settings.recent_project_dirs = dirs;
            }
        }
        "updaterEndpointOverride" => {
            settings.updater_endpoint_override = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        _ => {}
    }
}

fn encode_tags(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

fn decode_tags(raw: String) -> Vec<String> {
    serde_json::from_str(&raw).unwrap_or_default()
}

fn parse_tool(raw: String) -> std::result::Result<ToolKind, rusqlite::Error> {
    raw.parse::<ToolKind>()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn parse_scope_opt(raw: Option<String>) -> std::result::Result<Option<Scope>, rusqlite::Error> {
    match raw {
        None => Ok(None),
        Some(value) => value
            .parse::<Scope>()
            .map(Some)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error))),
    }
}

fn parse_kind(raw: String) -> std::result::Result<OperationKind, rusqlite::Error> {
    raw.parse::<OperationKind>()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn parse_op_status(raw: String) -> std::result::Result<OperationStatus, rusqlite::Error> {
    raw.parse::<OperationStatus>()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn parse_tool_status(raw: String) -> std::result::Result<ToolStatus, rusqlite::Error> {
    raw.parse::<ToolStatus>()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn map_operation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    Ok(Operation {
        id: row.get(0)?,
        tool: parse_tool(row.get(1)?)?,
        kind: parse_kind(row.get(2)?)?,
        status: parse_op_status(row.get(3)?)?,
        preview: row.get::<_, i64>(4)? != 0,
        prompt_id: row.get(5)?,
        scope: parse_scope_opt(row.get(6)?)?,
        project_dir: row.get(7)?,
        request_json: row.get(8)?,
        envelope_json: row.get(9)?,
        error: row.get(10)?,
        parent_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn map_activation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Activation> {
    let project_dir: String = row.get(4)?;
    Ok(Activation {
        id: row.get(0)?,
        prompt_id: row.get(1)?,
        tool: parse_tool(row.get(2)?)?,
        scope: parse_scope_opt(Some(row.get(3)?))?.unwrap_or(Scope::User),
        project_dir: if project_dir.is_empty() {
            None
        } else {
            Some(project_dir)
        },
        status: parse_tool_status(row.get(5)?)?,
        fingerprint: row.get(6)?,
        operation_id: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}
