use keysmith_switch_lib::db::markdown::{parse_markdown, render_markdown, FrontMatter};
use keysmith_switch_lib::db::schema::SCHEMA_VERSION;
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::{
    Activation, Operation, OperationKind, OperationStatus, RuntimeTarget, Scope, SettingsPatch,
    ToolKind, ToolStatus,
};
use keysmith_switch_lib::paths::{atomic_write, AppPaths};

fn write_sample(paths: &AppPaths, id: &str, tool: ToolKind, title: &str, body: &str) {
    let front = FrontMatter {
        id: id.to_string(),
        tool,
        title: title.to_string(),
        tags: vec!["core".into()],
        version: 1,
        deleted: false,
    };
    atomic_write(&paths.prompt_file(tool, id), &render_markdown(&front, body)).unwrap();
}

#[test]
fn sqlite_first_create_applies_migration() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert!(paths.db.is_file());
    assert!(store
        .list_prompts(ToolKind::Claude, None, None, Default::default())
        .unwrap()
        .is_empty());
}

#[test]
fn sqlite_migration_backs_up_existing_db() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    store.clear_schema_versions_for_test().unwrap();
    drop(store);
    let store = Store::open(&paths).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    let backups: Vec<_> = std::fs::read_dir(&paths.backups)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .collect();
    assert!(
        backups
            .iter()
            .any(|entry| entry.file_name().to_string_lossy().contains("pre-migrate")),
        "expected pre-migrate backup, found {backups:?}"
    );
}

#[test]
fn sqlite_backup_db_writes_copy() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    let dest = store.backup_db("manual").unwrap();
    assert!(dest.is_file());
}

#[test]
fn activations_are_isolated_by_runtime_target() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    let activation = |id: &str, runtime_target: RuntimeTarget| Activation {
        id: id.into(),
        prompt_id: Some(format!("prompt-{id}")),
        tool: ToolKind::Codex,
        scope: Scope::User,
        runtime_target,
        project_dir: None,
        status: ToolStatus::Active,
        fingerprint: None,
        operation_id: None,
        created_at: "2026-11-24T00:00:00Z".into(),
        updated_at: "2026-11-24T00:00:00Z".into(),
    };
    let native = activation("native", RuntimeTarget::Native);
    let ubuntu = activation(
        "ubuntu",
        RuntimeTarget::Wsl {
            distribution: "Ubuntu-24.04".into(),
            user: None,
        },
    );
    store.upsert_activation(&native).unwrap();
    store.upsert_activation(&ubuntu).unwrap();

    assert_eq!(
        store
            .list_activations_for_runtime(ToolKind::Codex, &RuntimeTarget::Native)
            .unwrap(),
        vec![native]
    );
    assert_eq!(
        store
            .list_activations_for_runtime(
                ToolKind::Codex,
                &RuntimeTarget::Wsl {
                    distribution: "Ubuntu-24.04".into(),
                    user: None,
                },
            )
            .unwrap(),
        vec![ubuntu]
    );
}

#[test]
fn operations_and_settings_are_isolated_by_runtime_target() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    let operation = |id: &str, runtime_target: RuntimeTarget| Operation {
        id: id.into(),
        tool: ToolKind::Codex,
        kind: OperationKind::PlanActivate,
        status: OperationStatus::Preview,
        preview: true,
        runtime_target,
        prompt_id: None,
        scope: Some(Scope::User),
        project_dir: None,
        request_json: "{}".into(),
        envelope_json: None,
        error: None,
        parent_id: None,
        created_at: "2026-11-24T00:00:00Z".into(),
        updated_at: "2026-11-24T00:00:00Z".into(),
    };
    let native = operation("native-op", RuntimeTarget::Native);
    let ubuntu_target = RuntimeTarget::Wsl {
        distribution: "Ubuntu-24.04".into(),
        user: None,
    };
    let ubuntu = operation("ubuntu-op", ubuntu_target.clone());
    store.insert_operation(&native).unwrap();
    store.insert_operation(&ubuntu).unwrap();

    assert_eq!(
        store
            .list_operations_for_runtime(ToolKind::Codex, &RuntimeTarget::Native)
            .unwrap(),
        vec![native]
    );
    assert_eq!(
        store
            .list_operations_for_runtime(ToolKind::Codex, &ubuntu_target)
            .unwrap(),
        vec![ubuntu]
    );

    let mut targets = std::collections::HashMap::new();
    targets.insert(ToolKind::Codex, ubuntu_target.clone());
    store
        .update_settings(SettingsPatch {
            agent_runtime_targets: Some(targets),
            ..SettingsPatch::default()
        })
        .unwrap();
    drop(store);

    let reopened = Store::open(&paths).unwrap();
    assert_eq!(
        reopened
            .get_settings()
            .unwrap()
            .agent_runtime_targets
            .get(&ToolKind::Codex),
        Some(&ubuntu_target)
    );
}

