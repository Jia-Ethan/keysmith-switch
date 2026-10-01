//! Cleanup and rollback against the real vendored adapters, in a temporary home.
//! Nothing here touches the machine the tests run on.

use std::path::PathBuf;

use keysmith_switch_lib::adapter::AdapterOptions;
use keysmith_switch_lib::cleanup::{
    confirm_cleanup, confirm_rollback, delete_snapshot, list_snapshots, plan_cleanup, plan_rollback,
};
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::harness::{deploy_harness_with, harness_state};
use keysmith_switch_lib::models::{Scope, ToolKind};
use keysmith_switch_lib::ops::tool_status;
use keysmith_switch_lib::paths::AppPaths;

fn python3_available() -> bool {
    std::process::Command::new("python3")
        .arg("-c")
        .arg("import sys")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn vendor(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/keysmith")
        .join(rel)
}

struct World {
    _tmp: tempfile::TempDir,
    store: Store,
    opts: AdapterOptions,
    home: PathBuf,
}

fn world(script: &str) -> World {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();
    let opts = AdapterOptions {
        home: Some(home.clone()),
        cli_override: Some(vendor(script)),
        ..AdapterOptions::default()
    };
    World {
        _tmp: tmp,
        store,
        opts,
        home,
    }
}

fn memory_path(world: &World) -> PathBuf {
    world.home.join(".claude").join("CLAUDE.md")
}

/// Claude Code with a deployed prompt, and notes of the person's own in CLAUDE.md.
async fn claude_with_notes() -> (World, Vec<u8>) {
    let world = world("claude/claude-instruct.py");
    let outcome = deploy_harness_with(
        &world.store,
        ToolKind::Claude,
        &world.opts,
        Some("# Deployed rules\nBe careful.\n".into()),
    )
    .await
    .unwrap();
    assert!(outcome.ok, "{outcome:?}");
    let path = memory_path(&world);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n# My own notes\nAlways answer in Chinese.\nNever touch the billing code.\n");
    std::fs::write(&path, &text).unwrap();
    (world, text.into_bytes())
}

async fn deployed(world: &World, tool: ToolKind) -> bool {
    harness_state(&world.store, tool, &world.opts)
        .await
        .unwrap()
        .deployed
}

#[tokio::test]
async fn cleaning_claude_removes_the_deployment_and_empties_memory_after_saving_both() {
    if !python3_available() {
        return;
    }
    let (world, original) = claude_with_notes().await;
    let path = memory_path(&world);
    assert!(deployed(&world, ToolKind::Claude).await);

    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    assert!(
        plan.deployment.present && plan.deployment.restorable,
        "{plan:?}"
    );
    let memory = plan.memory.clone().expect("claude has a memory file");
    assert_eq!(memory.bytes, original.len() as u64);
    assert!(!plan.nothing_to_do && plan.blockers.is_empty());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "a preview changes nothing"
    );
    assert!(
        list_snapshots(&world.store).is_empty(),
        "a preview saves nothing"
    );

    let done = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap();
    assert!(done.deactivated && done.memory_cleared);
    let snapshot_id = done.snapshot_id.expect("a snapshot is taken first");

    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"",
        "the memory file is emptied, not deleted"
    );
    assert!(!deployed(&world, ToolKind::Claude).await);
    let status = tool_status(
        &world.store,
        ToolKind::Claude,
        Scope::User,
        None,
        &world.opts,
    )
    .await
    .unwrap();
    assert_ne!(
        status.status,
        keysmith_switch_lib::models::ToolStatus::Active
    );

    let snapshots = list_snapshots(&world.store);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, snapshot_id);
    assert_eq!(snapshots[0].kind, "cleanup");
    assert_eq!(snapshots[0].memory.as_ref().unwrap().sha256, memory.sha256);
    let saved = std::fs::read(
        world
            .store
            .paths()
            .home
            .join("snapshots")
            .join(&snapshot_id)
            .join("memory.bin"),
    )
    .unwrap();
    assert_eq!(saved, original, "the copy is complete");
}

