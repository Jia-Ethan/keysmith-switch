use std::path::PathBuf;

use keysmith_switch_lib::adapter::AdapterOptions;
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::harness::{
    accept_prompt_body, deploy_harness_with, harness_source, harness_state, remove_harness,
    store_harness_prompt, store_pasted_prompt, HarnessAction, MAX_PROMPT_BYTES,
};
use keysmith_switch_lib::models::{
    CreatePromptInput, PlanActivateInput, PromptSort, Scope, ToolKind, ToolStatus,
    UpdatePromptInput,
};
use keysmith_switch_lib::ops::{confirm_activate, create_prompt, plan_activate, update_prompt};
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
fn catalog_is_local_compatibility_metadata() {
    let cases = [
        (ToolKind::Claude, "Claude prompt"),
        (ToolKind::Codex, "Codex prompt"),
        (ToolKind::Grok, "Grok prompt"),
        (ToolKind::Zcode, "ZCode prompt"),
    ];
    for (tool, title) in cases {
        let source = harness_source(tool);
        assert_eq!(source.tool, tool);
        assert_eq!(source.title(), title);
    }
}

#[tokio::test]
async fn default_deploy_requires_user_content() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let outcome = deploy_harness_with(&store, ToolKind::Claude, &opts, None)
        .await
        .unwrap();
    assert!(!outcome.ok, "{outcome:?}");
    assert!(outcome.error.unwrap().contains("content is required"));
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

#[test]
fn pasted_prompt_uses_a_generic_source_tag_and_title() {
    let (_tmp, store, _opts) = setup(ToolKind::Claude);
    let id = keysmith_switch_lib::harness::store_pasted_prompt(
        &store,
        ToolKind::Claude,
        "My rules",
        "be concise\n",
    )
    .unwrap();
    let prompt = store.get_prompt(&id).unwrap();
    assert_eq!(prompt.title, "My rules");
    assert!(prompt.tags.iter().any(|tag| tag == "pasted"));
    assert!(!prompt.tags.iter().any(|tag| tag == "harness"));
}

#[test]
fn same_content_reuses_existing_metadata_without_retagging() {
    let (_tmp, store, _opts) = setup(ToolKind::Claude);
    let original = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Claude,
            title: "Existing import".into(),
            content: "shared body".into(),
            tags: vec!["imported".into()],
        },
    )
    .unwrap();
    let reused =
        store_pasted_prompt(&store, ToolKind::Claude, "New pasted title", "shared body").unwrap();
    assert_eq!(reused, original.id);
    let detail = store.get_prompt(&reused).unwrap();
    assert_eq!(detail.title, "Existing import");
    assert_eq!(detail.tags, vec!["imported"]);
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn concurrent_identical_pastes_share_one_entry() {
    let (_tmp, store, _opts) = setup(ToolKind::Claude);
    std::thread::scope(|threads| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                threads
                    .spawn(|| store_pasted_prompt(&store, ToolKind::Claude, "Rules", "exact body"))
            })
            .collect();
        let ids: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap().unwrap())
            .collect();
        assert!(ids.iter().all(|id| id == &ids[0]));
    });
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn pasted_activation_deploys_only_body_and_cleans_temporary_input() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let body = "\nOnly these rules, with no library metadata.";
    let prompt_id = store_pasted_prompt(&store, ToolKind::Claude, "My title", body).unwrap();
    let plan = plan_activate(
        &store,
        PlanActivateInput {
            prompt_id: prompt_id.clone(),
            scope: Scope::User,
            project_dir: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
        },
        &opts,
    )
    .await
    .unwrap();
    assert!(plan.envelope.ok, "{plan:?}");
    let adapter_file = store
        .get_prompt(&prompt_id)
        .unwrap()
        .path
        .with_extension("adapter.txt");
    assert!(!adapter_file.exists(), "preview input must not persist");
    let executed = confirm_activate(&store, &plan.operation_id, &opts)
        .await
        .unwrap();
    assert!(executed.envelope.ok, "{executed:?}");
    let installed = opts
        .home
        .unwrap()
        .join(".claude/keysmith/claude-project-rules.md");
    assert_eq!(std::fs::read_to_string(installed).unwrap(), body);
    assert!(!adapter_file.exists(), "execution input must not persist");
    assert_eq!(store.get_prompt(&prompt_id).unwrap().content, body);
}

