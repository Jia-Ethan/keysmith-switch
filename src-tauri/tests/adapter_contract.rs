use std::path::PathBuf;

use keysmith_switch_lib::adapter::normalize::normalize;
use keysmith_switch_lib::adapter::process::Captured;
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
            runtime: false,
            append_file: None,
            max_tokens: None,
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

fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
    argv.iter()
        .position(|item| item == flag)
        .and_then(|index| argv.get(index + 1))
        .map(String::as_str)
}

#[tokio::test]
async fn claude_install_runtime_flags_preview_before_yes() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let file = home.join("prompt.md");
    let append = home.join("append.md");
    std::fs::write(&file, "hello\n").unwrap();
    std::fs::write(&append, "more\n").unwrap();
    let opts = opts(&home, "claude-instruct.py");
    let plan = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::PlanActivate {
            file: file.clone(),
            scope: Scope::User,
            project_dir: None,
            name: Some("claude-project-rules".into()),
            runtime: true,
            append_file: Some(append.clone()),
            max_tokens: Some(8192),
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(plan.preview, "{plan:?}");
    assert!(plan.argv.iter().any(|item| item == "--runtime"));
    assert_eq!(
        arg_after(&plan.argv, "--append-file"),
        Some(append.to_str().unwrap())
    );
    assert_eq!(arg_after(&plan.argv, "--max-tokens"), Some("8192"));
    let yes_at = plan.argv.iter().position(|item| item == "--yes");
    let runtime_at = plan.argv.iter().position(|item| item == "--runtime");
    assert!(yes_at.is_none());
    assert!(runtime_at.is_some());

    let activate = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::Activate {
            file,
            scope: Scope::User,
            project_dir: None,
            name: Some("claude-project-rules".into()),
            runtime: true,
            append_file: Some(append),
            max_tokens: Some(8192),
            expected_preview_token: None,
        },
        &opts,
    )
    .await
    .unwrap();
    let yes_at = activate
        .argv
        .iter()
        .position(|item| item == "--yes")
        .unwrap();
    let runtime_at = activate
        .argv
        .iter()
        .position(|item| item == "--runtime")
        .unwrap();
    assert!(runtime_at < yes_at);
    assert!(!activate.preview);
}

