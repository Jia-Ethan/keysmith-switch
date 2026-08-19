use keysmith_switch_lib::data::{
    clear_all_data, clear_plan, export_zip, import_candidates, import_markdown_file, import_zip,
    inspect_zip, scan_import_candidates, CLEAR_CONFIRM_PHRASE,
};
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::{Activation, UpdatePromptInput};
use keysmith_switch_lib::models::{PromptSort, ToolKind};
use keysmith_switch_lib::models::{Scope, SettingsPatch, ToolStatus};
use keysmith_switch_lib::ops::update_prompt;
use keysmith_switch_lib::paths::AppPaths;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn clear_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn store() -> (tempfile::TempDir, Store) {
    let (tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    (tmp, store)
}

#[test]
fn scan_does_not_activate_native_prompts() {
    let (tmp, store) = store();
    let home = tmp.path().join("user-home");
    let dir = home.join(".claude").join("keysmith");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("rules.md"), "# Native Rule\n\nhello from native\n").unwrap();
    let candidates = scan_import_candidates(&store, &home).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "Native Rule");
    assert!(!candidates[0].already_imported);
    assert!(store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Title)
        .unwrap()
        .is_empty());
}

#[test]
fn import_copies_into_library_without_activation() {
    let (tmp, store) = store();
    let file = tmp.path().join("prompt.md");
    fs::write(&file, "# Imported\n\nbody\n").unwrap();
    assert!(import_markdown_file(&store, ToolKind::Codex, &file).unwrap());
    let listed = store
        .list_prompts(ToolKind::Codex, None, None, PromptSort::Title)
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "Imported");
    let activations = store.list_activations(ToolKind::Codex).unwrap();
    assert!(activations.is_empty());
}

#[test]
fn zip_roundtrip() {
    let (tmp, store_a) = store();
    let file = tmp.path().join("a.md");
    fs::write(&file, "# Zip Me\n\nzip body\n").unwrap();
    import_markdown_file(&store_a, ToolKind::Grok, &file).unwrap();
    let zip_path = tmp.path().join("export.zip");
    export_zip(&store_a, &zip_path).unwrap();
    assert!(zip_path.is_file());

    let (_tmp2, store2) = store();
    let result = import_zip(&store2, &zip_path).unwrap();
    assert_eq!(result.imported, 1);
    let listed = store2
        .list_prompts(ToolKind::Grok, None, None, PromptSort::Title)
        .unwrap();
    assert_eq!(listed[0].title, "Zip Me");
}

#[test]
fn inspect_zip_distinguishes_full_restore_from_legacy_import() {
    let (tmp, store) = store();
    let file = tmp.path().join("a.md");
    fs::write(&file, "# Full\n\nbody\n").unwrap();
    import_markdown_file(&store, ToolKind::Grok, &file).unwrap();
    let full = tmp.path().join("full.zip");
    export_zip(&store, &full).unwrap();
    assert_eq!(inspect_zip(&full).unwrap().mode, "restore");

    let legacy = tmp.path().join("legacy.zip");
    let output = fs::File::create(&legacy).unwrap();
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default();
    use std::io::Write;
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(br#"{"app":"Keysmith Switch","version":1}"#)
        .unwrap();
    writer.start_file("prompts/legacy.md", options).unwrap();
    writer.write_all(b"# Legacy\n\nbody\n").unwrap();
    writer.finish().unwrap();
    assert_eq!(inspect_zip(&legacy).unwrap().mode, "import");
}

#[test]
fn backup_restore_preserves_history_settings_and_activation_without_duplicates() {
    let (tmp, store_a) = store();
    let file = tmp.path().join("a.md");
    fs::write(&file, "# Original\n\nversion one\n").unwrap();
    import_markdown_file(&store_a, ToolKind::Grok, &file).unwrap();
    let id = store_a
        .list_prompts(ToolKind::Grok, None, None, PromptSort::Title)
        .unwrap()[0]
        .id
        .clone();
    update_prompt(
        &store_a,
        UpdatePromptInput {
            id: id.clone(),
            title: Some("Renamed".into()),
            content: Some("version two\n".into()),
            tags: Some(vec!["stable".into(), "shared".into()]),
        },
    )
    .unwrap();
    store_a
        .update_settings(SettingsPatch {
            language: Some("en".into()),
            theme: Some("dark".into()),
            first_run_completed: Some(true),
            ..Default::default()
        })
        .unwrap();
    store_a
        .upsert_activation(&Activation {
            id: "activation-1".into(),
            prompt_id: Some(id.clone()),
            tool: ToolKind::Grok,
            scope: Scope::User,
            project_dir: None,
            status: ToolStatus::Active,
            fingerprint: Some("fingerprint".into()),
            operation_id: None,
            created_at: "2026-08-19T00:00:00Z".into(),
            updated_at: "2026-08-19T00:00:00Z".into(),
        })
        .unwrap();
    let zip_path = tmp.path().join("backup.zip");
    export_zip(&store_a, &zip_path).unwrap();

    let (_tmp2, store_b) = store();
    assert_eq!(import_zip(&store_b, &zip_path).unwrap().imported, 1);
    assert_eq!(import_zip(&store_b, &zip_path).unwrap().imported, 1);
    let prompts = store_b
        .list_prompts(ToolKind::Grok, None, None, PromptSort::Title)
        .unwrap();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].id, id);
    assert_eq!(prompts[0].title, "Renamed");
    assert_eq!(prompts[0].tags, vec!["stable", "shared"]);
    let versions = store_b.list_versions(&id).unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].content.trim(), "# Original\n\nversion one");
    assert_eq!(versions[1].content.trim(), "version two");
    let settings = store_b.get_settings().unwrap();
    assert_eq!(settings.language, "en");
    assert_eq!(settings.theme, "dark");
    assert!(settings.first_run_completed);
    let activations = store_b.list_activations(ToolKind::Grok).unwrap();
    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].prompt_id.as_deref(), Some(id.as_str()));
}

