use std::path::PathBuf;

use keysmith_switch_lib::adapter::AdapterOptions;
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::harness::{
    accept_prompt_body, deploy_harness_with, harness_source, harness_state, remove_harness,
    store_harness_prompt, HarnessAction, MAX_PROMPT_BYTES,
};
use keysmith_switch_lib::models::{PromptSort, Scope, ToolKind, ToolStatus};
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

#[test]
fn catalog_points_at_the_current_default_prompt() {
    let cases = [
        (
            ToolKind::Claude,
            "claude-keysmith",
            "main",
            "examples/claude-project-rules.md",
        ),
        (
            ToolKind::Codex,
            "codex-keysmith",
            "main",
            "examples/gpt-overlay.md",
        ),
        (
            ToolKind::Grok,
            "grok-keysmith",
            "main",
            "examples/grok-unrestricted.md",
        ),
        (
            ToolKind::Zcode,
            "zcode-keysmith",
            "master",
            "examples/system-role.md",
        ),
    ];
    for (tool, repo, branch, path) in cases {
        let source = harness_source(tool);
        assert_eq!(source.repo, repo);
        assert_eq!(source.branch, branch);
        assert_eq!(source.path, path);
        assert_eq!(
            source.url(),
            format!("https://raw.githubusercontent.com/Jia-Ethan/{repo}/{branch}/{path}")
        );
    }
}

#[test]
fn prompt_body_rejects_empty_and_oversized() {
    assert!(accept_prompt_body(b"").is_err());
    assert!(accept_prompt_body(b"   \n").is_err());
    assert!(accept_prompt_body(&vec![b'a'; MAX_PROMPT_BYTES + 1]).is_err());
    assert_eq!(accept_prompt_body(b"hello\n").unwrap(), "hello\n");
}

#[test]
fn same_sha_reuses_the_existing_prompt() {
    let (_tmp, store, _opts) = setup(ToolKind::Claude);
    let first = store_harness_prompt(&store, ToolKind::Claude, "same body\n").unwrap();
    let second = store_harness_prompt(&store, ToolKind::Claude, "same body\n").unwrap();
    assert_eq!(first, second);
    let prompts = store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
        .unwrap();
    assert_eq!(prompts.len(), 1);

    let other = store_harness_prompt(&store, ToolKind::Claude, "other body\n").unwrap();
    assert_ne!(first, other);
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn deploy_then_remove_uses_user_scope_and_keeps_the_library() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let outcome = deploy_harness_with(
        &store,
        ToolKind::Claude,
        &opts,
        Some("managed harness body\n".into()),
    )
    .await
    .unwrap();
    assert!(outcome.ok, "{outcome:?}");
    assert_eq!(outcome.action, HarnessAction::Deploy);
    let prompt_id = outcome.prompt_id.expect("stored prompt");
    let deployed = harness_state(&store, ToolKind::Claude, &opts)
        .await
        .unwrap();
    assert!(
        deployed.deployed,
        "deploy must be visible on the next read: {deployed:?}"
    );
    assert!(deployed.error.is_none());

    let prompts = store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
        .unwrap();
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].id, prompt_id);

    let removed = remove_harness(&store, ToolKind::Claude, &opts)
        .await
        .unwrap();
    assert!(removed.ok, "{removed:?}");
    assert_eq!(removed.action, HarnessAction::Remove);
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
            .unwrap()
            .len(),
        1,
        "remove must not wipe the library"
    );
    let _ = Scope::User;
    let cleared = harness_state(&store, ToolKind::Claude, &opts)
        .await
        .unwrap();
    assert!(
        !cleared.deployed,
        "remove must leave the machine undeployed: {cleared:?}"
    );
    assert!(cleared.error.is_none());
}

#[tokio::test]
async fn deploy_does_not_execute_when_preview_is_blocked() {
    let (_tmp, store, mut opts) = setup(ToolKind::Claude);
    opts.extra_env.insert("FIXTURE_FAIL".into(), "1".into());
    let before = store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
        .unwrap()
        .len();
    let outcome = deploy_harness_with(&store, ToolKind::Claude, &opts, Some("blocked\n".into()))
        .await
        .unwrap();
    assert!(!outcome.ok, "{outcome:?}");
    assert!(outcome.error.unwrap().len() > 0);
    let prompts = store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
        .unwrap();
    assert_eq!(
        prompts.len(),
        before + 1,
        "the fetched body is stored before preview"
    );
    let activations = store.list_activations(ToolKind::Claude).unwrap();
    assert!(
        activations
            .iter()
            .all(|item| item.status != ToolStatus::Active),
        "a blocked preview must not activate"
    );
}

#[tokio::test]
async fn zcode_on_windows_fails_without_calling_the_sidecar() {
    let (_tmp, store, opts) = setup(ToolKind::Zcode);
    if !cfg!(windows) {
        let outcome =
            deploy_harness_with(&store, ToolKind::Zcode, &opts, Some("system role\n".into()))
                .await
                .unwrap();
        assert!(
            outcome.ok,
            "non-windows zcode deploy through the fixture should succeed: {outcome:?}"
        );
        return;
    }
    let outcome = deploy_harness_with(&store, ToolKind::Zcode, &opts, Some("system role\n".into()))
        .await
        .unwrap();
    assert!(!outcome.ok);
    assert!(outcome.error.unwrap().contains("not available on Windows"));
    assert!(store
        .list_prompts(ToolKind::Zcode, None, None, PromptSort::Updated)
        .unwrap()
        .is_empty());
}
