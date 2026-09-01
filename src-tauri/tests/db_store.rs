use keysmith_switch_lib::db::markdown::{parse_markdown, render_markdown, FrontMatter};
use keysmith_switch_lib::db::schema::SCHEMA_VERSION;
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::ToolKind;
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
}
