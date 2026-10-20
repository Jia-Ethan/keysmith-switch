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
        salvage_config: false,
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
            salvage_config: false,
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
            salvage_config: false,
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

/// With `--runtime` the Claude adapter counts only the shell wrapper as installed,
/// so a prompt that was just deployed would read as not deployed.
#[test]
fn claude_status_does_not_ask_for_runtime_alignment() {
    use keysmith_switch_lib::adapter::argv::prepare;
    let prepared = prepare(
        ToolKind::Claude,
        &AdapterCommand::Status {
            scope: Scope::User,
            project_dir: None,
            name: Some("claude-project-rules".into()),
        },
        None,
    )
    .unwrap();
    assert!(
        !prepared.args.iter().any(|item| item == "--runtime"),
        "{:?}",
        prepared.args
    );
}

/// The status text is what the vendored Codex adapter really prints (with `./`
/// in front of the file name), so the instruction file is found from it.
#[test]
fn codex_status_reports_the_instruction_file_it_loads() {
    use keysmith_switch_lib::adapter::normalize::normalize;
    use keysmith_switch_lib::adapter::process::Captured;
    use keysmith_switch_lib::adapter::Envelope;

    let stdout = "\
[Status] Found 1 Codex configuration location(s) (read-only inspection):

── Status directory: /home/u/.codex ──
    config.toml: regular file (/home/u/.codex/config.toml)
    my-rules.md: regular file (/home/u/.codex/my-rules.md)
    hooks.json: missing (/home/u/.codex/hooks.json)
    model_instructions_file: ./my-rules.md
    Config activation: active (the current config loads the managed prompt)
    Transaction residue: none
";
    let run = |text: &str| {
        let captured = Captured {
            stdout: text.into(),
            stderr: String::new(),
            exit_code: 0,
            timed_out: false,
            truncated: false,
            argv: vec!["--status".into()],
        };
        normalize(
            ToolKind::Codex,
            &AdapterCommand::Status {
                scope: Scope::User,
                project_dir: None,
                name: None,
            },
            &captured,
            Envelope::new(ToolKind::Codex, "status"),
        )
    };

    let envelope = run(stdout);
    assert_eq!(envelope.status, ToolStatus::Active);
    let instruction: Vec<_> = envelope
        .target_paths
        .iter()
        .filter(|target| target.role == "instruction")
        .collect();
    assert_eq!(instruction.len(), 1, "{:?}", envelope.target_paths);
    assert!(instruction[0].path.ends_with("my-rules.md"));
    assert!(instruction[0].exists);

    // Nothing is reported when the config does not name a file.
    let unset = run(&stdout.replace("./my-rules.md", "<未设置或无法识别>"));
    assert!(!unset
        .target_paths
        .iter()
        .any(|target| target.role == "instruction"));
}

/// A dry run prints a refused step as `→ [Blocked] reason`; the reason must reach the person,
/// not just `ok=false` / `exit 1`.
#[test]
fn codex_dry_run_blocker_behind_an_arrow_is_reported() {
    use keysmith_switch_lib::adapter::normalize::normalize;
    use keysmith_switch_lib::adapter::process::Captured;
    use keysmith_switch_lib::adapter::Envelope;

    let stdout = "\
[DRY RUN] Preview mode; no files will be changed.

  Target: /home/u/.codex
    → Write MD: /home/u/.codex/gpt-unrestricted.md
    → [Blocked] config.toml not found: /home/u/.codex/config.toml

[Error] dry-run found 1 confirmed blocker(s); no files were changed.
";
    let captured = Captured {
        stdout: stdout.into(),
        stderr: String::new(),
        exit_code: 1,
        timed_out: false,
        truncated: false,
        argv: vec!["--dry-run".into()],
    };
    let envelope = normalize(
        ToolKind::Codex,
        &AdapterCommand::PlanActivate {
            file: "p.md".into(),
            scope: Scope::User,
            project_dir: None,
            name: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
        },
        &captured,
        Envelope::new(ToolKind::Codex, "plan-activate"),
    );
    assert!(!envelope.ok);
    assert!(
        envelope
            .blockers
            .iter()
            .any(|item| item.starts_with("config.toml not found")),
        "{:?}",
        envelope.blockers
    );
}