#[tokio::test]
async fn rolling_back_restores_the_memory_file_byte_for_byte_and_the_deployment() {
    if !python3_available() {
        return;
    }
    let (world, original) = claude_with_notes().await;
    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    let snapshot_id = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap()
        .snapshot_id
        .unwrap();

    let back = plan_rollback(&world.store, &snapshot_id, &world.opts)
        .await
        .unwrap();
    assert!(back.blockers.is_empty(), "{back:?}");
    assert!(
        !back.saves_current,
        "nothing is there to lose after a cleanup"
    );
    assert_eq!(
        std::fs::read(memory_path(&world)).unwrap(),
        b"",
        "a preview changes nothing"
    );

    let done = confirm_rollback(&world.store, &back.operation_id, &world.opts)
        .await
        .unwrap();
    assert!(done.redeployed && done.memory_restored);
    assert_eq!(done.saved_snapshot_id, None);
    assert_eq!(std::fs::read(memory_path(&world)).unwrap(), original);
    assert!(
        deployed(&world, ToolKind::Claude).await,
        "the prompt is live again"
    );
    let status = tool_status(
        &world.store,
        ToolKind::Claude,
        Scope::User,
        None,
        &world.opts,
    )
    .await
    .unwrap();
    assert_eq!(
        status.status,
        keysmith_switch_lib::models::ToolStatus::Active,
        "{status:?}"
    );
}

#[tokio::test]
async fn a_rollback_saves_what_it_overwrites_so_it_can_be_undone() {
    if !python3_available() {
        return;
    }
    let (world, original) = claude_with_notes().await;
    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    let first = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap()
        .snapshot_id
        .unwrap();

    // After the cleanup the person writes new notes.
    let newer = b"# Written after the cleanup\nuse tabs\n".to_vec();
    std::fs::write(memory_path(&world), &newer).unwrap();

    let back = plan_rollback(&world.store, &first, &world.opts)
        .await
        .unwrap();
    assert!(back.saves_current);
    assert!(back.memory.as_ref().unwrap().current_differs);
    let done = confirm_rollback(&world.store, &back.operation_id, &world.opts)
        .await
        .unwrap();
    let safety = done.saved_snapshot_id.expect("the new notes are kept");
    assert_eq!(std::fs::read(memory_path(&world)).unwrap(), original);
    assert_eq!(
        list_snapshots(&world.store)
            .iter()
            .find(|s| s.id == safety)
            .unwrap()
            .kind,
        "before-rollback"
    );

    // And that rollback can be undone: the newer notes come back.
    let undo = plan_rollback(&world.store, &safety, &world.opts)
        .await
        .unwrap();
    confirm_rollback(&world.store, &undo.operation_id, &world.opts)
        .await
        .unwrap();
    assert_eq!(std::fs::read(memory_path(&world)).unwrap(), newer);
}

#[tokio::test]
async fn only_the_user_level_memory_file_is_ever_touched() {
    if !python3_available() {
        return;
    }
    let (world, _original) = claude_with_notes().await;
    // A project, with its own CLAUDE.md and files, next to the home.
    let project = world.home.parent().unwrap().join("project");
    std::fs::create_dir_all(project.join(".claude")).unwrap();
    std::fs::write(project.join("CLAUDE.md"), "project memory\n").unwrap();
    std::fs::write(project.join(".claude/CLAUDE.local.md"), "local memory\n").unwrap();
    std::fs::write(project.join("main.rs"), "fn main() {}\n").unwrap();
    let before: Vec<Vec<u8>> = ["CLAUDE.md", ".claude/CLAUDE.local.md", "main.rs"]
        .iter()
        .map(|name| std::fs::read(project.join(name)).unwrap())
        .collect();

    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap();

    let after: Vec<Vec<u8>> = ["CLAUDE.md", ".claude/CLAUDE.local.md", "main.rs"]
        .iter()
        .map(|name| std::fs::read(project.join(name)).unwrap())
        .collect();
    assert_eq!(before, after, "project files are not touched");
}

#[cfg(unix)]
#[tokio::test]
async fn a_memory_file_that_is_a_link_elsewhere_is_never_emptied() {
    if !python3_available() {
        return;
    }
    let (world, _) = claude_with_notes().await;
    let path = memory_path(&world);
    let elsewhere = world.home.parent().unwrap().join("somewhere-else.md");
    std::fs::write(&elsewhere, "precious\n").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &path).unwrap();

    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts).await;
    assert!(plan.is_err(), "a link is refused: {plan:?}");
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "precious\n");
}

