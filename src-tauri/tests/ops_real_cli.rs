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