/// A CLI that stops with nothing on stdout but a reason on stderr: the reason reaches the
/// person. Python's argparse and the ZCode adapter's text mode both write `error: …`; an
/// uncaught exception leaves its message on the last line of the traceback.
#[test]
fn a_refusal_only_on_stderr_is_reported() {
    let failed = |stderr: &str| {
        let captured = Captured {
            stdout: String::new(),
            stderr: stderr.into(),
            exit_code: 2,
            timed_out: false,
            truncated: false,
            argv: vec!["install".into(), "--dry-run".into()],
        };
        normalize(
            ToolKind::Zcode,
            &AdapterCommand::PlanActivate {
                file: "p.md".into(),
                scope: Scope::User,
                project_dir: None,
                name: None,
                runtime: false,
                append_file: None,
                max_tokens: None,
            },
            &captured,
            Envelope::new(ToolKind::Zcode, "plan-activate"),
        )
    };

    let shape = failed(
        "error: ZCode runtime entrypoint shape was not recognized.\n\
         Runtime: C:\\ZCode\\resources\\glm\\zcode.cjs\n\
         The installer expected the runtime context builder anchor used by current ZCode releases.\n",
    );
    assert!(!shape.ok);
    assert_eq!(shape.blockers.len(), 1, "{:?}", shape.blockers);
    assert!(
        shape.blockers[0]
            .starts_with("ZCode runtime entrypoint shape was not recognized. Runtime: "),
        "{:?}",
        shape.blockers
    );

    let traceback = failed(
        "Traceback (most recent call last):\n  File \"zcode-keysmith.py\", line 1, in <module>\n\
         PermissionError: [Errno 13] Permission denied: 'zcode.cjs'\n",
    );
    assert_eq!(
        traceback.blockers,
        vec!["PermissionError: [Errno 13] Permission denied: 'zcode.cjs'".to_string()]
    );

    // Nothing on stderr either: nothing is made up.
    assert!(failed("").blockers.is_empty());
}

fn pi_status() -> AdapterCommand {
    AdapterCommand::Status {
        scope: Scope::User,
        project_dir: None,
        name: None,
    }
}

fn pi_plan(file: &std::path::Path) -> AdapterCommand {
    AdapterCommand::PlanActivate {
        file: file.to_path_buf(),
        scope: Scope::User,
        project_dir: None,
        name: Some("system-prompt".into()),
        runtime: false,
        append_file: None,
        max_tokens: None,
    }
}

fn pi_activate(file: &std::path::Path, token: Option<String>) -> AdapterCommand {
    AdapterCommand::Activate {
        file: file.to_path_buf(),
        scope: Scope::User,
        project_dir: None,
        name: Some("system-prompt".into()),
        runtime: false,
        append_file: None,
        max_tokens: None,
        expected_preview_token: token,
    }
}

fn pi_recover(execute: bool, token: Option<String>) -> AdapterCommand {
    AdapterCommand::Recover {
        scope: Scope::User,
        project_dir: None,
        execute,
        expected_preview_token: token,
    }
}

