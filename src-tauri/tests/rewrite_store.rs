use keysmith_rewrite::{Rule, Snapshot, Tool};
use keysmith_switch_lib::db::rules::{pack_table_id, TableKind, USER_TABLE_ID};
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::SettingsPatch;
use keysmith_switch_lib::paths::AppPaths;
use keysmith_switch_lib::rewrite;
use pretty_assertions::assert_eq;

fn rules(pairs: &[(&str, &str)]) -> Vec<Rule> {
    pairs.iter().map(|(f, t)| Rule::new(*f, *t)).collect()
}

#[test]
fn starts_empty_and_off() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    let view = rewrite::view(&store).unwrap();
    assert!(!view.enabled);
    assert!(view.codex_enabled);
    assert_eq!(view.tables.len(), 1);
    assert_eq!(view.tables[0].kind, TableKind::User);
    assert!(view.tables[0].rules.is_empty());
    let snapshot = rewrite::build_snapshot(&store).unwrap();
    assert!(snapshot.rules.is_empty());
    assert!(snapshot.codex_matcher().is_none());
}

#[test]
fn user_rules_keep_order_and_reject_duplicates() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store
        .save_user_rules(&rules(&[("b", "2"), ("a", "1")]))
        .unwrap();
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("b", "2"), ("a", "1")])
    );
    assert!(store
        .save_user_rules(&rules(&[("a", "1"), ("a", "2")]))
        .is_err());
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("b", "2"), ("a", "1")])
    );
}

#[test]
fn user_rules_come_before_packs_and_packs_follow_their_order() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store.save_user_rules(&rules(&[("x", "mine")])).unwrap();
    store
        .upsert_pack_rules(
            "p.one",
            "1.0.0",
            "One",
            &rules(&[("x", "one"), ("y", "one")]),
            &[Tool::Codex],
            true,
        )
        .unwrap();
    store
        .upsert_pack_rules(
            "p.two",
            "1.0.0",
            "Two",
            &rules(&[("y", "two")]),
            &[Tool::Codex],
            true,
        )
        .unwrap();
    let active = store.active_rules(Tool::Codex).unwrap();
    let matcher = keysmith_rewrite::Matcher::new(&active);
    assert_eq!(matcher.rewrite("x y"), "mine one");

    store
        .reorder_rule_tables(&[pack_table_id("p.two"), pack_table_id("p.one")])
        .unwrap();
    let matcher = keysmith_rewrite::Matcher::new(&store.active_rules(Tool::Codex).unwrap());
    assert_eq!(matcher.rewrite("x y"), "mine two");
}

#[test]
fn disabled_table_contributes_nothing() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store
        .upsert_pack_rules(
            "p.one",
            "1.0.0",
            "One",
            &rules(&[("a", "b")]),
            &[Tool::Codex],
            false,
        )
        .unwrap();
    assert!(store.active_rules(Tool::Codex).unwrap().is_empty());
    store
        .set_rule_table_enabled(&pack_table_id("p.one"), true)
        .unwrap();
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("a", "b")])
    );
    store.set_rule_table_enabled(USER_TABLE_ID, false).unwrap();
    assert!(store.set_rule_table_enabled("missing", true).is_err());
}

#[test]
fn enabled_pack_update_waits_for_acceptance() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store
        .upsert_pack_rules(
            "p.one",
            "1.0.0",
            "One",
            &rules(&[("a", "1")]),
            &[Tool::Codex],
            true,
        )
        .unwrap();
    let pending = store
        .upsert_pack_rules(
            "p.one",
            "1.1.0",
            "One",
            &rules(&[("a", "2")]),
            &[Tool::Codex],
            false,
        )
        .unwrap();
    assert!(pending);
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("a", "1")])
    );
    let table = store
        .list_rule_tables()
        .unwrap()
        .into_iter()
        .find(|table| table.id == pack_table_id("p.one"))
        .unwrap();
    assert_eq!(table.pending.as_ref().unwrap().version, "1.1.0");

    store.accept_pack_update(&pack_table_id("p.one")).unwrap();
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("a", "2")])
    );
    assert!(store.accept_pack_update(&pack_table_id("p.one")).is_err());
}