#[tokio::test]
async fn preview_rejects_changed_prompt_even_if_body_sha_is_unchanged() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let id = store_pasted_prompt(&store, ToolKind::Claude, "Old title", "unchanged body").unwrap();
    let plan = plan_activate(
        &store,
        PlanActivateInput {
            prompt_id: id.clone(),
            scope: Scope::User,
            project_dir: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
        },
        &opts,
    )
    .await
    .unwrap();
    update_prompt(
        &store,
        UpdatePromptInput {
            id,
            title: Some("New title".into()),
            content: None,
            tags: None,
        },
    )
    .unwrap();
    let result = confirm_activate(&store, &plan.operation_id, &opts).await;
    assert!(
        result.is_err(),
        "a title-only version change must invalidate preview"
    );
    assert!(!opts
        .home
        .unwrap()
        .join(".claude/keysmith/claude-project-rules.md")
        .exists());
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
    assert_eq!(
        deployed.prompt_id.as_deref(),
        Some(prompt_id.as_str()),
        "the live prompt must be named so the UI can show it: {deployed:?}"
    );
    assert_eq!(deployed.prompt_title.as_deref(), Some("Claude prompt"));

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
    assert!(cleared.prompt_id.is_none() && cleared.prompt_title.is_none());
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

#[tokio::test]
async fn adopting_refuses_text_that_does_not_match_the_live_fingerprint() {
    // The fixture adapters report canned fingerprints, so no file hashes to them:
    // nothing may be adopted on a guess.
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let outcome = deploy_harness_with(&store, ToolKind::Claude, &opts, Some("live body\n".into()))
        .await
        .unwrap();
    assert!(outcome.ok, "{outcome:?}");
    keysmith_switch_lib::ops::delete_prompt(&store, &outcome.prompt_id.unwrap()).unwrap();

    let before = harness_state(&store, ToolKind::Claude, &opts)
        .await
        .unwrap();
    assert!(before.deployed && before.prompt_id.is_none(), "{before:?}");

    let result =
        keysmith_switch_lib::harness::adopt_live_prompt(&store, ToolKind::Claude, "x", &opts).await;
    assert!(result.is_err());
    assert!(store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn adopting_without_a_deployment_is_refused() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let result =
        keysmith_switch_lib::harness::adopt_live_prompt(&store, ToolKind::Claude, "x", &opts).await;
    assert!(result.is_err());
    assert!(store
        .list_prompts(ToolKind::Claude, None, None, PromptSort::Updated)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn codex_live_prompt_is_read_from_the_file_its_config_loads() {
    let (_tmp, store, opts) = setup(ToolKind::Codex);
    let body = "You are careful.\nAsk before deleting.\n";
    let outcome = deploy_harness_with(&store, ToolKind::Codex, &opts, Some(body.into()))
        .await
        .unwrap();
    assert!(outcome.ok, "{outcome:?}");
    keysmith_switch_lib::ops::delete_prompt(&store, &outcome.prompt_id.unwrap()).unwrap();

    let before = harness_state(&store, ToolKind::Codex, &opts).await.unwrap();
    assert!(before.deployed && before.prompt_id.is_none(), "{before:?}");

    let id =
        keysmith_switch_lib::harness::adopt_live_prompt(&store, ToolKind::Codex, "Adopted", &opts)
            .await
            .unwrap();
    assert_eq!(store.get_prompt(&id).unwrap().content, body);

    // The library now names what Codex runs, also on a later read of the machine.
    let after = harness_state(&store, ToolKind::Codex, &opts).await.unwrap();
    assert_eq!(after.prompt_id.as_deref(), Some(id.as_str()));
}

#[cfg(unix)]
#[tokio::test]
async fn codex_prompt_outside_the_codex_directory_is_never_read() {
    let (tmp, store, opts) = setup(ToolKind::Codex);
    let outcome = deploy_harness_with(&store, ToolKind::Codex, &opts, Some("managed\n".into()))
        .await
        .unwrap();
    assert!(outcome.ok, "{outcome:?}");
    keysmith_switch_lib::ops::delete_prompt(&store, &outcome.prompt_id.unwrap()).unwrap();

    // The config now points at a file elsewhere on the machine.
    let elsewhere = tmp.path().join("private-notes.md");
    std::fs::write(&elsewhere, "not a prompt\n").unwrap();
    let managed = opts
        .home
        .clone()
        .unwrap()
        .join(".codex/gpt-unrestricted.md");
    std::fs::remove_file(&managed).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &managed).unwrap();

    let result =
        keysmith_switch_lib::harness::adopt_live_prompt(&store, ToolKind::Codex, "x", &opts).await;
    assert!(result.is_err());
    assert!(store
        .list_prompts(ToolKind::Codex, None, None, PromptSort::Updated)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_spent_plan_says_so_instead_of_claiming_nothing_was_planned() {
    let (_tmp, store, opts) = setup(ToolKind::Claude);
    let id = store_pasted_prompt(&store, ToolKind::Claude, "Once", "only once\n").unwrap();
    let plan = plan_activate(
        &store,
        PlanActivateInput {
            prompt_id: id,
            scope: Scope::User,
            project_dir: None,
            runtime: false,
            append_file: None,
            max_tokens: None,
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
    assert!(again.to_string().contains("plan already used"), "{again}");
    let unknown = confirm_activate(&store, "no-such-plan", &opts)
        .await
        .unwrap_err();
    assert!(unknown.to_string().contains("plan not found"), "{unknown}");
}