#[tokio::test]
async fn pi_fixture_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "pi-keysmith.py");
    let pi_dir = home.join(".pi").join("agent");

    // No ~/.pi/agent: not installed, a valid status, and a deploy is blocked without writing.
    let missing = run_adapter_with(ToolKind::Pi, pi_status(), &opts)
        .await
        .unwrap();
    assert_envelope(&missing, ToolKind::Pi, "status");
    assert!(missing.ok, "{missing:?}");
    assert_eq!(missing.status, ToolStatus::NotInstalled);
    assert_eq!(
        arg_after(&missing.argv, "--pi-dir"),
        Some(pi_dir.to_str().unwrap())
    );
    assert!(std::path::Path::new(arg_after(&missing.argv, "--pi-dir").unwrap()).is_absolute());
    for flag in ["--status", "--json"] {
        assert!(missing.argv.iter().any(|item| item == flag), "{flag}");
    }
    assert_eq!(arg_after(&missing.argv, "--lang"), Some("en"));
    let file = home.join("prompt.md");
    std::fs::write(&file, "hello pi\n").unwrap();
    let blocked = run_adapter_with(ToolKind::Pi, pi_plan(&file), &opts)
        .await
        .unwrap();
    assert!(!blocked.ok);
    assert!(
        blocked.blockers[0].starts_with("pi-dir-missing: "),
        "{:?}",
        blocked.blockers
    );
    assert!(blocked.confirmation_token.is_none());
    assert!(!pi_dir.exists());

    // Empty folder: not installed; a preview plans SYSTEM.md and hands out its token.
    std::fs::create_dir_all(&pi_dir).unwrap();
    let plan = run_adapter_with(ToolKind::Pi, pi_plan(&file), &opts)
        .await
        .unwrap();
    assert!(plan.ok && plan.preview, "{plan:?}");
    assert!(plan.argv.iter().any(|item| item == "--dry-run"));
    assert!(!plan.argv.iter().any(|item| item == "--yes"));
    assert!(!plan
        .argv
        .iter()
        .any(|item| item == "--expected-preview-token"));
    assert_eq!(plan.status, ToolStatus::NotInstalled);
    let token = plan.confirmation_token.clone().expect("preview token");
    assert_eq!(token.len(), 64);
    assert_eq!(
        plan.planned_files[0].path,
        pi_dir.join("SYSTEM.md").display().to_string()
    );
    assert!(
        !pi_dir.join("SYSTEM.md").exists(),
        "a preview writes nothing"
    );

    // A confirm needs the preview's token.
    assert!(
        run_adapter_with(ToolKind::Pi, pi_activate(&file, None), &opts)
            .await
            .is_err()
    );
    let confirmed = run_adapter_with(ToolKind::Pi, pi_activate(&file, Some(token.clone())), &opts)
        .await
        .unwrap();
    assert!(confirmed.ok && !confirmed.preview, "{confirmed:?}");
    assert!(confirmed
        .argv
        .windows(2)
        .any(|pair| pair[0] == "--expected-preview-token" && pair[1] == token));
    assert!(confirmed.argv.iter().any(|item| item == "--yes"));
    assert!(!confirmed.argv.iter().any(|item| item == "--dry-run"));
    assert_eq!(confirmed.status, ToolStatus::Active);
    let body_sha = keysmith_switch_lib::models::sha256_hex("hello pi\n");
    assert_eq!(
        confirmed.current_fingerprint.as_deref(),
        Some(body_sha.as_str())
    );

    // The same token again: the state moved on, so exit 3 and nothing written.
    let stale = run_adapter_with(ToolKind::Pi, pi_activate(&file, Some(token)), &opts)
        .await
        .unwrap();
    assert!(!stale.ok);
    assert_eq!(stale.exit_code, 3);
    assert!(!stale.recovery_required);
    assert!(stale.error.as_deref().unwrap().contains("preview again"));

    let active = run_adapter_with(ToolKind::Pi, pi_status(), &opts)
        .await
        .unwrap();
    assert_eq!(active.status, ToolStatus::Active);
    assert!(active.ok);
    assert!(active
        .target_paths
        .iter()
        .any(|target| target.role == "system_file" && target.exists));

    // Hand-edited SYSTEM.md: drift, exit 0, still ok. A deploy plan is not held up by it.
    std::fs::write(pi_dir.join("SYSTEM.md"), "edited\n").unwrap();
    let drift = run_adapter_with(ToolKind::Pi, pi_status(), &opts)
        .await
        .unwrap();
    assert_eq!(drift.status, ToolStatus::Drift);
    assert!(drift.ok);
    assert_eq!(drift.exit_code, 0);
    let redeploy = run_adapter_with(ToolKind::Pi, pi_plan(&file), &opts)
        .await
        .unwrap();
    assert!(redeploy.ok && redeploy.blockers.is_empty(), "{redeploy:?}");
    assert_ne!(redeploy.status, ToolStatus::Drift);

    // Leftover temp file: recovery-required. ok=false and exit 1, yet a valid state.
    std::fs::write(pi_dir.join(".keysmith-leftover"), "x").unwrap();
    let residue = run_adapter_with(ToolKind::Pi, pi_status(), &opts)
        .await
        .unwrap();
    assert_eq!(residue.status, ToolStatus::RecoveryRequired);
    assert!(residue.recovery_required);
    assert!(!residue.ok);
    assert_eq!(residue.exit_code, 1);
    assert!(
        residue.error.is_none(),
        "a state, not a CLI failure: {residue:?}"
    );

    // Recover: preview first, then confirm with its token.
    let preview = run_adapter_with(ToolKind::Pi, pi_recover(false, None), &opts)
        .await
        .unwrap();
    assert!(preview.ok && preview.preview, "{preview:?}");
    assert!(preview.argv.iter().any(|item| item == "--recover"));
    assert!(!preview.argv.iter().any(|item| item == "--yes"));
    assert_eq!(preview.planned_files.len(), 1);
    let recovered = run_adapter_with(
        ToolKind::Pi,
        pi_recover(true, preview.confirmation_token.clone()),
        &opts,
    )
    .await
    .unwrap();
    assert!(recovered.ok && !recovered.preview, "{recovered:?}");
    assert!(!pi_dir.join(".keysmith-leftover").exists());

    // Conflict: also ok=false / exit 1, shown as the state it is.
    std::fs::write(pi_dir.join(".fixture-conflict"), "x").unwrap();
    let conflict = run_adapter_with(ToolKind::Pi, pi_status(), &opts)
        .await
        .unwrap();
    assert_eq!(conflict.status, ToolStatus::Conflict);
    assert!(!conflict.ok);
    assert_eq!(conflict.conflicts, vec!["manifest-invalid".to_string()]);
    assert!(conflict.error.is_none(), "{conflict:?}");
}

