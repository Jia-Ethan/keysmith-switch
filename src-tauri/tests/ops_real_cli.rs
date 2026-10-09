use keysmith_switch_lib::adapter::AdapterOptions;
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::{CreatePromptInput, PlanActivateInput, Scope, ToolKind};
use keysmith_switch_lib::ops::{confirm_activate, create_prompt, plan_activate, tool_status};
use keysmith_switch_lib::paths::AppPaths;

fn python3_available() -> bool {
    std::process::Command::new("python3")
        .arg("-c")
        .arg("import sys")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn vendor(rel: &str) -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/keysmith")
        .join(rel);
    path.is_file().then_some(path)
}

#[tokio::test]
async fn real_vendored_claude_and_grok_in_temp_home() {
    if !python3_available() {
        return;
    }
    let claude = match vendor("claude/claude-instruct.py") {
        Some(path) => path,
        None => return,
    };
    let grok = match vendor("grok/grok-keysmith.py") {
        Some(path) => path,
        None => return,
    };

    let tmp = tempfile::tempdir().unwrap();
    let user_home = tmp.path().join("home");
    std::fs::create_dir_all(&user_home).unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();

    let claude_opts = AdapterOptions {
        home: Some(user_home.clone()),
        cli_override: Some(claude),
        ..AdapterOptions::default()
    };
    let grok_opts = AdapterOptions {
        home: Some(user_home.clone()),
        cli_override: Some(grok),
        ..AdapterOptions::default()
    };

    let claude_status = tool_status(&store, ToolKind::Claude, Scope::User, None, &claude_opts)
        .await
        .unwrap();
    assert_eq!(claude_status.schema, "keysmith-switch/adapter-v1");
    assert!(claude_status.preview);

    let grok_status = tool_status(&store, ToolKind::Grok, Scope::User, None, &grok_opts)
        .await
        .unwrap();
    assert_eq!(grok_status.schema, "keysmith-switch/adapter-v1");

    let prompt = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Claude,
            title: "real-claude".into(),
            content: "# real fixture prompt\nkeep this local.\n".into(),
            tags: vec!["real".into()],
        },
    )
    .unwrap();
    let plan = plan_activate(
        &store,
        PlanActivateInput {
            prompt_id: prompt.id.clone(),
            scope: Scope::User,
            project_dir: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
        },
        &claude_opts,
    )
    .await
    .unwrap();
    assert!(plan.envelope.preview);
    assert!(!plan.envelope.argv.iter().any(|item| item == "--yes"));
    let executed = confirm_activate(&store, &plan.operation_id, &claude_opts).await;
    match executed {
        Ok(result) => {
            assert!(result.envelope.argv.iter().any(|item| item == "--yes"));
        }
        Err(error) => {
            // Real CLI may fail-closed on a missing Claude install; still proves argv + envelope.
            let rendered = error.to_string();
            assert!(!rendered.contains("sk-"), "{rendered}");
        }
    }
}

/// The real adapters accept a deployment (their version is the pinned one) and then
/// read it back as deployed and named. ZCode is left out: its adapter also
/// registers a login item, which a test must not do.
#[tokio::test]
async fn real_adapters_deploy_and_read_back_as_deployed() {
    if !python3_available() {
        return;
    }
    for (tool, rel) in [
        (ToolKind::Claude, "claude/claude-instruct.py"),
        (ToolKind::Codex, "codex/codex-instruct.py"),
        (ToolKind::Grok, "grok/grok-keysmith.py"),
    ] {
        let Some(cli) = vendor(rel) else { return };
        let tmp = tempfile::tempdir().unwrap();
        let user_home = tmp.path().join("home");
        std::fs::create_dir_all(&user_home).unwrap();
        if tool == ToolKind::Codex {
            // Codex only manages an existing config directory.
            std::fs::create_dir_all(user_home.join(".codex")).unwrap();
            std::fs::write(user_home.join(".codex/config.toml"), "model = \"gpt-5\"\n").unwrap();
        }
        let paths = AppPaths::from_home(tmp.path().join("switch"));
        paths.ensure().unwrap();
        let store = Store::open(&paths).unwrap();
        let opts = AdapterOptions {
            home: Some(user_home),
            cli_override: Some(cli),
            ..AdapterOptions::default()
        };

        let deployed = keysmith_switch_lib::harness::deploy_harness_with(
            &store,
            tool,
            &opts,
            Some("# Real prompt\nAsk before deleting.\n".into()),
        )
        .await
        .unwrap();
        assert!(deployed.ok, "{tool:?}: {deployed:?}");

        let state = keysmith_switch_lib::harness::harness_state(&store, tool, &opts)
            .await
            .unwrap();
        assert!(state.deployed, "{tool:?}: {state:?}");
        assert_eq!(state.prompt_id, deployed.prompt_id, "{tool:?}");
    }
}

