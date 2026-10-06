use std::path::{Path, PathBuf};
use std::process::Command;

use keysmith_switch_lib::rewrite::claude::{self, ClaudeLinkState, Unsupported, ENV_KEY};
use pretty_assertions::assert_eq;

const SETTINGS: &str = r#"{
  "model": "opus",
  "env": {
    "ANTHROPIC_API_KEY": "sk-secret",
    "ANTHROPIC_BASE_URL": "https://gateway.example/v1",
    "DISABLE_TELEMETRY": "1"
  },
  "permissions": {
    "allow": ["Bash(ls:*)"]
  }
}
"#;

const RELAY: &str = "http://127.0.0.1:4000/t/tok/claude/anthropic";

fn setup(settings: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let claude = dir.path().join(".claude");
    std::fs::create_dir_all(&claude).unwrap();
    if let Some(settings) = settings {
        std::fs::write(claude.join("settings.json"), settings).unwrap();
    }
    (dir, claude)
}

fn read(claude: &Path) -> String {
    std::fs::read_to_string(claude.join("settings.json")).unwrap()
}

#[test]
fn link_changes_one_value_and_unlink_puts_it_back_byte_for_byte() {
    let (_tmp, claude) = setup(Some(SETTINGS));
    assert_eq!(
        claude::current_upstream(&claude).unwrap(),
        "https://gateway.example/v1"
    );
    let record = claude::link(&claude, None, RELAY).unwrap();
    assert_eq!(
        record.previous.as_deref(),
        Some("https://gateway.example/v1")
    );
    assert_eq!(
        read(&claude),
        SETTINGS.replace("https://gateway.example/v1", RELAY)
    );
    assert_eq!(
        claude::state(Some(&record), &claude),
        ClaudeLinkState::Linked
    );

    let report = claude::unlink(&record).unwrap();
    assert!(report.restored && !report.left_alone);
    assert_eq!(read(&claude), SETTINGS);
}

#[test]
fn without_a_base_url_the_relay_forwards_to_anthropic_and_unlink_removes_the_key() {
    let original = "{\n    \"model\": \"opus\"\n}\n";
    let (_tmp, claude) = setup(Some(original));
    assert_eq!(
        claude::current_upstream(&claude).unwrap(),
        claude::DEFAULT_UPSTREAM
    );
    let record = claude::link(&claude, None, RELAY).unwrap();
    assert_eq!(record.previous, None);
    // Four-space indentation is kept.
    assert!(read(&claude).contains("\n    \"env\": {\n        \"ANTHROPIC_BASE_URL\""));
    claude::unlink(&record).unwrap();
    assert_eq!(read(&claude), original);
}

#[test]
fn a_missing_settings_file_is_created_and_emptied_again() {
    let (_tmp, claude) = setup(None);
    let record = claude::link(&claude, None, RELAY).unwrap();
    let value: serde_json::Value = serde_json::from_str(&read(&claude)).unwrap();
    assert_eq!(value["env"][ENV_KEY], RELAY);
    claude::unlink(&record).unwrap();
    assert_eq!(read(&claude), "{}\n");
}

#[test]
fn a_value_changed_by_someone_else_is_left_alone() {
    let (_tmp, claude) = setup(Some(SETTINGS));
    let record = claude::link(&claude, None, RELAY).unwrap();
    let changed = read(&claude).replace(RELAY, "https://other.example");
    std::fs::write(claude.join("settings.json"), &changed).unwrap();
    assert_eq!(
        claude::state(Some(&record), &claude),
        ClaudeLinkState::Bypassed
    );
    let report = claude::unlink(&record).unwrap();
    assert!(!report.restored && report.left_alone);
    assert_eq!(read(&claude), changed);
}

#[test]
fn relinking_keeps_the_first_recorded_address() {
    let (_tmp, claude) = setup(Some(SETTINGS));
    let first = claude::link(&claude, None, RELAY).unwrap();
    let second = claude::link(&claude, Some(&first), RELAY).unwrap();
    assert_eq!(
        second.previous.as_deref(),
        Some("https://gateway.example/v1")
    );
    // An address already on the relay with no record is not mistaken for the user's own.
    assert!(claude::link(&claude, None, RELAY).is_err());
}

#[test]
fn bedrock_vertex_and_broken_files_are_refused() {
    let (_tmp, claude) = setup(Some(r#"{"env":{"CLAUDE_CODE_USE_BEDROCK":"1"}}"#));
    assert_eq!(claude::current_upstream(&claude), Err(Unsupported::Bedrock));
    let (_tmp, claude) = setup(Some(r#"{"env":{"CLAUDE_CODE_USE_VERTEX":"true"}}"#));
    assert_eq!(claude::current_upstream(&claude), Err(Unsupported::Vertex));
    let (_tmp, claude) = setup(Some(r#"{"env":{"CLAUDE_CODE_USE_BEDROCK":"0"}}"#));
    assert!(claude::current_upstream(&claude).is_ok());
    let (_tmp, claude) = setup(Some("[1,2]"));
    assert_eq!(
        claude::current_upstream(&claude),
        Err(Unsupported::BadSettings)
    );
    assert!(claude::link(&claude, None, RELAY).is_err());
    assert_eq!(read(&claude), "[1,2]");
}

fn python() -> Option<String> {
    ["python3", "python"]
        .into_iter()
        .find(|name| {
            Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(str::to_string)
}

#[test]
fn prompt_deploy_runtime_and_uninstall_keep_the_link() {
    let Some(python) = python() else {
        eprintln!("python not found; skipping");
        return;
    };
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/keysmith/claude/claude-instruct.py");
    let (tmp, claude) = setup(Some(SETTINGS));
    let record = claude::link(&claude, None, RELAY).unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(&python)
            .arg(&script)
            .args(args)
            .env("HOME", tmp.path())
            .env("CLAUDE_KEYSMITH_HOME", tmp.path())
            .env_remove("CLAUDE_CONFIG_DIR")
            .env("SHELL", "/bin/zsh")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["install", "--scope", "user", "--runtime", "--yes"]);
    assert_eq!(
        claude::state(Some(&record), &claude),
        ClaudeLinkState::Linked
    );
    run(&["uninstall", "--scope", "user", "--runtime", "--yes"]);
    assert_eq!(
        claude::state(Some(&record), &claude),
        ClaudeLinkState::Linked
    );
    // Unlinking afterwards still restores the user's own address.
    claude::unlink(&record).unwrap();
    let value: serde_json::Value = serde_json::from_str(&read(&claude)).unwrap();
    assert_eq!(value["env"][ENV_KEY], "https://gateway.example/v1");
    assert_eq!(value["env"]["ANTHROPIC_API_KEY"], "sk-secret");
}