#[tokio::test]
async fn pi_busy_lock_and_failed_write_are_told_apart() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let pi_dir = home.join(".pi").join("agent");
    std::fs::create_dir_all(&pi_dir).unwrap();
    let file = home.join("prompt.md");
    std::fs::write(&file, "hello pi\n").unwrap();
    let mut opts = opts(&home, "pi-keysmith.py");
    let plan = run_adapter_with(ToolKind::Pi, pi_plan(&file), &opts)
        .await
        .unwrap();
    let token = plan.confirmation_token.clone().unwrap();

    opts.extra_env
        .insert("FIXTURE_LOCK_BUSY".into(), "1".into());
    let busy = run_adapter_with(ToolKind::Pi, pi_activate(&file, Some(token.clone())), &opts)
        .await
        .unwrap();
    assert!(!busy.ok);
    assert_eq!(busy.exit_code, 5);
    assert!(!busy.recovery_required);
    assert!(busy.error.as_deref().unwrap().contains("try again"));
    assert!(
        busy.blockers[0].starts_with("lock-busy: "),
        "{:?}",
        busy.blockers
    );
    assert!(!pi_dir.join("SYSTEM.md").exists());

    opts.extra_env.remove("FIXTURE_LOCK_BUSY");
    opts.extra_env
        .insert("FIXTURE_WRITE_FAILED".into(), "1".into());
    let failed = run_adapter_with(ToolKind::Pi, pi_activate(&file, Some(token)), &opts)
        .await
        .unwrap();
    assert!(!failed.ok);
    assert_eq!(failed.exit_code, 4);
    assert!(failed.recovery_required);
    assert!(failed.error.as_deref().unwrap().contains("recover"));
}

#[tokio::test]
async fn pi_backups_and_reconcile_are_unsupported() {
    let tmp = tempfile::tempdir().unwrap();
    let opts = opts(tmp.path(), "pi-keysmith.py");
    for command in [
        AdapterCommand::Backups {
            scope: Scope::User,
            project_dir: None,
        },
        AdapterCommand::Reconcile {
            execute: false,
            expected_preview_token: None,
        },
    ] {
        assert!(run_adapter_with(ToolKind::Pi, command, &opts)
            .await
            .is_err());
    }
}

fn pi_captured(stdout: &str, exit_code: i32) -> Captured {
    Captured {
        stdout: stdout.into(),
        stderr: String::new(),
        exit_code,
        timed_out: false,
        truncated: false,
        argv: vec!["--status".into()],
    }
}