#[test]
fn disabled_pack_updates_silently() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store
        .upsert_pack_rules(
            "p.one",
            "1.0.0",
            "One",
            &rules(&[("a", "1")]),
            &[Tool::Codex],
            false,
        )
        .unwrap();
    let pending = store
        .upsert_pack_rules(
            "p.one",
            "1.1.0",
            "One",
            &rules(&[("a", "2")]),
            &[Tool::Codex],
            false,
        )
        .unwrap();
    assert!(!pending);
    let table = store
        .list_rule_tables()
        .unwrap()
        .into_iter()
        .find(|table| table.id == pack_table_id("p.one"))
        .unwrap();
    assert_eq!(table.rules, rules(&[("a", "2")]));
    assert!(!table.enabled);
    assert!(table.pending.is_none());
}

#[test]
fn copy_to_user_skips_text_already_matched() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store.save_user_rules(&rules(&[("a", "mine")])).unwrap();
    store
        .upsert_pack_rules(
            "p.one",
            "1.0.0",
            "One",
            &rules(&[("a", "p"), ("b", "p")]),
            &[Tool::Codex],
            false,
        )
        .unwrap();
    store.copy_rules_to_user(&pack_table_id("p.one")).unwrap();
    let user = store
        .list_rule_tables()
        .unwrap()
        .into_iter()
        .find(|table| table.kind == TableKind::User)
        .unwrap();
    assert_eq!(user.rules, rules(&[("a", "mine"), ("b", "p")]));
}

#[test]
fn publish_writes_a_snapshot_the_relay_can_read() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    rewrite::change(&store, |store| {
        store.save_user_rules(&rules(&[("a", "b")]))?;
        store.update_settings(SettingsPatch {
            rewrite_enabled: Some(true),
            ..Default::default()
        })?;
        Ok(())
    })
    .unwrap();
    let bytes = std::fs::read(rewrite::snapshot_path(&paths)).unwrap();
    let snapshot = Snapshot::parse(&bytes).unwrap();
    assert!(snapshot.enabled && snapshot.tools.codex);
    assert_eq!(snapshot.codex_matcher().unwrap().rewrite("a"), "b");
}

#[test]
fn clear_all_removes_rules() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store.save_user_rules(&rules(&[("a", "b")])).unwrap();
    store
        .upsert_pack_rules(
            "p.one",
            "1.0.0",
            "One",
            &rules(&[("c", "d")]),
            &[Tool::Codex],
            true,
        )
        .unwrap();
    store.clear_all().unwrap();
    assert!(store.active_rules(Tool::Codex).unwrap().is_empty());
    assert_eq!(store.list_rule_tables().unwrap().len(), 1);
}

#[test]
fn rules_survive_reopen() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    {
        let store = Store::open(&paths).unwrap();
        store.save_user_rules(&rules(&[("a", "b")])).unwrap();
    }
    let store = Store::open(&paths).unwrap();
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("a", "b")])
    );
}

#[test]
fn restoring_a_schema_1_backup_migrates_it_forward() {
    let (tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let old_db = tmp.path().join("v1.db");
    {
        let store = Store::open(&paths).unwrap();
        store.backup_db_to(&old_db).unwrap();
    }
    {
        let conn = rusqlite::Connection::open(&old_db).unwrap();
        conn.execute_batch(
            "DROP TABLE rules; DROP TABLE rule_tables;
             DELETE FROM schema_migrations WHERE version >= 2;",
        )
        .unwrap();
    }
    let empty_prompts = tmp.path().join("prompts");
    std::fs::create_dir_all(&empty_prompts).unwrap();

    let (_tmp2, paths2) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths2).unwrap();
    store.save_user_rules(&rules(&[("a", "b")])).unwrap();
    store.restore_snapshot(&old_db, &empty_prompts).unwrap();
    assert_eq!(
        store.schema_version().unwrap(),
        keysmith_switch_lib::db::schema::SCHEMA_VERSION
    );
    assert!(store.active_rules(Tool::Codex).unwrap().is_empty());
    store.save_user_rules(&rules(&[("c", "d")])).unwrap();
}