#[test]
fn backup_rejects_checksum_tampering() {
    let (tmp, store) = store();
    let file = tmp.path().join("a.md");
    fs::write(&file, "# Safe\n\nbody\n").unwrap();
    import_markdown_file(&store, ToolKind::Claude, &file).unwrap();
    let zip_path = tmp.path().join("backup.zip");
    export_zip(&store, &zip_path).unwrap();

    let tampered = tmp.path().join("tampered.zip");
    let input = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(input).unwrap();
    let output = fs::File::create(&tampered).unwrap();
    let mut writer = zip::ZipWriter::new(output);
    let options = zip::write::SimpleFileOptions::default();
    for index in 0..archive.len() {
        use std::io::{Read, Write};
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        writer.start_file(&name, options).unwrap();
        if name.starts_with("prompts/") {
            bytes.extend_from_slice(b"tampered");
        }
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
    assert!(import_zip(&store, &tampered).is_err());
}

#[test]
fn import_candidates_selected_paths_only() {
    let (tmp, store) = store();
    let a = tmp.path().join("a.md");
    let b = tmp.path().join("b.md");
    fs::write(&a, "# A\n").unwrap();
    fs::write(&b, "# B\n").unwrap();
    let result = import_candidates(&store, &[a.display().to_string()]).unwrap();
    assert_eq!(result.imported, 1);
}

#[test]
fn clear_all_data_requires_phrase_and_confirmation() {
    let _guard = clear_test_lock();
    let (_tmp, store) = store();
    let paths = store.paths().clone();
    let plan = clear_plan(&paths);
    assert!(plan.irreversible);
    assert!(clear_all_data(&store, "nope", true).is_err());
    assert!(clear_all_data(&store, CLEAR_CONFIRM_PHRASE, false).is_err());
    let file = paths.home.join("before.md");
    fs::write(&file, "# Before\n\nbody\n").unwrap();
    import_markdown_file(&store, ToolKind::Claude, &file).unwrap();
    clear_all_data(&store, CLEAR_CONFIRM_PHRASE, true).unwrap();
    assert!(paths.home.exists());
    assert!(paths.db.exists());
    assert!(store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Title)
        .unwrap()
        .is_empty());

    let after = paths.home.join("after.md");
    fs::write(&after, "# After\n\nbody\n").unwrap();
    import_markdown_file(&store, ToolKind::Claude, &after).unwrap();
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, PromptSort::Title)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn clear_all_rolls_back_if_data_directories_cannot_be_staged() {
    let _guard = clear_test_lock();
    let (tmp, store) = store();
    let file = tmp.path().join("before.md");
    fs::write(&file, "# Before\n\nbody\n").unwrap();
    import_markdown_file(&store, ToolKind::Claude, &file).unwrap();
    std::env::set_var("KEYSMITH_SWITCH_CLEAR_FAIL_STAGE_FOR_TEST", "backups");
    let result = clear_all_data(&store, CLEAR_CONFIRM_PHRASE, true);
    std::env::remove_var("KEYSMITH_SWITCH_CLEAR_FAIL_STAGE_FOR_TEST");

    assert!(result.is_err());
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, PromptSort::Title)
            .unwrap()
            .len(),
        1
    );
    assert!(store.paths().prompts.exists());
}

#[test]
fn sidecar_dir_prefers_frozen_binary() {
    use keysmith_switch_lib::adapter::process::resolve_cli;
    use keysmith_switch_lib::adapter::AdapterOptions;
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let bin: PathBuf = dir.path().join("keysmith-claude");
    fs::write(&bin, b"#!/bin/sh\necho frozen\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&bin).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&bin, perm).unwrap();
    }
    std::env::set_var("KEYSMITH_SWITCH_SIDECAR_DIR", dir.path());
    std::env::remove_var("KEYSMITH_SWITCH_FORCE_PYTHON");
    let cli = resolve_cli(ToolKind::Claude, &AdapterOptions::default()).unwrap();
    std::env::remove_var("KEYSMITH_SWITCH_SIDECAR_DIR");
    assert!(
        cli.frozen,
        "expected frozen sidecar, got {}",
        cli.cli_path()
    );
    assert!(cli.cli_path().contains("keysmith-claude"));
}