/// A pi adapter older than the contract, or one that names another scope, is not trusted.
#[test]
fn pi_envelope_version_and_scope_are_checked() {
    let envelope = |version: &str, scope: &str| {
        format!(
            r#"{{"schema":"pi-keysmith.envelope.v1","tool":"pi-keysmith","version":"{version}","operation":"status","preview":true,"apply":false,"ok":true,"exit_code":0,"target":{{"pi_dir":"/h/.pi/agent","pi_dir_resolved":"/h/.pi/agent","scope":"{scope}"}},"status":{{"state":"active-aligned","recovery_required":false,"nodes":{{}},"conflicts":[],"drift":[],"residue":{{"journals":[],"cleanup_markers":[],"temp_files":[]}},"blockers":{{"deploy":[],"uninstall":[]}},"deployable":true,"uninstall_ready":true}},"plan":null,"result":null,"diagnostics":["windows-beta: beta"]}}"#
        )
    };
    let run = |stdout: String| {
        normalize(
            ToolKind::Pi,
            &pi_status(),
            &pi_captured(&stdout, 0),
            Envelope::new(ToolKind::Pi, "status"),
        )
    };
    let current = run(envelope("0.2.0", "user"));
    assert!(current.ok, "{current:?}");
    assert_eq!(current.status, ToolStatus::Active);
    assert!(
        current.blockers.is_empty(),
        "windows-beta is a warning only"
    );
    assert_eq!(current.warnings, vec!["windows-beta: beta".to_string()]);
    assert!(run(envelope("0.10.1", "user")).ok);

    let old = run(envelope("0.1.0", "user"));
    assert!(!old.ok);
    assert!(old.error.as_deref().unwrap().contains("too old"));

    let project = run(envelope("0.2.0", "project"));
    assert!(!project.ok);
    assert!(project.error.as_deref().unwrap().contains("scope"));
}

/// A token that is not 64 lowercase hex is not passed on.
#[test]
fn pi_token_must_be_lowercase_hex() {
    let stdout = |token: &str| {
        format!(
            r#"{{"schema":"pi-keysmith.envelope.v1","version":"0.2.0","operation":"uninstall","preview":true,"ok":true,"exit_code":0,"target":{{"pi_dir":"/h/.pi/agent","scope":"user"}},"status":{{"state":"active-aligned"}},"plan":{{"confirmation_token":"{token}","blockers":[],"writes":[]}},"result":null,"diagnostics":[]}}"#
        )
    };
    let command = AdapterCommand::PlanDeactivate {
        scope: Scope::User,
        project_dir: None,
        name: None,
        salvage_config: false,
    };
    let token = |value: &str| {
        normalize(
            ToolKind::Pi,
            &command,
            &pi_captured(&stdout(value), 0),
            Envelope::new(ToolKind::Pi, "plan-deactivate"),
        )
        .confirmation_token
    };
    let good = "a".repeat(64);
    assert_eq!(token(&good), Some(good.clone()));
    assert_eq!(token(&"A".repeat(64)), None);
    assert_eq!(token(&"a".repeat(63)), None);
}

// ─── dsh ────────────────────────────────────────────────────────────────────
//
// dsh-keysmith differs from pi in three ways the contract has to pin down:
// it names its own state, it has no preview token, and its adapter hot-fix can
// silently stop applying. These tests are the record of each.

fn dsh_status() -> AdapterCommand {
    AdapterCommand::Status {
        scope: Scope::User,
        project_dir: None,
        name: None,
    }
}

fn dsh_plan(file: &std::path::Path) -> AdapterCommand {
    AdapterCommand::PlanActivate {
        file: file.to_path_buf(),
        scope: Scope::User,
        project_dir: None,
        name: Some("dsh-unrestricted".into()),
        runtime: false,
        append_file: None,
        max_tokens: None,
    }
}

fn dsh_activate(file: &std::path::Path) -> AdapterCommand {
    AdapterCommand::Activate {
        file: file.to_path_buf(),
        scope: Scope::User,
        project_dir: None,
        name: Some("dsh-unrestricted".into()),
        runtime: false,
        append_file: None,
        max_tokens: None,
        expected_preview_token: None,
    }
}

