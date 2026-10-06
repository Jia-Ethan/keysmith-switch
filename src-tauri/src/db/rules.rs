//! Input rewrite rule tables. "My rules" is the one table the person edits; every other
//! table came from an extension pack and is read-only here.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use keysmith_rewrite::{validate_table, Rule};

use super::Store;
use crate::error::{Error, Result};
use crate::models::now_rfc3339;

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
    pub rules: Vec<Rule>,
    /// A pack update that waits for the person to accept it.
    pub pending: Option<PendingUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingUpdate {
    pub version: String,
    pub title: String,
    pub rules: Vec<Rule>,
}

fn invalid_rules(error: keysmith_rewrite::RuleError) -> Error {
    Error::invalid(error.to_string())
}

impl Store {
    /// Every table, "My rules" first and the packs after it in priority order.
    pub fn list_rule_tables(&self) -> Result<Vec<RuleTable>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, title, enabled, priority, pack_id, pack_version, pending_json
             FROM rule_tables
             ORDER BY CASE kind WHEN 'user' THEN 0 ELSE 1 END, priority, created_at",
        )?;
        let mut tables = stmt
            .query_map([], |row| {
                let kind: String = row.get(1)?;
                let pending: Option<String> = row.get(7)?;
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
                    rules: Vec::new(),
                    pending: pending.and_then(|text| serde_json::from_str(&text).ok()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut rules = conn.prepare(
            "SELECT from_text, to_text FROM rules WHERE table_id = ?1 ORDER BY position",
        )?;
        for table in &mut tables {
            table.rules = rules
                .query_map(params![table.id], |row| {
                    Ok(Rule::new(
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
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
                    rules: Vec::new(),
                    pending: None,
                },
            );
        }
        Ok(tables)
    }

    /// The rules of every enabled table, highest priority first.
    pub fn active_rules(&self) -> Result<Vec<Rule>> {
        Ok(self
            .list_rule_tables()?
            .into_iter()
            .filter(|table| table.enabled)
            .flat_map(|table| table.rules)
            .collect())
    }

    /// Replace "My rules" with these, in this order.
    pub fn save_user_rules(&self, rules: &[Rule]) -> Result<Vec<RuleTable>> {
        validate_table(rules).map_err(invalid_rules)?;
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            ensure_user_table(&tx)?;
            replace_rules(&tx, USER_TABLE_ID, rules)?;
            touch(&tx, USER_TABLE_ID)?;
            tx.commit()?;
        }
        self.list_rule_tables()
    }

    pub fn set_rule_table_enabled(&self, id: &str, enabled: bool) -> Result<Vec<RuleTable>> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            if id == USER_TABLE_ID {
                ensure_user_table(&tx)?;
            }
            let changed = tx.execute(
                "UPDATE rule_tables SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, enabled as i64, now_rfc3339()],
            )?;
            if changed == 0 {
                return Err(Error::invalid("rule table not found"));
            }
            tx.commit()?;
        }
        self.list_rule_tables()
    }

    /// Put the pack tables in this order. "My rules" always stays first.
    pub fn reorder_rule_tables(&self, ids: &[String]) -> Result<Vec<RuleTable>> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            for (index, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE rule_tables SET priority = ?2 WHERE id = ?1 AND kind = 'pack'",
                    params![id, index as i64 + 1],
                )?;
            }
            tx.commit()?;
        }
        self.list_rule_tables()
    }

    /// Append the rules of a pack table to "My rules", skipping text "My rules" already matches.
    pub fn copy_rules_to_user(&self, id: &str) -> Result<Vec<RuleTable>> {
        let tables = self.list_rule_tables()?;
        let source = tables
            .iter()
            .find(|table| table.id == id && table.kind == TableKind::Pack)
            .ok_or_else(|| Error::invalid("rule table not found"))?;
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
        self.save_user_rules(&mine)
    }

    /// Install a pack table or update one that is not enabled. An enabled table keeps its
    /// rules and holds the new version as pending until [`Store::accept_pack_update`].
    /// Returns true when the update is pending.
    pub fn upsert_pack_rules(
        &self,
        pack_id: &str,
        version: &str,
        title: &str,
        rules: &[Rule],
        enable: bool,
    ) -> Result<bool> {
        validate_table(rules).map_err(invalid_rules)?;
        let id = pack_table_id(pack_id);
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let existing: Option<(bool, Option<String>)> = tx
            .query_row(
                "SELECT enabled, pack_version FROM rule_tables WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, i64>(0)? != 0, row.get(1)?)),
            )
            .optional()?;
        let pending = match existing {
            None => {
                let priority: i64 = tx.query_row(
                    "SELECT COALESCE(MAX(priority), 0) + 1 FROM rule_tables WHERE kind = 'pack'",
                    [],
                    |row| row.get(0),
                )?;
                let now = now_rfc3339();
                tx.execute(
                    "INSERT INTO rule_tables
                     (id, kind, title, enabled, priority, pack_id, pack_version, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        id,
                        TableKind::Pack.as_str(),
                        title,
                        enable as i64,
                        priority,
                        pack_id,
                        version,
                        now
                    ],
                )?;
                replace_rules(&tx, &id, rules)?;
                false
            }
            Some((enabled, current)) if enabled && current.as_deref() != Some(version) => {
                let pending = PendingUpdate {
                    version: version.into(),
                    title: title.into(),
                    rules: rules.to_vec(),
                };
                tx.execute(
                    "UPDATE rule_tables SET pending_json = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, serde_json::to_string(&pending)?, now_rfc3339()],
                )?;
                true
            }
            Some(_) => {
                tx.execute(
                    "UPDATE rule_tables
                     SET title = ?2, pack_version = ?3, pending_json = NULL,
                         enabled = MAX(enabled, ?4), updated_at = ?5
                     WHERE id = ?1",
                    params![id, title, version, enable as i64, now_rfc3339()],
                )?;
                replace_rules(&tx, &id, rules)?;
                false
            }
        };
        tx.commit()?;
        Ok(pending)
    }

    /// Switch an enabled pack table to the update it was holding.
    pub fn accept_pack_update(&self, id: &str) -> Result<Vec<RuleTable>> {
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let pending: Option<String> = tx
                .query_row(
                    "SELECT pending_json FROM rule_tables WHERE id = ?1 AND kind = 'pack'",
                    params![id],
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
                 SET title = ?2, pack_version = ?3, pending_json = NULL, updated_at = ?4
                 WHERE id = ?1",
                params![id, pending.title, pending.version, now_rfc3339()],
            )?;
            replace_rules(&tx, id, &pending.rules)?;
            tx.commit()?;
        }
        self.list_rule_tables()
    }

    pub fn remove_pack_rules(&self, pack_id: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM rule_tables WHERE id = ?1 AND kind = 'pack'",
            params![pack_table_id(pack_id)],
        )?;
        Ok(())
    }
}

pub fn pack_table_id(pack_id: &str) -> String {
    format!("pack:{pack_id}")
}

fn ensure_user_table(tx: &Transaction<'_>) -> Result<()> {
    let now = now_rfc3339();
    tx.execute(
        "INSERT OR IGNORE INTO rule_tables (id, kind, title, enabled, priority, created_at, updated_at)
         VALUES (?1, 'user', '', 1, 0, ?2, ?2)",
        params![USER_TABLE_ID, now],
    )?;
    Ok(())
}

fn replace_rules(tx: &Transaction<'_>, table_id: &str, rules: &[Rule]) -> Result<()> {
    tx.execute("DELETE FROM rules WHERE table_id = ?1", params![table_id])?;
    let mut insert = tx.prepare(
        "INSERT INTO rules (table_id, position, from_text, to_text) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (position, rule) in rules.iter().enumerate() {
        insert.execute(params![table_id, position as i64, rule.from, rule.to])?;
    }
    Ok(())
}

fn touch(tx: &Transaction<'_>, id: &str) -> Result<()> {
    tx.execute(
        "UPDATE rule_tables SET updated_at = ?2 WHERE id = ?1",
        params![id, now_rfc3339()],
    )?;
    Ok(())
}