#[tokio::test]
async fn claude_backups_and_restore_keep_preview_first() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "claude-instruct.py");
    let backups = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::Backups {
            scope: Scope::User,
            project_dir: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(backups.preview);
    assert!(backups.argv.iter().any(|item| item == "backups"));
    assert!(backups.argv.iter().any(|item| item == "--json"));
    assert!(!backups.argv.iter().any(|item| item == "--yes"));

    let target = home.join("CLAUDE.md");
    let restore = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::Restore {
            target: target.clone(),
            backup: "CLAUDE.md.bak_20260922".into(),
            scope: Some(Scope::User),
            project_dir: None,
            execute: false,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(restore.preview, "{restore:?}");
    assert_eq!(
        arg_after(&restore.argv, "--backup"),
        Some("CLAUDE.md.bak_20260922")
    );
    assert!(!restore.argv.iter().any(|item| item == "--yes"));

    let execute = run_adapter_with(
        ToolKind::Claude,
        AdapterCommand::Restore {
            target,
            backup: "CLAUDE.md.bak_20260922".into(),
            scope: Some(Scope::User),
            project_dir: None,
            execute: true,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(execute.argv.iter().any(|item| item == "--yes"));
    assert!(!execute.preview);
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

const GROK_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn grok_confirm_forwards_envelope_token_and_reconcile_previews_first() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "grok-keysmith.py");
    let file = home.join("prompt.md");
    std::fs::write(&file, "hello grok\n").unwrap();

    let missing = run_adapter_with(
        ToolKind::Grok,
        AdapterCommand::Activate {
            file: file.clone(),
            scope: Scope::User,
            project_dir: None,
            name: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
            expected_preview_token: None,
        },
        &opts,
    )
    .await;
    assert!(
        missing.is_err(),
        "confirm without the envelope token must not run"
    );

    let activate = run_adapter_with(
        ToolKind::Grok,
        AdapterCommand::Activate {
            file,
            scope: Scope::User,
            project_dir: None,
            name: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
            expected_preview_token: Some(GROK_TOKEN.into()),
        },
        &opts,
    )
    .await
    .unwrap();
    let token_at = activate
        .argv
        .iter()
        .position(|item| item == "--expected-preview-token")
        .unwrap();
    let yes_at = activate
        .argv
        .iter()
        .position(|item| item == "--yes")
        .unwrap();
    assert!(token_at < yes_at);
    assert_eq!(activate.argv[token_at + 1], GROK_TOKEN);
    assert!(!activate.preview);

    let preview = run_adapter_with(
        ToolKind::Grok,
        AdapterCommand::Reconcile {
            execute: false,
            expected_preview_token: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(preview.preview, "{preview:?}");
    assert!(preview.argv.iter().any(|item| item == "--reconcile"));
    assert!(!preview.argv.iter().any(|item| item == "--yes"));
    assert!(!preview
        .argv
        .iter()
        .any(|item| item == "--expected-preview-token"));

    let confirm = run_adapter_with(
        ToolKind::Grok,
        AdapterCommand::Reconcile {
            execute: true,
            expected_preview_token: Some(GROK_TOKEN.into()),
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(confirm.argv.iter().any(|item| item == "--reconcile"));
    assert!(confirm
        .argv
        .windows(2)
        .any(|pair| pair[0] == "--expected-preview-token" && pair[1] == GROK_TOKEN));
    assert!(confirm.argv.iter().any(|item| item == "--yes"));
    assert!(!confirm.preview);
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
            expected_preview_token: None,
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

#[tokio::test]
async fn zcode_032_json_uninstall_parses() {
    let command = AdapterCommand::PlanDeactivate {
        scope: Scope::User,
        project_dir: None,
        name: None,
    };
    let captured = Captured {
        argv: vec![
            "uninstall".into(),
            "--dry-run".into(),
            "--json".into(),
        ],
        truncated: false,
        stdout: r#"{
            "schema": "zcode-keysmith/v1",
            "operation": "uninstall",
            "mode": "preview",
            "ok": true,
            "actions": [{"action": "plan", "path": "/tmp/managed/system-role.md", "detail": "target"}],
            "warnings": [],
            "blockers": [],
            "exit_status": 0,
            "error": null,
            "managed_dir": "/tmp/managed",
            "write": false,
            "removed": [],
            "backups": []
        }"#
        .into(),
        stderr: String::new(),
        exit_code: 0,
        timed_out: false,
    };
    let envelope = Envelope::new(ToolKind::Zcode, "plan-deactivate");
    let parsed = normalize(ToolKind::Zcode, &command, &captured, envelope);
    assert!(parsed.ok, "{parsed:?}");
    assert!(parsed.preview);
    assert_eq!(parsed.planned_files.len(), 1);
    assert_eq!(parsed.planned_files[0].action, "plan");
    assert!(parsed
        .target_paths
        .iter()
        .any(|path| path.role == "managed_dir"));
    assert!(captured.argv.iter().any(|item| item == "--json"));
    let json_at = captured.argv.iter().position(|item| item == "--json");
    let yes_at = captured.argv.iter().position(|item| item == "--yes");
    assert!(json_at.is_some());
    assert!(yes_at.is_none());
}

#[tokio::test]
async fn zcode_uninstall_json_preview_before_yes() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "zcode-keysmith.py");
    let preview = run_adapter_with(
        ToolKind::Zcode,
        AdapterCommand::PlanDeactivate {
            scope: Scope::User,
            project_dir: None,
            name: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(preview.preview, "{preview:?}");
    assert!(preview.ok, "{preview:?}");
    assert!(preview.argv.iter().any(|item| item == "--json"));
    assert!(preview.argv.iter().any(|item| item == "--dry-run"));
    assert!(!preview.argv.iter().any(|item| item == "--yes"));
    assert!(preview
        .planned_files
        .iter()
        .any(|file| file.action == "plan"));
    assert!(!preview.argv.iter().any(|item| item == "doctor"));

    let confirm = run_adapter_with(
        ToolKind::Zcode,
        AdapterCommand::Deactivate {
            scope: Scope::User,
            project_dir: None,
            name: None,
            expected_preview_token: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(!confirm.preview, "{confirm:?}");
    assert!(confirm.ok, "{confirm:?}");
    let json_at = confirm.argv.iter().position(|item| item == "--json");
    let yes_at = confirm.argv.iter().position(|item| item == "--yes");
    assert!(json_at.is_some() && yes_at.is_some());
    assert!(json_at.unwrap() > yes_at.unwrap());
    assert_eq!(confirm.status, ToolStatus::NotInstalled);

    let doctor = run_adapter_with(ToolKind::Zcode, AdapterCommand::Doctor, &opts)
        .await
        .unwrap();
    assert!(!doctor.argv.iter().any(|item| item == "--json"));
    assert!(doctor.argv.iter().any(|item| item == "doctor"));
}
