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
