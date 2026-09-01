use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::diff::{change_summary, unified_diff};
use keysmith_switch_lib::models::{CreatePromptInput, ToolKind, UpdatePromptInput};
use keysmith_switch_lib::ops::{
    copy_prompt, create_prompt, prompt_diff, prompt_history, update_prompt,
};
use keysmith_switch_lib::paths::AppPaths;

fn open_store() -> (tempfile::TempDir, Store) {
    let (tmp, paths) = AppPaths::in_temp_dir().unwrap();
    let store = Store::open(&paths).unwrap();
    (tmp, store)
}

#[test]
fn markdown_import_via_create_prompt() {
    let (_tmp, store) = open_store();
    let created = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Claude,
            title: "Imported".into(),
            content: "line one\n".into(),
            tags: vec!["alpha".into()],
        },
    )
    .unwrap();
    assert!(created.path.is_file());
    let listed = store
        .list_prompts(
            ToolKind::Claude,
            Some("import"),
            Some("alpha"),
            Default::default(),
        )
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[test]
fn cross_tool_copy_keeps_history_separate() {
    let (_tmp, store) = open_store();
    let source = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Claude,
            title: "Shared".into(),
            content: "shared body\n".into(),
            tags: vec!["x".into()],
        },
    )
    .unwrap();
    let copied = copy_prompt(&store, &source.id, ToolKind::Grok).unwrap();
    assert_ne!(copied.id, source.id);
    assert_eq!(copied.tool, ToolKind::Grok);
    assert_eq!(copied.content, source.content);
    assert_eq!(
        store
            .list_prompts(ToolKind::Grok, None, None, Default::default())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store
            .list_prompts(ToolKind::Claude, None, None, Default::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn history_unified_diff_and_summary() {
    let (_tmp, store) = open_store();
    let created = create_prompt(
        &store,
        CreatePromptInput {
            tool: ToolKind::Codex,
            title: "Diff Me".into(),
            content: "alpha\nbeta\n".into(),
            tags: vec![],
        },
    )
    .unwrap();
    update_prompt(
        &store,
        UpdatePromptInput {
            id: created.id.clone(),
            title: None,
            content: Some("alpha\ngamma\n".into()),
            tags: None,
        },
    )
    .unwrap();
    let versions = prompt_history(&store, &created.id).unwrap();
    assert_eq!(versions.len(), 2);
    let diff = prompt_diff(&store, &created.id, 1, 2).unwrap();
    assert!(diff.unified.contains("-beta"));
    assert!(diff.unified.contains("+gamma"));
    assert_eq!(diff.summary.added_lines, 1);
    assert_eq!(diff.summary.removed_lines, 1);
    assert!(diff.summary.hunks >= 1);
    let summary = change_summary("alpha\nbeta\n", "alpha\ngamma\n");
    assert_eq!(summary.added_lines, 1);
    let unified = unified_diff("a\n", "b\n", "old", "new");
    assert!(unified.contains("--- old"));
    assert!(unified.contains("+++ new"));
}