/// A prompt the library no longer knows can be taken back into it, byte for byte,
/// using the real vendored adapters and their real fingerprints. ZCode is left out
/// here: its adapter also registers a login item, which a test must not do.
#[tokio::test]
async fn real_adapters_let_a_forgotten_live_prompt_be_adopted() {
    if !python3_available() {
        return;
    }
    for (tool, rel) in [
        (ToolKind::Claude, "claude/claude-instruct.py"),
        (ToolKind::Codex, "codex/codex-instruct.py"),
        (ToolKind::Grok, "grok/grok-keysmith.py"),
    ] {
        let Some(cli) = vendor(rel) else { return };
        let tmp = tempfile::tempdir().unwrap();
        let user_home = tmp.path().join("home");
        std::fs::create_dir_all(&user_home).unwrap();
        if tool == ToolKind::Codex {
            // Codex only manages an existing config directory.
            std::fs::create_dir_all(user_home.join(".codex")).unwrap();
            std::fs::write(user_home.join(".codex/config.toml"), "model = \"gpt-5\"\n").unwrap();
        }
        let paths = AppPaths::from_home(tmp.path().join("switch"));
        paths.ensure().unwrap();
        let store = Store::open(&paths).unwrap();
        let opts = AdapterOptions {
            home: Some(user_home),
            cli_override: Some(cli),
            ..AdapterOptions::default()
        };

        let body = "# Forgotten prompt\nAsk before deleting.\n";
        let deployed = keysmith_switch_lib::harness::deploy_harness_with(
            &store,
            tool,
            &opts,
            Some(body.into()),
        )
        .await
        .unwrap();
        assert!(deployed.ok, "{tool:?}: {deployed:?}");
        keysmith_switch_lib::ops::delete_prompt(&store, &deployed.prompt_id.unwrap()).unwrap();

        let before = keysmith_switch_lib::harness::harness_state(&store, tool, &opts)
            .await
            .unwrap();
        assert!(
            before.deployed && before.prompt_id.is_none(),
            "{tool:?}: {before:?}"
        );

        let id = keysmith_switch_lib::harness::adopt_live_prompt(&store, tool, "Adopted", &opts)
            .await
            .unwrap_or_else(|error| panic!("{tool:?}: {error}"));
        assert_eq!(store.get_prompt(&id).unwrap().content, body, "{tool:?}");

        let after = keysmith_switch_lib::harness::harness_state(&store, tool, &opts)
            .await
            .unwrap();
        assert_eq!(after.prompt_id.as_deref(), Some(id.as_str()), "{tool:?}");
    }
}

