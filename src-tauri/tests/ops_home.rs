use std::path::PathBuf;

use keysmith_switch_lib::adapter::AdapterOptions;
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::lock::HomeLock;
use keysmith_switch_lib::models::UpdatePromptInput;
use keysmith_switch_lib::models::{
    CreatePromptInput, PlanActivateInput, PlanDeactivateInput, Scope, ToolKind, ToolStatus,
};
use keysmith_switch_lib::ops::{
    confirm_activate, confirm_deactivate, confirm_recover, create_prompt, plan_activate,
    plan_deactivate, plan_recover, restore_prompt_version, tool_status, update_prompt,
};
use keysmith_switch_lib::paths::AppPaths;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cli")
        .join(name)
}

fn setup(tool: ToolKind) -> (tempfile::TempDir, Store, AdapterOptions) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("user-home");
    std::fs::create_dir_all(&home).unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();
    let script = match tool {
        ToolKind::Claude => "claude-instruct.py",
        ToolKind::Codex => "codex-instruct.py",
        ToolKind::Grok => "grok-keysmith.py",
        ToolKind::Zcode => "zcode-keysmith.py",
    };
    let opts = AdapterOptions {
        home: Some(home),
        cli_override: Some(fixture(script)),
        extra_env: [(
            tool.env_cli_key().to_string(),
            fixture(script).display().to_string(),
        )]
        .into_iter()
        .collect(),
        ..AdapterOptions::default()
    };
    (tmp, store, opts)
}

#[tokio::test]
async fn temp_home_activate_deactivate_restore_recover_drift_lock() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let created = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Claude,
            title: "Home Prompt".into(),
            content: "managed body\n".into(),
            tags: vec!["t".into()],
        },
    )
    .unwrap();

    let plan = plan_activate(
        &store,
        PlanActivateInput {
            prompt_id: created.id.clone(),
            scope: Scope::User,
            project_dir: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(plan.envelope.preview);
    assert!(!plan.envelope.argv.iter().any(|item| item == "--yes"));

    let activated = confirm_activate(&store, &plan.operation_id, &opts)
        .await
        .unwrap();
    assert!(activated.envelope.ok);
    assert!(activated.envelope.argv.iter().any(|item| item == "--yes"));

    let status = tool_status(&store, ToolKind::Claude, Scope::User, None, &opts)
        .await
        .unwrap();
    assert_eq!(status.status, ToolStatus::Active);

    let updated = update_prompt(
        &store,
        UpdatePromptInput {
            id: created.id.clone(),
            title: None,
            content: Some("second body\n".into()),
            tags: None,
        },
    )
    .unwrap();
    assert_eq!(updated.version, 2);
    let restored = restore_prompt_version(&store, &created.id, 1).unwrap();
    assert!(restored.content.contains("managed body"));
    assert_eq!(restored.version, 3);

    let instruction = opts
        .home
        .as_ref()
        .unwrap()
        .join(".claude/keysmith/claude-project-rules.md");
    if instruction.exists() {
        let mut body = std::fs::read_to_string(&instruction).unwrap();
        body.push_str("unmanaged\n");
        std::fs::write(&instruction, body).unwrap();
    }

    let deactivate_plan = plan_deactivate(
        &store,
        PlanDeactivateInput {
            prompt_id: Some(created.id.clone()),
            tool: ToolKind::Claude,
            scope: Scope::User,
            project_dir: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(
        deactivate_plan.envelope.recovery_required
            || deactivate_plan.envelope.status == ToolStatus::Drift
            || !deactivate_plan.envelope.ok
    );
    let refused = confirm_deactivate(&store, &deactivate_plan.operation_id, &opts).await;
    assert!(refused.is_err(), "{refused:?}");

    let recover_plan = plan_recover(&store, ToolKind::Claude, Scope::User, None, &opts)
        .await
        .unwrap();
    assert!(recover_plan.envelope.preview);
    assert!(!recover_plan
        .envelope
        .argv
        .iter()
        .any(|item| item == "--yes"));
    let recovered = confirm_recover(&store, &recover_plan.operation_id, &opts)
        .await
        .unwrap();
    assert!(recovered.envelope.ok || recovered.envelope.command == "recover");
    assert!(recovered.envelope.argv.iter().any(|item| item == "--yes"));

    let lock1 = HomeLock::acquire(store.paths()).unwrap();
    let conflict = HomeLock::try_acquire(store.paths());
    assert!(conflict.is_err());
    drop(lock1);
    let lock2 = HomeLock::try_acquire(store.paths()).unwrap();
    drop(lock2);
}