#[test]
fn schema_v4_rows_migrate_to_native_runtime() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    let activation = Activation {
        id: "legacy".into(),
        prompt_id: None,
        tool: ToolKind::Codex,
        scope: Scope::User,
        runtime_target: RuntimeTarget::Native,
        project_dir: None,
        status: ToolStatus::Active,
        fingerprint: None,
        operation_id: None,
        created_at: "2026-11-24T00:00:00Z".into(),
        updated_at: "2026-11-24T00:00:00Z".into(),
    };
    store.upsert_activation(&activation).unwrap();
    drop(store);

    let conn = rusqlite::Connection::open(&paths.db).unwrap();
    conn.execute_batch(
        "ALTER TABLE activations DROP COLUMN runtime_target;
         ALTER TABLE operations DROP COLUMN runtime_target;
         DROP TABLE tool_state;
         CREATE TABLE tool_state (
             tool TEXT NOT NULL,
             scope TEXT NOT NULL,
             project_dir TEXT NOT NULL DEFAULT '',
             status TEXT,
             fingerprint TEXT,
             last_operation_id TEXT,
             last_checked_at TEXT,
             PRIMARY KEY (tool, scope, project_dir)
         );
         DELETE FROM schema_migrations WHERE version >= 5;",
    )
    .unwrap();
    drop(conn);

    let migrated = Store::open(&paths).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), SCHEMA_VERSION);
    let inspect = rusqlite::Connection::open(&paths.db).unwrap();
    let rows: Vec<(String, String)> = inspect
        .prepare("SELECT id, runtime_target FROM activations")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![("legacy".into(), "native".into())]);
    drop(inspect);
    assert_eq!(
        migrated
            .find_activation_for_runtime(
                ToolKind::Codex,
                &RuntimeTarget::Native,
                Scope::User,
                None,
            )
            .unwrap()
            .unwrap()
            .id,
        "legacy"
    );
}

#[test]
fn sqlite_corrupt_rebuilds_from_markdown() {
    let (_tmp, paths) = AppPaths::in_temp_dir().unwrap();
    write_sample(
        &paths,
        "p-rebuild",
        ToolKind::Claude,
        "Rebuild Me",
        "hello from markdown\n",
    );
    std::fs::write(&paths.db, b"this is not a sqlite database").unwrap();
    let store = Store::open(&paths).unwrap();
    let listed = store
        .list_prompts(ToolKind::Claude, None, None, Default::default())
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "p-rebuild");
    let detail = store.get_prompt("p-rebuild").unwrap();
    assert_eq!(detail.content.trim(), "hello from markdown");
    let quarantined: Vec<_> = std::fs::read_dir(&paths.backups)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .collect();
    assert!(quarantined
        .iter()
        .any(|entry| { entry.file_name().to_string_lossy().contains("corrupt") }));
}

#[test]
fn markdown_roundtrip_front_matter() {
    let front = FrontMatter {
        id: "abc".into(),
        tool: ToolKind::Grok,
        title: "Title".into(),
        tags: vec!["a".into(), "b".into()],
        version: 3,
        deleted: false,
    };
    let rendered = render_markdown(&front, "body line\n");
    let parsed = parse_markdown(&rendered).unwrap();
    assert_eq!(parsed.front, front);
    assert_eq!(parsed.content, "body line\n");
    for body in [
        "no trailing newline",
        "\nleading blank\n",
        "\n\nmultiple blank lines\n",
        "",
    ] {
        let parsed = parse_markdown(&render_markdown(&front, body)).unwrap();
        assert_eq!(parsed.content, body);
    }
}
