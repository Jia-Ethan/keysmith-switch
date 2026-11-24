//! Input rewrite rule tables. "My rules" is the one table the person edits; every other
//! table came from an extension pack and is read-only here.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use keysmith_rewrite::{validate_table, Rule, Tool};

use super::Store;
use crate::error::{Error, Result};
use crate::models::{now_rfc3339, RuntimeTarget};

pub const USER_TABLE_ID: &str = "user";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TableKind {
    User,
    Pack,
}

impl TableKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Pack => "pack",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleTable {
    pub id: String,
    pub kind: TableKind,
    pub title: String,
    pub enabled: bool,
    pub priority: i64,
    pub pack_id: Option<String>,
    pub pack_version: Option<String>,
    /// The agents this table applies to; `None` means every agent.
    pub tools: Option<Vec<Tool>>,
    /// True when the pack asked the app not to show `from` / `to` in the interface.
    pub opaque: bool,
    /// How many rules the table holds, including when [`Self::rules`] is empty for an opaque table.
    pub rule_count: usize,
    pub rules: Vec<Rule>,
    /// A pack update that waits for the person to accept it.
    pub pending: Option<PendingUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewriteSwitches {
    pub enabled: bool,
    pub codex_enabled: bool,
    pub claude_enabled: bool,
    pub grok_enabled: bool,
    pub zcode_enabled: bool,
}

impl Default for RewriteSwitches {
    fn default() -> Self {
        Self {
            enabled: false,
            codex_enabled: true,
            claude_enabled: true,
            grok_enabled: true,
            zcode_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingUpdate {
    pub version: String,
    pub title: String,
    pub rules: Vec<Rule>,
    /// How many rules the pending version holds; set when the view is built.
    #[serde(default)]
    pub rule_count: usize,
    #[serde(default)]
    pub opaque: bool,
}

impl RuleTable {
    pub fn applies_to(&self, tool: Tool) -> bool {
        self.tools
            .as_ref()
            .is_none_or(|tools| tools.contains(&tool))
    }

    /// Fill counts, then drop rule text the interface must not show.
    pub fn seal_for_view(&mut self) {
        self.rule_count = self.rules.len();
        if self.opaque {
            self.rules.clear();
        }
        if let Some(pending) = &mut self.pending {
            pending.rule_count = pending.rules.len();
            if pending.opaque || self.opaque {
                pending.rules.clear();
            }
        }
    }
}

fn parse_tools(text: &str) -> Option<Vec<Tool>> {
    let tools: Vec<Tool> = serde_json::from_str(text).ok()?;
    (!tools.is_empty()).then_some(tools)
}

/// Tools in a fixed order, so the stored text only changes when the set does.
fn tools_json(tools: &[Tool]) -> String {
    let mut sorted = tools.to_vec();
    sorted.sort();
    sorted.dedup();
    serde_json::to_string(&sorted).unwrap_or_else(|_| "[]".into())
}

/// A pack naming every agent applies to every agent, including ones added later.
fn pack_tools_json(tools: &[Tool]) -> Option<String> {
    let all = Tool::ALL.iter().all(|tool| tools.contains(tool));
    (!all && !tools.is_empty()).then(|| tools_json(tools))
}

fn invalid_rules(error: keysmith_rewrite::RuleError) -> Error {
    Error::invalid(error.to_string())
}

impl Store {
    /// Every table, "My rules" first and the packs after it in priority order.
    pub fn list_rule_tables(&self) -> Result<Vec<RuleTable>> {
        self.list_rule_tables_for(&RuntimeTarget::Native)
    }

    pub fn list_rule_tables_for(&self, runtime_target: &RuntimeTarget) -> Result<Vec<RuleTable>> {
        let runtime_key = runtime_target.storage_key();
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, title, enabled, priority, pack_id, pack_version, pending_json, tools_json, opaque
             FROM rule_tables
             WHERE runtime_target = ?1
             ORDER BY CASE kind WHEN 'user' THEN 0 ELSE 1 END, priority, created_at",
        )?;
        let mut tables = stmt
            .query_map(params![runtime_key], |row| {
                let kind: String = row.get(1)?;
                let pending: Option<String> = row.get(7)?;
                let tools: Option<String> = row.get(8)?;
                let opaque: i64 = row.get(9)?;
                Ok(RuleTable {
                    id: row.get(0)?,
                    kind: if kind == "user" {
                        TableKind::User
                    } else {
                        TableKind::Pack
                    },
                    title: row.get(2)?,
                    enabled: row.get::<_, i64>(3)? != 0,
                    priority: row.get(4)?,
                    pack_id: row.get(5)?,
                    pack_version: row.get(6)?,
                    tools: tools.as_deref().and_then(parse_tools),
                    opaque: opaque != 0,
                    rule_count: 0,
                    rules: Vec::new(),
                    pending: pending.and_then(|text| serde_json::from_str(&text).ok()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut rules = conn.prepare(
            "SELECT from_text, to_text FROM rules
             WHERE table_id = ?1 AND runtime_target = ?2 ORDER BY position",
        )?;
        for table in &mut tables {
            table.rules = rules
                .query_map(params![table.id, runtime_key], |row| {
                    Ok(Rule::new(
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            table.rule_count = table.rules.len();
            if let Some(pending) = &mut table.pending {
                pending.rule_count = pending.rules.len();
            }
        }
        if !tables.iter().any(|table| table.kind == TableKind::User) {
            tables.insert(
                0,
                RuleTable {
                    id: USER_TABLE_ID.into(),
                    kind: TableKind::User,
                    title: String::new(),
                    enabled: true,
                    priority: 0,
                    pack_id: None,
                    pack_version: None,
                    tools: None,
                    opaque: false,
                    rule_count: 0,
                    rules: Vec::new(),
                    pending: None,
                },
            );
        }
        Ok(tables)
    }

    /// The rules of every enabled table that applies to `tool`, highest priority first.
    pub fn active_rules(&self, tool: Tool) -> Result<Vec<Rule>> {
        self.active_rules_for(tool, &RuntimeTarget::Native)
    }

    pub fn active_rules_for(
        &self,
        tool: Tool,
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<Rule>> {
        Ok(self
            .list_rule_tables_for(runtime_target)?
            .into_iter()
            .filter(|table| table.enabled && table.applies_to(tool))
            .flat_map(|table| table.rules)
            .collect())
    }

    /// Narrow a table to some agents, or `None` for every agent. An empty list is refused:
    /// turn the table off instead.
    pub fn set_rule_table_tools(&self, id: &str, tools: Option<&[Tool]>) -> Result<Vec<RuleTable>> {
        self.set_rule_table_tools_for(id, tools, &RuntimeTarget::Native)
    }

    pub fn set_rule_table_tools_for(
        &self,
        id: &str,
        tools: Option<&[Tool]>,
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<RuleTable>> {
        if tools.is_some_and(<[Tool]>::is_empty) {
            return Err(Error::invalid(
                "a rule table must apply to at least one agent",
            ));
        }
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            if id == USER_TABLE_ID {
                ensure_user_table(&tx, runtime_target)?;
            }
            let changed = tx.execute(
                "UPDATE rule_tables SET tools_json = ?3, updated_at = ?4
                 WHERE id = ?1 AND runtime_target = ?2",
                params![
                    id,
                    runtime_target.storage_key(),
                    tools.map(tools_json),
                    now_rfc3339()
                ],
            )?;
            if changed == 0 {
                return Err(Error::invalid("rule table not found"));
            }
            tx.commit()?;
        }
        self.list_rule_tables_for(runtime_target)
    }

    /// Replace "My rules" with these, in this order.
    pub fn save_user_rules(&self, rules: &[Rule]) -> Result<Vec<RuleTable>> {
        self.save_user_rules_for(rules, &RuntimeTarget::Native)
    }

    pub fn save_user_rules_for(
        &self,
        rules: &[Rule],
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<RuleTable>> {
        validate_table(rules).map_err(invalid_rules)?;
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            ensure_user_table(&tx, runtime_target)?;
            replace_rules(&tx, USER_TABLE_ID, runtime_target, rules)?;
            touch(&tx, USER_TABLE_ID, runtime_target)?;
            tx.commit()?;
        }
        self.list_rule_tables_for(runtime_target)
    }

    pub fn set_rule_table_enabled(&self, id: &str, enabled: bool) -> Result<Vec<RuleTable>> {
        self.set_rule_table_enabled_for(id, enabled, &RuntimeTarget::Native)
    }

    pub fn set_rule_table_enabled_for(
        &self,
        id: &str,
        enabled: bool,
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<RuleTable>> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            if id == USER_TABLE_ID {
                ensure_user_table(&tx, runtime_target)?;
            }
            let changed = tx.execute(
                "UPDATE rule_tables SET enabled = ?3, updated_at = ?4
                 WHERE id = ?1 AND runtime_target = ?2",
                params![
                    id,
                    runtime_target.storage_key(),
                    enabled as i64,
                    now_rfc3339()
                ],
            )?;
            if changed == 0 {
                return Err(Error::invalid("rule table not found"));
            }
            tx.commit()?;
        }
        self.list_rule_tables_for(runtime_target)
    }

    /// Put the pack tables in this order. "My rules" always stays first.
    pub fn reorder_rule_tables(&self, ids: &[String]) -> Result<Vec<RuleTable>> {
        self.reorder_rule_tables_for(ids, &RuntimeTarget::Native)
    }

    pub fn reorder_rule_tables_for(
        &self,
        ids: &[String],
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<RuleTable>> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            for (index, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE rule_tables SET priority = ?3
                     WHERE id = ?1 AND runtime_target = ?2 AND kind = 'pack'",
                    params![id, runtime_target.storage_key(), index as i64 + 1],
                )?;
            }
            tx.commit()?;
        }
        self.list_rule_tables_for(runtime_target)
    }

    /// Append the rules of a pack table to "My rules", skipping text "My rules" already matches.
    pub fn copy_rules_to_user(&self, id: &str) -> Result<Vec<RuleTable>> {
        self.copy_rules_to_user_for(id, &RuntimeTarget::Native)
    }

    pub fn copy_rules_to_user_for(
        &self,
        id: &str,
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<RuleTable>> {
        let tables = self.list_rule_tables_for(runtime_target)?;
        let source = tables
            .iter()
            .find(|table| table.id == id && table.kind == TableKind::Pack)
            .ok_or_else(|| Error::invalid("rule table not found"))?;
        if source.opaque {
            return Err(Error::invalid(
                "this rule pack keeps its rules hidden; it cannot be copied",
            ));
        }
        let mut mine = tables
            .iter()
            .find(|table| table.kind == TableKind::User)
            .map(|table| table.rules.clone())
            .unwrap_or_default();
        for rule in &source.rules {
            if !mine.iter().any(|existing| existing.from == rule.from) {
                mine.push(rule.clone());
            }
        }
        self.save_user_rules_for(&mine, runtime_target)
    }

    /// Install a pack table or update one that is not enabled. An enabled table keeps its
    /// rules and holds the new version as pending until [`Store::accept_pack_update`].
    /// A new table applies to the agents the pack names; after that the person's choice of
    /// agents stays. Returns true when the update is pending.
    pub fn upsert_pack_rules(
        &self,
        pack_id: &str,
        version: &str,
        title: &str,
        rules: &[Rule],
        tools: &[Tool],
        enable: bool,
        opaque: bool,
    ) -> Result<bool> {
        self.upsert_pack_rules_for(
            pack_id,
            version,
            title,
            rules,
            tools,
            enable,
            opaque,
            &RuntimeTarget::Native,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_pack_rules_for(
        &self,
        pack_id: &str,
        version: &str,
        title: &str,
        rules: &[Rule],
        tools: &[Tool],
        enable: bool,
        opaque: bool,
        runtime_target: &RuntimeTarget,
    ) -> Result<bool> {
        validate_table(rules).map_err(invalid_rules)?;
        let id = pack_table_id(pack_id);
        let runtime_key = runtime_target.storage_key();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let existing: Option<(bool, Option<String>)> = tx
            .query_row(
                "SELECT enabled, pack_version FROM rule_tables
                 WHERE id = ?1 AND runtime_target = ?2",
                params![id, runtime_key],
                |row| Ok((row.get::<_, i64>(0)? != 0, row.get(1)?)),
            )
            .optional()?;
        let pending = match existing {
            None => {
                let priority: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(priority), 0) + 1 FROM rule_tables
                     WHERE runtime_target = ?1 AND kind = 'pack'",
                    params![runtime_key],
                    |row| row.get(0),
                )?;
                let now = now_rfc3339();
                tx.execute(
                    "INSERT INTO rule_tables
                     (id, runtime_target, kind, title, enabled, priority, pack_id, pack_version,
                      tools_json, opaque, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                    params![
                        id,
                        runtime_key,
                        TableKind::Pack.as_str(),
                        title,
                        enable as i64,
                        priority,
                        pack_id,
                        version,
                        pack_tools_json(tools),
                        opaque as i64,
                        now
                    ],
                )?;
                replace_rules(&tx, &id, runtime_target, rules)?;
                false
            }
            Some((enabled, current)) if enabled && current.as_deref() != Some(version) => {
                let pending = PendingUpdate {
                    version: version.into(),
                    title: title.into(),
                    rules: rules.to_vec(),
                    rule_count: rules.len(),
                    opaque,
                };
                tx.execute(
                    "UPDATE rule_tables SET pending_json = ?3, updated_at = ?4
                     WHERE id = ?1 AND runtime_target = ?2",
                    params![
                        id,
                        runtime_key,
                        serde_json::to_string(&pending)?,
                        now_rfc3339()
                    ],
                )?;
                true
            }
            Some(_) => {
                tx.execute(
                    "UPDATE rule_tables
                     SET title = ?3, pack_version = ?4, pending_json = NULL,
                         enabled = MAX(enabled, ?5), opaque = ?6, updated_at = ?7
                     WHERE id = ?1 AND runtime_target = ?2",
                    params![
                        id,
                        runtime_key,
                        title,
                        version,
                        enable as i64,
                        opaque as i64,
                        now_rfc3339()
                    ],
                )?;
                replace_rules(&tx, &id, runtime_target, rules)?;
                false
            }
        };
        tx.commit()?;
        Ok(pending)
    }

    /// Switch an enabled pack table to the update it was holding.
    pub fn accept_pack_update(&self, id: &str) -> Result<Vec<RuleTable>> {
        self.accept_pack_update_for(id, &RuntimeTarget::Native)
    }

    pub fn accept_pack_update_for(
        &self,
        id: &str,
        runtime_target: &RuntimeTarget,
    ) -> Result<Vec<RuleTable>> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let pending: Option<String> = tx
                .query_row(
                    "SELECT pending_json FROM rule_tables
                     WHERE id = ?1 AND runtime_target = ?2 AND kind = 'pack'",
                    params![id, runtime_target.storage_key()],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            let pending: PendingUpdate = pending
                .and_then(|text| serde_json::from_str(&text).ok())
                .ok_or_else(|| Error::invalid("no pending update for this rule table"))?;
            validate_table(&pending.rules).map_err(invalid_rules)?;
            tx.execute(
                "UPDATE rule_tables
                 SET title = ?3, pack_version = ?4, pending_json = NULL,
                     opaque = ?5, updated_at = ?6
                 WHERE id = ?1 AND runtime_target = ?2",
                params![
                    id,
                    runtime_target.storage_key(),
                    pending.title,
                    pending.version,
                    pending.opaque as i64,
                    now_rfc3339()
                ],
            )?;
            replace_rules(&tx, id, runtime_target, &pending.rules)?;
            tx.commit()?;
        }
        self.list_rule_tables_for(runtime_target)
    }

    pub fn remove_pack_rules(&self, pack_id: &str) -> Result<()> {
        self.remove_pack_rules_for(pack_id, &RuntimeTarget::Native)
    }

    pub fn remove_pack_rules_for(
        &self,
        pack_id: &str,
        runtime_target: &RuntimeTarget,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM rule_tables
             WHERE id = ?1 AND runtime_target = ?2 AND kind = 'pack'",
            params![pack_table_id(pack_id), runtime_target.storage_key()],
        )?;
        Ok(())
    }

    pub fn rewrite_switches(&self, runtime_target: &RuntimeTarget) -> Result<RewriteSwitches> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT enabled, codex_enabled, claude_enabled, grok_enabled, zcode_enabled
             FROM rewrite_profiles WHERE runtime_target = ?1",
            params![runtime_target.storage_key()],
            |row| {
                Ok(RewriteSwitches {
                    enabled: row.get::<_, i64>(0)? != 0,
                    codex_enabled: row.get::<_, i64>(1)? != 0,
                    claude_enabled: row.get::<_, i64>(2)? != 0,
                    grok_enabled: row.get::<_, i64>(3)? != 0,
                    zcode_enabled: row.get::<_, i64>(4)? != 0,
                })
            },
        )
        .optional()
        .map(|value| value.unwrap_or_default())
        .map_err(Error::from)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_rewrite_switches(
        &self,
        runtime_target: &RuntimeTarget,
        enabled: Option<bool>,
        codex_enabled: Option<bool>,
        claude_enabled: Option<bool>,
        grok_enabled: Option<bool>,
        zcode_enabled: Option<bool>,
    ) -> Result<RewriteSwitches> {
        let current = self.rewrite_switches(runtime_target)?;
        let next = RewriteSwitches {
            enabled: enabled.unwrap_or(current.enabled),
            codex_enabled: codex_enabled.unwrap_or(current.codex_enabled),
            claude_enabled: claude_enabled.unwrap_or(current.claude_enabled),
            grok_enabled: grok_enabled.unwrap_or(current.grok_enabled),
            zcode_enabled: zcode_enabled.unwrap_or(current.zcode_enabled),
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO rewrite_profiles (
                runtime_target, enabled, codex_enabled, claude_enabled, grok_enabled,
                zcode_enabled, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(runtime_target) DO UPDATE SET
                enabled = excluded.enabled,
                codex_enabled = excluded.codex_enabled,
                claude_enabled = excluded.claude_enabled,
                grok_enabled = excluded.grok_enabled,
                zcode_enabled = excluded.zcode_enabled,
                updated_at = excluded.updated_at",
            params![
                runtime_target.storage_key(),
                next.enabled as i64,
                next.codex_enabled as i64,
                next.claude_enabled as i64,
                next.grok_enabled as i64,
                next.zcode_enabled as i64,
                now_rfc3339(),
            ],
        )?;
        if runtime_target.is_native() {
            for (key, value) in [
                ("rewriteEnabled", next.enabled),
                ("rewriteCodexEnabled", next.codex_enabled),
                ("rewriteClaudeEnabled", next.claude_enabled),
                ("rewriteGrokEnabled", next.grok_enabled),
                ("rewriteZcodeEnabled", next.zcode_enabled),
            ] {
                tx.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, value.to_string()],
                )?;
            }
        }
        tx.commit()?;
        Ok(next)
    }

    pub fn rewrite_runtime_targets(&self) -> Result<Vec<RuntimeTarget>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT runtime_target FROM rewrite_profiles
             UNION SELECT runtime_target FROM rule_tables",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut targets = Vec::new();
        for row in rows {
            if let Ok(target) = RuntimeTarget::from_storage_key(&row?) {
                targets.push(target);
            }
        }
        if !targets.contains(&RuntimeTarget::Native) {
            targets.push(RuntimeTarget::Native);
        }
        Ok(targets)
    }

    pub fn has_pack_rules_for(
        &self,
        pack_id: &str,
        runtime_target: &RuntimeTarget,
    ) -> Result<bool> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM rule_tables
                WHERE id = ?1 AND runtime_target = ?2 AND kind = 'pack'
             )",
            params![pack_table_id(pack_id), runtime_target.storage_key()],
            |row| row.get(0),
        )
        .map_err(Error::from)
    }
}

pub fn pack_table_id(pack_id: &str) -> String {
    format!("pack:{pack_id}")
}

fn ensure_user_table(tx: &Transaction<'_>, runtime_target: &RuntimeTarget) -> Result<()> {
    let now = now_rfc3339();
    tx.execute(
        "INSERT OR IGNORE INTO rule_tables
         (id, runtime_target, kind, title, enabled, priority, created_at, updated_at)
         VALUES (?1, ?2, 'user', '', 1, 0, ?3, ?3)",
        params![USER_TABLE_ID, runtime_target.storage_key(), now],
    )?;
    Ok(())
}

fn replace_rules(
    tx: &Transaction<'_>,
    table_id: &str,
    runtime_target: &RuntimeTarget,
    rules: &[Rule],
) -> Result<()> {
    let runtime_key = runtime_target.storage_key();
    tx.execute(
        "DELETE FROM rules WHERE table_id = ?1 AND runtime_target = ?2",
        params![table_id, runtime_key],
    )?;
    let mut insert = tx.prepare(
        "INSERT INTO rules (table_id, runtime_target, position, from_text, to_text)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (position, rule) in rules.iter().enumerate() {
        insert.execute(params![
            table_id,
            runtime_key,
            position as i64,
            rule.from,
            rule.to
        ])?;
    }
    Ok(())
}

fn touch(tx: &Transaction<'_>, id: &str, runtime_target: &RuntimeTarget) -> Result<()> {
    tx.execute(
        "UPDATE rule_tables SET updated_at = ?3 WHERE id = ?1 AND runtime_target = ?2",
        params![id, runtime_target.storage_key(), now_rfc3339()],
    )?;
    Ok(())
}
