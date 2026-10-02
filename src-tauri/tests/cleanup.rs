//! Cleanup and rollback against the real vendored adapters, in a temporary home.
//! Nothing here touches the machine the tests run on.

use std::path::PathBuf;

use keysmith_switch_lib::adapter::AdapterOptions;
use keysmith_switch_lib::cleanup::{
    confirm_cleanup, confirm_rollback, delete_snapshot, list_snapshots, plan_cleanup,
    plan_rollback, CODEX_RUNNING_KEY,
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

    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    let snapshot_id = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    let first = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    let refused = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    confirm_cleanup(&world.store, &fresh.operation_id, false, &world.opts)
        .await
        .unwrap();
    let again = confirm_cleanup(&world.store, &fresh.operation_id, false, &world.opts)
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
    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    let id = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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
    let id = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
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

// ---- Codex: AGENTS.md and the memories folder -------------------------------------------

fn codex_world(running: bool) -> World {
    let mut world = world("codex/codex-instruct.py");
    world.opts.extra_env.insert(
        CODEX_RUNNING_KEY.into(),
        if running { "1" } else { "0" }.into(),
    );
    world
}

/// Codex with a deployed prompt, notes in AGENTS.md, and a memories folder next to a database.
async fn codex_with_memories(running: bool) -> World {
    let world = codex_world(running);
    let root = world.home.join(".codex");
    std::fs::create_dir_all(root.join("memories").join("notes")).unwrap();
    std::fs::write(root.join("config.toml"), "").unwrap();
    let outcome = deploy_harness_with(
        &world.store,
        ToolKind::Codex,
        &world.opts,
        Some("# Codex rules\nBe careful.\n".into()),
    )
    .await
    .unwrap();
    assert!(outcome.ok, "{outcome:?}");
    std::fs::write(root.join("AGENTS.md"), "# Mine\nAnswer in Chinese.\n").unwrap();
    std::fs::write(root.join("memories").join("MEMORY.md"), "remember this\n").unwrap();
    std::fs::write(root.join("memories").join("notes").join("a.md"), "more\n").unwrap();
    std::fs::write(root.join("memories_1.sqlite"), b"database").unwrap();
    world
}

fn memories_text(world: &World) -> (String, String) {
    let dir = world.home.join(".codex").join("memories");
    (
        std::fs::read_to_string(dir.join("MEMORY.md")).unwrap(),
        std::fs::read_to_string(dir.join("notes").join("a.md")).unwrap(),
    )
}

#[tokio::test]
async fn cleaning_codex_empties_agents_md_but_leaves_the_memories_folder_unless_asked() {
    if !python3_available() {
        return;
    }
    let world = codex_with_memories(false).await;
    let root = world.home.join(".codex");

    let plan = plan_cleanup(&world.store, ToolKind::Codex, &world.opts)
        .await
        .unwrap();
    assert!(plan.deployment.present, "{plan:?}");
    assert_eq!(
        plan.memory.as_ref().map(|m| m.path.ends_with("AGENTS.md")),
        Some(true)
    );
    let memories = plan.memories.clone().expect("the folder has files in it");
    assert_eq!(memories.files, 2);
    assert!(!plan.agent_running);

    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
        .await
        .unwrap();
    assert!(done.deactivated && done.memory_cleared && !done.memories_cleared);
    assert_eq!(std::fs::read(root.join("AGENTS.md")).unwrap(), b"");
    assert_eq!(
        memories_text(&world),
        ("remember this\n".into(), "more\n".into()),
        "without the tick the folder is not touched"
    );
    let snapshots = list_snapshots(&world.store);
    assert!(snapshots[0].memories.is_none());
}

#[tokio::test]
async fn clearing_the_codex_memories_folder_moves_it_into_the_snapshot_and_rollback_brings_it_back()
{
    if !python3_available() {
        return;
    }
    let world = codex_with_memories(false).await;
    let root = world.home.join(".codex");
    let agents = std::fs::read(root.join("AGENTS.md")).unwrap();

    let plan = plan_cleanup(&world.store, ToolKind::Codex, &world.opts)
        .await
        .unwrap();
    let done = confirm_cleanup(&world.store, &plan.operation_id, true, &world.opts)
        .await
        .unwrap();
    assert!(done.memories_cleared && done.memory_cleared);
    let id = done.snapshot_id.unwrap();

    let folder = root.join("memories");
    assert!(folder.is_dir(), "the folder is put back, empty");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
    assert!(
        !root.join("memories_1.sqlite").exists(),
        "Codex's own database is session history: it is erased, not saved"
    );
    let saved = list_snapshots(&world.store)
        .into_iter()
        .find(|snapshot| snapshot.id == id)
        .unwrap();
    assert_eq!(saved.memories.as_ref().map(|m| m.files), Some(2));

    // The person starts writing again before they change their mind.
    std::fs::write(folder.join("MEMORY.md"), "new beginnings\n").unwrap();
    let back = plan_rollback(&world.store, &id, &world.opts).await.unwrap();
    let counts = back.memories.clone().unwrap();
    assert_eq!((counts.restore_files, counts.current_files), (2, 1));
    assert!(back.saves_current);
    let restored = confirm_rollback(&world.store, &back.operation_id, &world.opts)
        .await
        .unwrap();
    assert!(restored.memories_restored && restored.memory_restored && restored.redeployed);
    assert_eq!(
        memories_text(&world),
        ("remember this\n".into(), "more\n".into())
    );
    assert_eq!(std::fs::read(root.join("AGENTS.md")).unwrap(), agents);

    // What was overwritten is in a snapshot of its own, and the first snapshot is still whole.
    let all = list_snapshots(&world.store);
    let undo = all
        .iter()
        .find(|snapshot| Some(&snapshot.id) == restored.saved_snapshot_id.as_ref())
        .expect("the rollback saved what it replaced");
    assert_eq!(undo.memories.as_ref().map(|m| m.files), Some(1));
    assert!(all
        .iter()
        .any(|snapshot| snapshot.id == id && snapshot.memories.is_some()));
}

#[tokio::test]
async fn the_codex_memories_folder_is_not_touched_while_codex_runs_or_when_it_changed() {
    if !python3_available() {
        return;
    }
    let world = codex_with_memories(true).await;
    let plan = plan_cleanup(&world.store, ToolKind::Codex, &world.opts)
        .await
        .unwrap();
    assert!(plan.agent_running);
    let refused = confirm_cleanup(&world.store, &plan.operation_id, true, &world.opts).await;
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(
        memories_text(&world),
        ("remember this\n".into(), "more\n".into())
    );
    assert!(
        deployed(&world, ToolKind::Codex).await,
        "nothing was removed either"
    );
    assert!(list_snapshots(&world.store).is_empty());

    // Closed now, but a file arrives between the preview and the tick.
    let mut world = world;
    world
        .opts
        .extra_env
        .insert(CODEX_RUNNING_KEY.into(), "0".into());
    let plan = plan_cleanup(&world.store, ToolKind::Codex, &world.opts)
        .await
        .unwrap();
    std::fs::write(world.home.join(".codex/memories/late.md"), "late").unwrap();
    let stale = confirm_cleanup(&world.store, &plan.operation_id, true, &world.opts).await;
    assert!(stale.is_err(), "{stale:?}");
    assert!(world.home.join(".codex/memories/late.md").exists());
    assert!(list_snapshots(&world.store).is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn a_codex_memories_folder_that_is_a_link_is_refused() {
    if !python3_available() {
        return;
    }
    let world = codex_with_memories(false).await;
    let elsewhere = world.home.join("elsewhere");
    std::fs::rename(world.home.join(".codex/memories"), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, world.home.join(".codex/memories")).unwrap();
    let plan = plan_cleanup(&world.store, ToolKind::Codex, &world.opts).await;
    assert!(plan.is_err(), "{plan:?}");
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("MEMORY.md")).unwrap(),
        "remember this\n"
    );
}

// ---- Full cleanup: back to a fresh install, logins and history included --------------------

const SECRET: &str = "sk-ant-LOGINSECRET123456";

fn write(path: PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A Claude home with settings, a login, history, plugins and a git project beside it.
fn claude_fresh_install_world() -> (World, PathBuf) {
    let world = world("claude/claude-instruct.py");
    let home = &world.home;
    write(
        home.join(".claude/settings.json"),
        &format!(
            r#"{{"model":"opus","hooks":{{"Stop":[]}},"env":{{"ANTHROPIC_AUTH_TOKEN":"{SECRET}","API_TIMEOUT_MS":"5"}}}}"#
        ),
    );
    write(
        home.join(".claude.json"),
        &format!(
            r#"{{"hasCompletedOnboarding":true,"oauthAccount":{{"emailAddress":"a@b.c"}},"mcpServers":{{"x":{{"command":"npx"}}}},"apiKey":"{SECRET}"}}"#
        ),
    );
    write(home.join(".claude/rules/a.md"), "rule\n");
    write(home.join(".claude/skills/s/SKILL.md"), "skill\n");
    write(home.join(".claude/history.jsonl"), "{\"display\":\"hi\"}\n");
    write(home.join(".claude/projects/p/s.jsonl"), "conversation\n");
    write(home.join(".claude/plugins/x/plugin.json"), "{}");
    write(
        home.join(".claude/settings.json.bak_1"),
        &format!("{{\"k\":\"{SECRET}\"}}"),
    );
    let project = world.home.parent().unwrap().join("project");
    write(project.join("CLAUDE.md"), "project notes\n");
    write(project.join(".git/HEAD"), "ref\n");
    (world, project)
}

fn tree_text(root: &std::path::Path) -> String {
    let mut all = String::new();
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if entry.file_type().is_file() {
            all.push_str(&String::from_utf8_lossy(
                &std::fs::read(entry.path()).unwrap(),
            ));
        }
    }
    all
}

#[tokio::test]
async fn a_full_cleanup_returns_claude_to_a_fresh_install_and_keeps_no_login_in_the_snapshot() {
    if !python3_available() {
        return;
    }
    let (world, project) = claude_fresh_install_world();
    let home = world.home.clone();

    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    assert!(!plan.nothing_to_do && plan.blockers.is_empty());
    assert!(plan
        .extras
        .iter()
        .any(|e| e.name == ".claude/settings.json" && e.kind == "saved-without-login"));
    assert!(plan
        .extras
        .iter()
        .any(|e| e.name == ".claude/projects" && e.kind == "erased"));
    assert!(
        !plan.login_in_keychain,
        "a test home never reaches the real keychain"
    );

    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
        .await
        .unwrap();
    assert!(done.erased >= 4 && done.extras_cleared >= 3, "{done:?}");

    for gone in [
        ".claude/settings.json",
        ".claude.json",
        ".claude/rules",
        ".claude/skills",
        ".claude/history.jsonl",
        ".claude/projects",
        ".claude/plugins",
        ".claude/settings.json.bak_1",
    ] {
        assert!(!home.join(gone).exists(), "{gone} should be gone");
    }
    assert_eq!(
        std::fs::read_to_string(project.join("CLAUDE.md")).unwrap(),
        "project notes\n"
    );
    assert!(
        project.join(".git/HEAD").exists(),
        "a git project is never touched"
    );

    let snapshots = world.store.paths().home.join("snapshots");
    let kept = tree_text(&snapshots);
    assert!(
        !kept.contains(SECRET) && !kept.contains("a@b.c"),
        "no login in a snapshot"
    );
    assert!(
        kept.contains("opus") && kept.contains("mcpServers"),
        "settings are kept"
    );
    assert!(
        !kept.contains("conversation"),
        "history is erased, not saved"
    );

    // Rolling back brings the setup back, without a login.
    let id = done.snapshot_id.unwrap();
    let back = plan_rollback(&world.store, &id, &world.opts).await.unwrap();
    confirm_rollback(&world.store, &back.operation_id, &world.opts)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(home.join(".claude/rules/a.md")).unwrap(),
        "rule\n"
    );
    let settings = std::fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    assert!(
        settings.contains("opus") && !settings.contains(SECRET),
        "{settings}"
    );
    assert!(
        !home.join(".claude/history.jsonl").exists(),
        "history is not restored"
    );
}