#[tokio::test]
async fn a_plan_is_used_once_and_only_while_nothing_has_changed() {
    if !python3_available() {
        return;
    }
    let (world, _) = claude_with_notes().await;
    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();

    // The person edits the file after the preview: the preview no longer describes what is there.
    let mut edited = std::fs::read_to_string(memory_path(&world)).unwrap();
    edited.push_str("\nchanged after the preview\n");
    std::fs::write(memory_path(&world), &edited).unwrap();
    let refused = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("plan already used"),
        "{refused}"
    );
    assert_eq!(
        std::fs::read_to_string(memory_path(&world)).unwrap(),
        edited
    );
    assert!(list_snapshots(&world.store).is_empty());
    assert!(
        deployed(&world, ToolKind::Claude).await,
        "nothing was removed"
    );

    // And a plan that was confirmed is spent.
    let fresh = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    confirm_cleanup(&world.store, &fresh.operation_id, &world.opts)
        .await
        .unwrap();
    let again = confirm_cleanup(&world.store, &fresh.operation_id, &world.opts)
        .await
        .unwrap_err();
    assert!(again.to_string().contains("plan already used"), "{again}");
}

#[tokio::test]
async fn cleaning_an_agent_with_nothing_to_clean_says_so_and_saves_nothing() {
    if !python3_available() {
        return;
    }
    let world = world("grok/grok-keysmith.py");
    let plan = plan_cleanup(&world.store, ToolKind::Grok, &world.opts)
        .await
        .unwrap();
    assert!(plan.nothing_to_do && plan.memory.is_none() && !plan.deployment.present);
    let done = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap();
    assert!(!done.deactivated && !done.memory_cleared && done.snapshot_id.is_none());
    assert!(list_snapshots(&world.store).is_empty());
}

#[tokio::test]
async fn cleaning_grok_removes_the_deployment_and_rolling_back_restores_it() {
    if !python3_available() {
        return;
    }
    let world = world("grok/grok-keysmith.py");
    let outcome = deploy_harness_with(
        &world.store,
        ToolKind::Grok,
        &world.opts,
        Some("# Grok rules\nAsk first.\n".into()),
    )
    .await
    .unwrap();
    assert!(outcome.ok, "{outcome:?}");

    let plan = plan_cleanup(&world.store, ToolKind::Grok, &world.opts)
        .await
        .unwrap();
    assert!(
        plan.deployment.present && plan.memory.is_none(),
        "grok has no memory file Keysmith touches"
    );
    let done = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap();
    assert!(done.deactivated && !done.memory_cleared);
    assert!(!deployed(&world, ToolKind::Grok).await);

    let back = plan_rollback(&world.store, &done.snapshot_id.unwrap(), &world.opts)
        .await
        .unwrap();
    let restored = confirm_rollback(&world.store, &back.operation_id, &world.opts)
        .await
        .unwrap();
    assert!(restored.redeployed && !restored.memory_restored);
    assert!(deployed(&world, ToolKind::Grok).await);
}

#[tokio::test]
async fn snapshot_ids_cannot_name_other_paths_and_snapshots_can_be_deleted() {
    if !python3_available() {
        return;
    }
    let (world, _) = claude_with_notes().await;
    for bad in ["../outside", "a/b", "", "..", "x y"] {
        assert!(
            plan_rollback(&world.store, bad, &world.opts).await.is_err(),
            "{bad:?}"
        );
        assert!(delete_snapshot(&world.store, bad).is_err(), "{bad:?}");
    }
    let outside = world.store.paths().home.join("keep-me");
    std::fs::create_dir_all(&outside).unwrap();
    assert!(delete_snapshot(&world.store, "../keep-me").is_err());
    assert!(outside.is_dir());

    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    let id = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap()
        .snapshot_id
        .unwrap();
    assert_eq!(list_snapshots(&world.store).len(), 1);
    delete_snapshot(&world.store, &id).unwrap();
    assert!(list_snapshots(&world.store).is_empty());
}

#[tokio::test]
async fn an_unfinished_snapshot_is_not_listed_and_a_damaged_one_is_refused() {
    if !python3_available() {
        return;
    }
    let (world, _) = claude_with_notes().await;
    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    let id = confirm_cleanup(&world.store, &plan.operation_id, &world.opts)
        .await
        .unwrap()
        .snapshot_id
        .unwrap();
    let root = world.store.paths().home.join("snapshots");
    std::fs::create_dir_all(root.join(".half-written.writing")).unwrap();
    std::fs::create_dir_all(root.join("20260101T000000Z-claude-abc123")).unwrap();
    assert_eq!(
        list_snapshots(&world.store).len(),
        1,
        "only complete snapshots are listed"
    );

    // The copy of the memory file is damaged: a rollback must refuse rather than write it back.
    let memory = root.join(&id).join("memory.bin");
    let mut bytes = std::fs::read(&memory).unwrap();
    bytes[0] ^= 0xff;
    std::fs::write(&memory, bytes).unwrap();
    let refused = plan_rollback(&world.store, &id, &world.opts).await;
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(std::fs::read(memory_path(&world)).unwrap(), b"");
}