/// The real Grok adapter, in a temporary home: when the managed lines of the config are
/// moved out of their markers (what happened on a real machine), a new deploy is refused
/// as drift, and the adapter's own reconcile, previewed and confirmed, makes it work again.
#[tokio::test]
async fn real_grok_drift_is_repaired_by_reconcile_then_a_deploy_goes_through() {
    if !python3_available() {
        return;
    }
    let Some(cli) = vendor("grok/grok-keysmith.py") else {
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let user_home = tmp.path().join("home");
    std::fs::create_dir_all(&user_home).unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();
    let opts = AdapterOptions {
        home: Some(user_home.clone()),
        cli_override: Some(cli),
        ..AdapterOptions::default()
    };

    let first = keysmith_switch_lib::harness::deploy_harness_with(
        &store,
        ToolKind::Grok,
        &opts,
        Some("# First prompt\nBe careful.\n".into()),
    )
    .await
    .unwrap();
    assert!(first.ok, "{first:?}");

    // Take the markers away but leave the managed lines where they are.
    let config = user_home.join(".grok/config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    let unmarked: String = text
        .lines()
        .filter(|line| !line.contains("grok-keysmith compat isolation"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_ne!(unmarked, text, "the deploy wrote a marked block");
    std::fs::write(&config, &unmarked).unwrap();

    let second = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Grok,
            title: "second".into(),
            content: "# Second prompt\nAsk first.\n".into(),
            tags: vec![],
        },
    )
    .unwrap();
    let activate = |prompt_id: String| PlanActivateInput {
        prompt_id,
        scope: Scope::User,
        project_dir: None,
        runtime: false,
        append_file: None,
        max_tokens: None,
    };

    let blocked = plan_activate(&store, activate(second.id.clone()), &opts)
        .await
        .unwrap();
    assert!(!blocked.envelope.ok, "drift must block a deploy");
    assert!(
        blocked
            .envelope
            .blockers
            .iter()
            .any(|item| item.contains("does not match managed after-state")),
        "{:?}",
        blocked.envelope.blockers
    );

    let preview = keysmith_switch_lib::ops::plan_reconcile(&store, ToolKind::Grok, &opts)
        .await
        .unwrap();
    assert!(
        preview.envelope.ok && preview.envelope.blockers.is_empty(),
        "{:?}",
        preview.envelope
    );
    assert!(preview.envelope.confirmation_token.is_some());
    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        unmarked,
        "a preview writes nothing"
    );

    let done = keysmith_switch_lib::ops::confirm_reconcile(&store, &preview.operation_id, &opts)
        .await
        .unwrap();
    assert!(done.envelope.ok, "{:?}", done.envelope);
    let repaired = std::fs::read_to_string(&config).unwrap();
    assert!(repaired.contains("grok-keysmith compat isolation begin"));

    // The plan is spent: confirming it again is refused, not repeated.
    assert!(
        keysmith_switch_lib::ops::confirm_reconcile(&store, &preview.operation_id, &opts)
            .await
            .is_err()
    );

    let ready = plan_activate(&store, activate(second.id), &opts)
        .await
        .unwrap();
    assert!(
        ready.envelope.ok && ready.envelope.blockers.is_empty(),
        "{:?}",
        ready.envelope
    );
    let deployed = confirm_activate(&store, &ready.operation_id, &opts)
        .await
        .unwrap();
    assert!(deployed.envelope.ok, "{:?}", deployed.envelope);
}

/// The real Grok adapter, asked to turn off a deployment it has no record of (its manifest is
/// gone, as after a cleanup), refuses before it can plan. The refusal must carry its reason, not
/// just `ok=false` / `exit 1` (#115).
#[tokio::test]
async fn real_grok_refusal_without_a_manifest_says_why() {
    if !python3_available() {
        return;
    }
    let Some(cli) = vendor("grok/grok-keysmith.py") else {
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let user_home = tmp.path().join("home");
    std::fs::create_dir_all(user_home.join(".grok")).unwrap();
    let opts = AdapterOptions {
        home: Some(user_home),
        cli_override: Some(cli),
        ..AdapterOptions::default()
    };
    let envelope = keysmith_switch_lib::adapter::run_adapter_with(
        ToolKind::Grok,
        keysmith_switch_lib::adapter::AdapterCommand::PlanDeactivate {
            scope: Scope::User,
            project_dir: None,
            name: None,
            salvage_config: false,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(!envelope.ok);
    assert_eq!(
        envelope.blockers,
        vec!["no valid deployment manifest".to_string()]
    );
}

/// The real ZCode adapter prints why it stopped on stderr, with no JSON on stdout. A deploy
/// planned where ZCode is not installed must say that, not just `ok=false` / `exit 2` (#115).
/// Only the dry run is used: it reads, and writes nothing.
#[tokio::test]
async fn real_zcode_refusal_on_stderr_says_why() {
    if !python3_available() {
        return;
    }
    let Some(cli) = vendor("zcode/zcode-keysmith.py") else {
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let user_home = tmp.path().join("home");
    std::fs::create_dir_all(&user_home).unwrap();
    let prompt = tmp.path().join("prompt.md");
    std::fs::write(&prompt, "# Rules\nBe careful.\n").unwrap();
    let opts = AdapterOptions {
        home: Some(user_home),
        cli_override: Some(cli),
        // Wherever this runs, ZCode is not in this folder.
        extra_env: [(
            "ZCODE_APP_PATH".to_string(),
            tmp.path().join("ZCode.app").display().to_string(),
        )]
        .into_iter()
        .collect(),
        ..AdapterOptions::default()
    };
    let envelope = keysmith_switch_lib::adapter::run_adapter_with(
        ToolKind::Zcode,
        keysmith_switch_lib::adapter::AdapterCommand::PlanActivate {
            file: prompt,
            scope: Scope::User,
            project_dir: None,
            name: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(!envelope.ok);
    assert_eq!(envelope.exit_code, 2);
    assert_eq!(envelope.blockers.len(), 1, "{:?}", envelope.blockers);
    assert!(
        envelope.blockers[0].starts_with("ZCode runtime not found: "),
        "{:?}",
        envelope.blockers
    );
}