#[tokio::test]
async fn a_saved_folder_holding_a_git_project_is_left_where_it_is() {
    if !python3_available() {
        return;
    }
    let (world, _project) = claude_fresh_install_world();
    write(world.home.join(".claude/agents/repo/.git/HEAD"), "ref\n");
    write(world.home.join(".claude/agents/repo/a.md"), "agent\n");
    let plan = plan_cleanup(&world.store, ToolKind::Claude, &world.opts)
        .await
        .unwrap();
    confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
        .await
        .unwrap();
    assert!(world.home.join(".claude/agents/repo/.git/HEAD").exists());
}

// ---- Grok and ZCode ----------------------------------------------------------------------

#[tokio::test]
async fn a_full_cleanup_returns_grok_to_a_fresh_install_and_keeps_its_program_and_projects() {
    if !python3_available() {
        return;
    }
    let world = world("grok/grok-keysmith.py");
    let home = world.home.clone();
    write(
        home.join(".grok/config.toml"),
        &format!(
            "[ui]\ntheme = \"dark\"\napi_key = \"{SECRET}\"\n[compat.claude]\nenabled = false\n"
        ),
    );
    write(
        home.join(".grok/auth.json"),
        &format!("{{\"https://auth.x.ai::x\":{{\"token\":\"{SECRET}\"}}}}"),
    );
    write(home.join(".grok/rules/50-mine.md"), "my rule\n");
    write(home.join(".grok/memory/MEMORY.md"), "remember\n");
    write(
        home.join(".grok/sessions/s/session.jsonl"),
        "conversation\n",
    );
    write(home.join(".grok/bin/grok"), "program\n");
    write(home.join(".grok/downloads/grok-1.0/grok"), "program\n");
    let project = world.home.parent().unwrap().join("project");
    write(project.join(".git/HEAD"), "ref\n");
    write(project.join("AGENTS.md"), "project notes\n");

    let plan = plan_cleanup(&world.store, ToolKind::Grok, &world.opts)
        .await
        .unwrap();
    assert!(!plan.nothing_to_do, "{plan:?}");
    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
        .await
        .unwrap();
    assert!(done.erased >= 2, "{done:?}");

    assert!(!home.join(".grok/auth.json").exists() && !home.join(".grok/sessions").exists());
    assert!(!home.join(".grok/config.toml").exists() && !home.join(".grok/rules").exists());
    assert!(
        home.join(".grok/bin/grok").exists() && home.join(".grok/downloads/grok-1.0/grok").exists(),
        "the program stays"
    );
    assert!(project.join(".git/HEAD").exists() && project.join("AGENTS.md").exists());

    let kept = tree_text(&world.store.paths().home.join("snapshots"));
    assert!(
        !kept.contains(SECRET) && !kept.contains("conversation"),
        "no login or history in a snapshot"
    );
    assert!(kept.contains("my rule") && kept.contains("compat.claude"));

    let id = done.snapshot_id.unwrap();
    let back = plan_rollback(&world.store, &id, &world.opts).await.unwrap();
    confirm_rollback(&world.store, &back.operation_id, &world.opts)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(home.join(".grok/rules/50-mine.md")).unwrap(),
        "my rule\n"
    );
    let config = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();
    assert!(
        config.contains("theme") && !config.contains(SECRET),
        "{config}"
    );
}

