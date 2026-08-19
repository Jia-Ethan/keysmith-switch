use keysmith_switch_lib::data::{
    clear_all_data, clear_plan, export_zip, import_candidates, import_markdown_file, import_zip,
    scan_import_candidates, CLEAR_CONFIRM_PHRASE,
};
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::{PromptSort, ToolKind};
use keysmith_switch_lib::paths::AppPaths;
use std::fs;
use std::path::PathBuf;

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
    let (_tmp, store) = store();
    let paths = store.paths().clone();
    let plan = clear_plan(&paths);
    assert!(plan.irreversible);
    assert!(clear_all_data(&paths, "nope", true).is_err());
    assert!(clear_all_data(&paths, CLEAR_CONFIRM_PHRASE, false).is_err());
    clear_all_data(&paths, CLEAR_CONFIRM_PHRASE, true).unwrap();
    assert!(paths.home.exists());
    assert!(!paths.db.exists());
}

#[test]
fn sidecar_dir_prefers_frozen_binary() {
    use keysmith_switch_lib::adapter::process::resolve_cli;
    use keysmith_switch_lib::adapter::AdapterOptions;
    use std::sync::{Mutex, OnceLock};
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
