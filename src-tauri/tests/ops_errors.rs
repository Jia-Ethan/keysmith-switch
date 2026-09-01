use std::path::PathBuf;

use keysmith_switch_lib::adapter::{
    check_adapter_version, run_adapter_with, AdapterCommand, AdapterOptions,
};
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::error::Error;
use keysmith_switch_lib::models::{
    CreatePromptInput, PlanActivateInput, Scope, ToolKind, ToolStatus,
};
use keysmith_switch_lib::ops::{confirm_activate, create_prompt, plan_activate, tool_status};
use keysmith_switch_lib::paths::AppPaths;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cli")
        .join(name)
}

fn store_and_home() -> (tempfile::TempDir, Store, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("user-home");
    std::fs::create_dir_all(&home).unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();
    (tmp, store, home)
}

#[tokio::test]
async fn cli_missing() {
    let opts = AdapterOptions {
        cli_override: Some(PathBuf::from("/tmp/missing-keysmith-cli-does-not-exist.py")),
        ..AdapterOptions::default()
    };
    let err = run_adapter_with(ToolKind::Claude, AdapterCommand::Version, &opts)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "cli-missing");
}

#[tokio::test]
async fn version_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let mut extra = std::collections::BTreeMap::new();
    extra.insert("FIXTURE_VERSION".into(), "0.0.0".into());
    let opts = AdapterOptions {
        home: Some(tmp.path().to_path_buf()),
        cli_override: Some(fixture("claude-instruct.py")),
        extra_env: extra,
        ..AdapterOptions::default()
    };
    let err = check_adapter_version(ToolKind::Claude, &opts)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "version-mismatch");
}

#[tokio::test]
async fn tool_not_installed_status() {
    let (_tmp, store, home) = store_and_home();
    let opts = AdapterOptions {
        home: Some(home),
        cli_override: Some(fixture("claude-instruct.py")),
        ..AdapterOptions::default()
    };
    let status = tool_status(&store, ToolKind::Claude, Scope::User, None, &opts)
        .await
        .unwrap();
    assert_eq!(status.status, ToolStatus::NotInstalled);
}

#[tokio::test]
async fn command_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let mut extra = std::collections::BTreeMap::new();
    extra.insert("FIXTURE_FAIL".into(), "1".into());
    let opts = AdapterOptions {
        home: Some(tmp.path().to_path_buf()),
        cli_override: Some(fixture("claude-instruct.py")),
        extra_env: extra,
        ..AdapterOptions::default()
    };
    let envelope = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::Status {
            scope: Scope::User,
            project_dir: None,
            name: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(!envelope.ok);
    assert!(!envelope.blockers.is_empty());
}

#[tokio::test]
async fn user_cancel_activate_without_confirmed_plan() {
    let (_tmp, store, home) = store_and_home();
    let opts = AdapterOptions {
        home: Some(home),
        cli_override: Some(fixture("claude-instruct.py")),
        ..AdapterOptions::default()
    };
    let err = confirm_activate(&store, "missing-plan", &opts)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "user-cancel");

    let created = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Claude,
            title: "Cancel".into(),
            content: "x\n".into(),
            tags: vec![],
        },
    )
    .unwrap();
    let plan = plan_activate(
        &store,
        PlanActivateInput {
            prompt_id: created.id,
            scope: Scope::User,
            project_dir: None,
        },
        &opts,
    )
    .await
    .unwrap();
    confirm_activate(&store, &plan.operation_id, &opts)
        .await
        .unwrap();
    let again = confirm_activate(&store, &plan.operation_id, &opts)
        .await
        .unwrap_err();
    assert_eq!(again.kind(), "user-cancel");
    let _ = Error::user_cancel("activate without confirmed plan");
}