#[tokio::test]
async fn a_full_cleanup_of_zcode_leaves_its_workspace_and_projects_alone() {
    if !python3_available() {
        return;
    }
    let world = world("zcode/zcode-keysmith.py");
    let home = world.home.clone();
    write(
        home.join(".zcode/v2/config.json"),
        &format!(
            r#"{{"provider":{{"p":{{"name":"x","options":{{"apiKey":"{SECRET}","baseURL":"https://u"}}}}}}}}"#
        ),
    );
    write(
        home.join(".zcode/v2/credentials.json"),
        &format!("{{\"k\":\"{SECRET}\"}}"),
    );
    write(home.join(".zcode/commands/neat.md"), "command\n");
    write(
        home.join(".zcode/cli/rollout/model-io.jsonl"),
        "conversation\n",
    );
    write(home.join(".zcode/cli/db/db.sqlite"), "db");
    write(
        home.join(".zcode/workspace/default/repo/.git/HEAD"),
        "ref\n",
    );
    write(home.join(".zcode/workspace/default/repo/a.md"), "work\n");
    write(home.join(".zcode-keysmith/system-role.md"), "role\n");
    write(
        home.join(".zcode-keysmith/backups/zcode.cjs.1.original"),
        "runtime\n",
    );
    write(
        home.join("Library/Application Support/ZCode/session/Preferences"),
        "{}",
    );

    let plan = plan_cleanup(&world.store, ToolKind::Zcode, &world.opts)
        .await
        .unwrap();
    assert!(!plan.nothing_to_do, "{plan:?}");
    assert!(
        !plan
            .extras
            .iter()
            .any(|e| e.name.starts_with(".zcode/workspace")),
        "the workspace is never named"
    );
    let done = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts)
        .await
        .unwrap();
    assert!(done.erased >= 3, "{done:?}");

    assert!(
        !home.join(".zcode/v2/credentials.json").exists()
            && !home.join(".zcode/cli/rollout").exists()
    );
    assert!(!home.join(".zcode/v2/config.json").exists() && !home.join(".zcode/commands").exists());
    assert!(!home
        .join("Library/Application Support/ZCode/session")
        .exists());
    assert!(
        home.join(".zcode/workspace/default/repo/.git/HEAD")
            .exists(),
        "projects are never touched"
    );
    assert!(home.join(".zcode/workspace/default/repo/a.md").exists());

    let kept = tree_text(&world.store.paths().home.join("snapshots"));
    assert!(
        !kept.contains(SECRET) && !kept.contains("conversation") && !kept.contains("runtime\n")
    );
    assert!(kept.contains("command") && kept.contains("role"));
}

#[tokio::test]
async fn zcode_is_not_cleaned_while_its_app_is_still_patched_and_not_deployed() {
    if !python3_available() {
        return;
    }
    let world = world("zcode/zcode-keysmith.py");
    let home = world.home.clone();
    write(
        home.join(".zcode-keysmith/config.json"),
        r#"{"app_bundle_modified": true}"#,
    );
    write(
        home.join(".zcode-keysmith/backups/zcode.cjs.1.original"),
        "original app\n",
    );
    write(home.join(".zcode/v2/credentials.json"), "{}");

    let plan = plan_cleanup(&world.store, ToolKind::Zcode, &world.opts)
        .await
        .unwrap();
    assert!(!plan.blockers.is_empty(), "{plan:?}");
    let refused = confirm_cleanup(&world.store, &plan.operation_id, false, &world.opts).await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(
        home.join(".zcode-keysmith/backups/zcode.cjs.1.original")
            .exists(),
        "the only copy of the original app is kept"
    );
    assert!(home.join(".zcode/v2/credentials.json").exists());
    assert!(list_snapshots(&world.store).is_empty());
}
