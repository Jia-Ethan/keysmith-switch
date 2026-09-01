use std::path::PathBuf;

use keysmith_switch_lib::adapter::{
    run_adapter_with, AdapterCommand, AdapterOptions, Envelope, ToolKind,
};
use keysmith_switch_lib::models::{Scope, ToolStatus, ADAPTER_SCHEMA};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cli")
        .join(name)
}

fn opts(home: &std::path::Path, script: &str) -> AdapterOptions {
    AdapterOptions {
        home: Some(home.to_path_buf()),
        cli_override: Some(fixture(script)),
        ..AdapterOptions::default()
    }
}

fn assert_envelope(env: &Envelope, tool: ToolKind, command: &str) {
    assert_eq!(env.schema, ADAPTER_SCHEMA);
    assert_eq!(env.tool, tool);
    assert_eq!(env.command, command);
    assert!(!env.argv.is_empty());
    assert_eq!(env.argv[0].contains("python"), true);
}

#[tokio::test]
async fn claude_fixture_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "claude-instruct.py");
    let status = run_adapter_with(
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
    assert_envelope(&status, ToolKind::Claude, "status");
    assert!(status.preview);
    assert_eq!(status.status, ToolStatus::NotInstalled);
    assert!(!status.argv.iter().any(|item| item == "--yes"));

    let file = home.join("prompt.md");
    std::fs::write(&file, "hello claude\n").unwrap();
    let plan = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::PlanActivate {
            file: file.clone(),
            scope: Scope::User,
            project_dir: None,
            name: Some("claude-project-rules".into()),
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(plan.preview);
    assert!(plan.ok);
    assert!(!plan.argv.iter().any(|item| item == "--yes"));
    assert!(!plan.planned_files.is_empty());
}

#[tokio::test]
async fn grok_fixture_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "grok-keysmith.py");
    let status = run_adapter_with(
        ToolKind::Grok,
        AdapterCommand::Status {
            scope: Scope::User,
            project_dir: None,
            name: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert_envelope(&status, ToolKind::Grok, "status");
    assert_eq!(status.status, ToolStatus::NotInstalled);
    assert!(status.argv.iter().any(|item| item == "--json"));
    assert!(status.argv.iter().any(|item| item == "--grok-dir"));
}

#[tokio::test]
async fn codex_fixture_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "codex-instruct.py");
    let status = run_adapter_with(
        ToolKind::Codex,
        AdapterCommand::Status {
            scope: Scope::User,
            project_dir: None,
            name: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert_envelope(&status, ToolKind::Codex, "status");
    assert_eq!(status.status, ToolStatus::NotInstalled);
    assert!(status.argv.iter().any(|item| item == "--lang"));
    assert!(status.argv.iter().any(|item| item == "en"));
}

#[tokio::test]
async fn zcode_fixture_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "zcode-keysmith.py");
    let doctor = run_adapter_with(ToolKind::Zcode, AdapterCommand::Doctor, &opts)
        .await
        .unwrap();
    assert_envelope(&doctor, ToolKind::Zcode, "doctor");
    assert_eq!(doctor.status, ToolStatus::NotInstalled);

    let recover = run_adapter_with(
        ToolKind::Zcode,
        AdapterCommand::Recover {
            scope: Scope::User,
            project_dir: None,
            execute: true,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(!recover.ok);
    assert!(recover
        .unavailable_reason
        .as_deref()
        .unwrap_or_default()
        .contains("recover"));
}