#[tokio::test]
async fn dsh_fixture_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "dsh-keysmith.py");
    let dsh_dir = home.join(".dsh");

    // dsh has never run here: a readable status, not an error, and it says so.
    let missing = run_adapter_with(ToolKind::Dsh, dsh_status(), &opts)
        .await
        .unwrap();
    assert_envelope(&missing, ToolKind::Dsh, "status");
    assert!(missing.ok, "{missing:?}");
    assert_eq!(missing.status, ToolStatus::NotInstalled);
    let root = missing
        .target_paths
        .iter()
        .find(|entry| entry.role == "config-root")
        .expect("config-root");
    assert_eq!(root.path, dsh_dir.to_str().unwrap());
    assert!(!root.exists, "dsh_dir_exists must be reported, not assumed");

    // Every call names --dsh-dir as an absolute path and asks for English.
    for flag in ["--status", "--json"] {
        assert!(missing.argv.iter().any(|item| item == flag), "{flag}");
    }
    assert_eq!(arg_after(&missing.argv, "--lang"), Some("en"));
    let dir_arg = arg_after(&missing.argv, "--dsh-dir").expect("--dsh-dir");
    assert!(std::path::Path::new(dir_arg).is_absolute(), "{dir_arg}");
    assert_eq!(dir_arg, dsh_dir.to_str().unwrap());

    // A preview over an existing home plans the writes and writes nothing.
    std::fs::create_dir_all(&dsh_dir).unwrap();
    let file = home.join("prompt.md");
    std::fs::write(&file, "hello dsh\n").unwrap();
    let plan = run_adapter_with(ToolKind::Dsh, dsh_plan(&file), &opts)
        .await
        .unwrap();
    assert!(plan.ok && plan.preview, "{plan:?}");
    assert!(plan.argv.iter().any(|item| item == "--dry-run"));
    assert!(!plan.argv.iter().any(|item| item == "--yes"));
    // No preview token exists in this adapter, so a confirmation is not bound to a preview.
    assert!(plan.confirmation_token.is_none());
    assert!(!plan
        .argv
        .iter()
        .any(|item| item == "--expected-preview-token"));
    assert!(plan
        .planned_files
        .iter()
        .any(|entry| entry.path.ends_with("AGENTS.md")));
    assert!(!dsh_dir.join("AGENTS.md").exists(), "a preview writes nothing");

    let confirmed = run_adapter_with(ToolKind::Dsh, dsh_activate(&file), &opts)
        .await
        .unwrap();
    assert!(confirmed.ok && !confirmed.preview, "{confirmed:?}");
    assert!(confirmed.argv.iter().any(|item| item == "--yes"));
    assert!(!confirmed.argv.iter().any(|item| item == "--dry-run"));
    assert_eq!(confirmed.status, ToolStatus::Active);
    let body_sha = keysmith_switch_lib::models::sha256_hex("hello dsh\n");
    assert_eq!(
        confirmed.current_fingerprint.as_deref(),
        Some(body_sha.as_str()),
        "the manifest's deployed-prompt sha is what names the live prompt"
    );
    assert!(dsh_dir.join("AGENTS.md").is_file());

    let active = run_adapter_with(ToolKind::Dsh, dsh_status(), &opts)
        .await
        .unwrap();
    assert_eq!(active.status, ToolStatus::Active);
    assert!(active.ok, "{active:?}");
}