#[test]
fn restoring_a_schema_2_backup_keeps_tables_for_every_agent() {
    let (tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let old_db = tmp.path().join("v2.db");
    {
        let store = Store::open(&paths).unwrap();
        store.save_user_rules(&rules(&[("a", "b")])).unwrap();
        store.backup_db_to(&old_db).unwrap();
    }
    {
        let conn = rusqlite::Connection::open(&old_db).unwrap();
        conn.execute_batch(
            "ALTER TABLE rule_tables DROP COLUMN tools_json;
             DELETE FROM schema_migrations WHERE version = 3;",
        )
        .unwrap();
    }
    let empty_prompts = tmp.path().join("prompts");
    std::fs::create_dir_all(&empty_prompts).unwrap();
    let (_tmp2, paths2) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths2).unwrap();
    store.restore_snapshot(&old_db, &empty_prompts).unwrap();
    assert_eq!(store.schema_version().unwrap(), 3);
    for tool in Tool::ALL {
        assert_eq!(store.active_rules(tool).unwrap(), rules(&[("a", "b")]));
    }
}

#[test]
fn a_table_can_be_narrowed_to_some_agents() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store.save_user_rules(&rules(&[("a", "b")])).unwrap();
    store
        .upsert_pack_rules(
            "p.claude",
            "1.0.0",
            "C",
            &rules(&[("c", "d")]),
            &[Tool::Claude],
            true,
        )
        .unwrap();
    assert_eq!(
        store.active_rules(Tool::Codex).unwrap(),
        rules(&[("a", "b")])
    );
    assert_eq!(
        store.active_rules(Tool::Claude).unwrap(),
        rules(&[("a", "b"), ("c", "d")])
    );

    let tables = store
        .set_rule_table_tools(USER_TABLE_ID, Some(&[Tool::Zcode]))
        .unwrap();
    assert_eq!(tables[0].tools, Some(vec![Tool::Zcode]));
    assert!(store.active_rules(Tool::Codex).unwrap().is_empty());
    assert_eq!(
        store.active_rules(Tool::Zcode).unwrap(),
        rules(&[("a", "b")])
    );

    assert!(store
        .set_rule_table_tools(USER_TABLE_ID, Some(&[]))
        .is_err());
    store.set_rule_table_tools(USER_TABLE_ID, None).unwrap();
    assert_eq!(
        store.active_rules(Tool::Grok).unwrap(),
        rules(&[("a", "b")])
    );
}

#[test]
fn a_pack_update_keeps_the_agents_the_person_chose() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    let all = Tool::ALL;
    store
        .upsert_pack_rules("p.one", "1.0.0", "One", &rules(&[("c", "d")]), &all, false)
        .unwrap();
    let id = pack_table_id("p.one");
    assert_eq!(store.list_rule_tables().unwrap()[1].tools, None);
    store
        .set_rule_table_tools(&id, Some(&[Tool::Codex]))
        .unwrap();
    store
        .upsert_pack_rules("p.one", "1.1.0", "One", &rules(&[("e", "f")]), &all, false)
        .unwrap();
    let table = store.list_rule_tables().unwrap().remove(1);
    assert_eq!(table.pack_version.as_deref(), Some("1.1.0"));
    assert_eq!(table.tools, Some(vec![Tool::Codex]));
}

#[test]
fn snapshot_stays_readable_by_a_v040_relay() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store.save_user_rules(&rules(&[("a", "b")])).unwrap();
    store
        .update_settings(SettingsPatch {
            rewrite_enabled: Some(true),
            ..Default::default()
        })
        .unwrap();
    let snapshot = rewrite::build_snapshot(&store).unwrap();
    // Every table applies everywhere: no per-agent lists, one shared list.
    assert_eq!(snapshot.by_tool, None);
    assert!(snapshot.tools.claude && snapshot.tools.zcode);
    let json = serde_json::to_value(&snapshot).unwrap();
    assert!(json.get("byTool").is_none());

    store
        .set_rule_table_tools(USER_TABLE_ID, Some(&[Tool::Claude]))
        .unwrap();
    let snapshot = rewrite::build_snapshot(&store).unwrap();
    assert!(
        snapshot.rules.is_empty(),
        "Codex's list is the top-level one"
    );
    assert!(snapshot.matcher(Tool::Codex).is_none());
    assert_eq!(snapshot.matcher(Tool::Claude).unwrap().rewrite("a"), "b");
    assert!(snapshot.matcher(Tool::Zcode).is_none());
}