#[tokio::test]
async fn dsh_reports_drift_and_a_deploy_still_plans() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "dsh-keysmith.py");
    let dsh_dir = home.join(".dsh");
    std::fs::create_dir_all(&dsh_dir).unwrap();
    let file = home.join("prompt.md");
    std::fs::write(&file, "hello dsh\n").unwrap();
    run_adapter_with(ToolKind::Dsh, dsh_activate(&file), &opts)
        .await
        .unwrap();
    std::fs::write(dsh_dir.join("AGENTS.md"), "hand edited\n").unwrap();

    let drifted = run_adapter_with(ToolKind::Dsh, dsh_status(), &opts)
        .await
        .unwrap();
    assert_eq!(drifted.status, ToolStatus::Drift);
    assert!(drifted.ok, "drift is a state, not a failure: {drifted:?}");
    assert!(
        drifted.warnings.iter().any(|note| note.contains("agents_md")),
        "{:?}",
        drifted.warnings
    );

    // dsh-keysmith backs the file up and replaces it, so drift does not block a deploy.
    // Showing Drift on the plan would read as a repair to make first.
    let plan = run_adapter_with(ToolKind::Dsh, dsh_plan(&file), &opts)
        .await
        .unwrap();
    assert!(plan.ok, "{plan:?}");
    assert_eq!(plan.status, ToolStatus::Inactive);
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);

    // Uninstalling it is a different matter: the adapter refuses, and says why.
    let blocked = run_adapter_with(
        ToolKind::Dsh,
        AdapterCommand::PlanDeactivate {
            scope: Scope::User,
            project_dir: None,
            name: None,
            salvage_config: false,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(!blocked.ok, "{blocked:?}");
    assert!(
        blocked.blockers.iter().any(|item| item.contains("drifted")),
        "{:?}",
        blocked.blockers
    );
}

#[tokio::test]
async fn dsh_rejects_an_adapter_without_the_state_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(home.join(".dsh")).unwrap();
    // 0.2.0 shipped dsh-keysmith/v1 without `version`, `state` or `dsh_dir_exists`.
    let mut opts = opts(&home, "dsh-keysmith.py");
    opts.extra_env
        .insert("FIXTURE_VERSION".into(), "0.2.0".into());
    let old = run_adapter_with(ToolKind::Dsh, dsh_status(), &opts)
        .await
        .unwrap();
    assert!(!old.ok, "{old:?}");
    assert!(
        old.error.as_deref().unwrap_or("").contains("too old"),
        "{:?}",
        old.error
    );
}

#[tokio::test]
async fn dsh_reports_recovery_required_from_residue() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "dsh-keysmith.py");
    let dsh_dir = home.join(".dsh");
    std::fs::create_dir_all(dsh_dir.join(".dsh-keysmith-transaction-deadbeef")).unwrap();

    let status = run_adapter_with(ToolKind::Dsh, dsh_status(), &opts)
        .await
        .unwrap();
    assert_eq!(status.status, ToolStatus::RecoveryRequired);
    assert!(status.recovery_required);
    assert!(!status.ok);

    // Recover is offered, and a preview of it writes nothing.
    let plan = run_adapter_with(
        ToolKind::Dsh,
        AdapterCommand::Recover {
            scope: Scope::User,
            project_dir: None,
            execute: false,
            expected_preview_token: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(plan.ok && plan.preview, "{plan:?}");
    assert!(dsh_dir
        .join(".dsh-keysmith-transaction-deadbeef")
        .is_dir());

    let done = run_adapter_with(
        ToolKind::Dsh,
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
    assert!(done.ok && !done.preview, "{done:?}");
    assert!(!dsh_dir
        .join(".dsh-keysmith-transaction-deadbeef")
        .exists());
}

#[tokio::test]
async fn dsh_surfaces_a_skipped_adapter_hotfix() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(home.join(".dsh")).unwrap();
    let mut opts = opts(&home, "dsh-keysmith.py");
    opts.extra_env
        .insert("FIXTURE_ADAPTER_STATE".into(), "unknown".into());

    let status = run_adapter_with(ToolKind::Dsh, dsh_status(), &opts)
        .await
        .unwrap();
    // The patch being skipped does not make the deployment unusable; it must not read as
    // silent success either.
    assert!(status.ok, "{status:?}");
    assert!(
        status
            .warnings
            .iter()
            .any(|note| note.contains("streaming tool-call fix was skipped")),
        "{:?}",
        status.warnings
    );
}

#[tokio::test]
async fn dsh_unsupported_commands_say_so() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let opts = opts(&home, "dsh-keysmith.py");
    for command in [
        AdapterCommand::Backups {
            scope: Scope::User,
            project_dir: None,
        },
        AdapterCommand::Restore {
            target: home.join("AGENTS.md"),
            backup: "x".into(),
            scope: Some(Scope::User),
            project_dir: None,
            execute: false,
        },
    ] {
        assert!(run_adapter_with(ToolKind::Dsh, command, &opts)
            .await
            .is_err());
    }
}
