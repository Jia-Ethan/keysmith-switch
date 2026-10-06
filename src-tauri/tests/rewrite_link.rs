use std::path::{Path, PathBuf};
use std::process::Command;

use keysmith_switch_lib::rewrite::link::{self, LinkState, Unsupported, MANAGED_PROVIDER};
use pretty_assertions::assert_eq;

const CONFIG: &str = r#"# my settings
model_provider = "custom"
model = "gpt-6"
model_instructions_file = "./mine.md"

[model_providers.custom]
name = "custom"
wire_api = "responses"
requires_openai_auth = false
base_url = "https://example.test/v1"
experimental_bearer_token = "sk-secret"   # keep me

[plugins."x@y"]
enabled = true
"#;

fn setup(config: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let codex = dir.path().join(".codex");
    std::fs::create_dir_all(&codex).unwrap();
    std::fs::write(codex.join("config.toml"), config).unwrap();
    (dir, codex)
}

fn read(codex: &Path) -> String {
    std::fs::read_to_string(codex.join("config.toml")).unwrap()
}

const RELAY: &str = "http://127.0.0.1:4000/r/tok/custom";

#[test]
fn link_adds_managed_provider_and_switches_to_it() {
    let (_tmp, codex) = setup(CONFIG);
    let (record, provider) = link::link(&codex, None, RELAY).unwrap();
    assert_eq!(provider.base_url, "https://example.test/v1");
    assert_eq!(record.previous_provider.as_deref(), Some("custom"));
    let text = read(&codex);
    let doc: toml_edit::DocumentMut = text.parse().unwrap();
    assert_eq!(doc["model_provider"].as_str(), Some(MANAGED_PROVIDER));
    let managed = &doc["model_providers"][MANAGED_PROVIDER];
    assert_eq!(managed["base_url"].as_str(), Some(RELAY));
    assert_eq!(
        managed["experimental_bearer_token"].as_str(),
        Some("sk-secret")
    );
    assert_eq!(managed["supports_websockets"].as_bool(), Some(false));
    // The person's own table, comments and other fields are untouched.
    assert!(text.contains("# my settings"));
    assert!(text.contains(r#"base_url = "https://example.test/v1""#));
    assert!(text.contains("# keep me"));
    assert!(text.contains(r#"model_instructions_file = "./mine.md""#));
    assert_eq!(
        link::state(Some(&record), &codex),
        LinkState::Linked {
            provider: "custom".into()
        }
    );
    // A backup was written next to the file.
    assert!(std::fs::read_dir(&codex).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("config.toml.keysmith-relay-")));
}

#[test]
fn unlink_restores_exactly_what_link_changed() {
    let (_tmp, codex) = setup(CONFIG);
    let (record, _) = link::link(&codex, None, RELAY).unwrap();
    let report = link::unlink(&record).unwrap();
    assert!(report.restored_provider && report.removed_managed);
    assert!(report.left_alone.is_empty());
    let a: toml_edit::DocumentMut = read(&codex).parse().unwrap();
    let b: toml_edit::DocumentMut = CONFIG.parse().unwrap();
    assert_eq!(a.to_string().trim(), b.to_string().trim());
}

#[test]
fn unlink_leaves_values_someone_else_changed() {
    let (_tmp, codex) = setup(CONFIG);
    let (record, _) = link::link(&codex, None, RELAY).unwrap();
    // cc-switch style: another tool switches the provider.
    let switched = read(&codex).replace(
        &format!("model_provider = \"{MANAGED_PROVIDER}\""),
        "model_provider = \"other\"",
    );
    std::fs::write(codex.join("config.toml"), switched).unwrap();
    assert_eq!(
        link::state(Some(&record), &codex),
        LinkState::Bypassed {
            provider: Some("other".into())
        }
    );
    let report = link::unlink(&record).unwrap();
    assert!(!report.restored_provider);
    assert_eq!(report.left_alone, vec!["model_provider".to_string()]);
    assert!(report.removed_managed);
    assert!(read(&codex).contains("model_provider = \"other\""));
}

#[test]
fn relink_while_linked_keeps_the_original_provider() {
    let (_tmp, codex) = setup(CONFIG);
    let (first, _) = link::link(&codex, None, RELAY).unwrap();
    let (second, provider) = link::link(&codex, Some(&first), RELAY).unwrap();
    assert_eq!(provider.id, "custom");
    assert_eq!(second.previous_provider.as_deref(), Some("custom"));
}

#[test]
fn unsupported_providers_are_reported_without_writing() {
    let (_tmp, codex) = setup("model = \"gpt-5\"\n");
    assert_eq!(
        link::current_provider(&codex),
        Err(Unsupported::BuiltIn("openai".into()))
    );
    assert!(link::link(&codex, None, RELAY).is_err());
    assert_eq!(read(&codex), "model = \"gpt-5\"\n");

    let (_tmp, codex) = setup(
        "model_provider = \"c\"\n[model_providers.c]\nbase_url = \"http://x/v1\"\nwire_api = \"chat\"\n",
    );
    assert_eq!(
        link::current_provider(&codex),
        Err(Unsupported::WireApi("c".into()))
    );

    let (_tmp, codex) = setup("model_provider = \"gone\"\n");
    assert_eq!(
        link::current_provider(&codex),
        Err(Unsupported::MissingProvider("gone".into()))
    );
}

#[test]
fn link_without_a_previous_top_level_provider_unlinks_by_removing_it() {
    let config = "[model_providers.c]\nbase_url = \"http://x/v1\"\n";
    let (_tmp, codex) = setup(&format!("model_provider = \"c\"\n{config}"));
    let (mut record, _) = link::link(&codex, None, RELAY).unwrap();
    record.previous_provider = None;
    link::unlink(&record).unwrap();
    assert!(!read(&codex).contains("model_provider ="));
}

fn python() -> Option<String> {
    ["python3", "python"]
        .into_iter()
        .find(|name| {
            Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
        })
        .map(str::to_string)
}

/// The prompt deploy owns `model_instructions_file`; deploying and undoing it must leave the
/// relay link in place.
#[test]
fn prompt_deploy_and_uninstall_keep_the_link() {
    let Some(python) = python() else {
        eprintln!("python not found; skipping");
        return;
    };
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/keysmith/codex/codex-instruct.py");
    let config = CONFIG.replace("model_instructions_file = \"./mine.md\"\n", "");
    let (tmp, codex) = setup(&config);
    let (record, _) = link::link(&codex, None, RELAY).unwrap();
    let prompt = tmp.path().join("p.md");
    std::fs::write(&prompt, "# Test\nhello\n").unwrap();

    let run = |args: &[&str]| {
        let out = Command::new(&python)
            .arg(&script)
            .args(args)
            .args([
                "--codex-dir",
                codex.to_str().unwrap(),
                "--lang",
                "en",
                "--yes",
            ])
            .env("HOME", tmp.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["--file", prompt.to_str().unwrap(), "--name", "ks-test"]);
    assert!(read(&codex).contains("model_instructions_file"));
    assert_eq!(
        link::state(Some(&record), &codex),
        LinkState::Linked {
            provider: "custom".into()
        }
    );

    run(&["--uninstall"]);
    assert!(!read(&codex).contains("model_instructions_file"));
    assert_eq!(
        link::state(Some(&record), &codex),
        LinkState::Linked {
            provider: "custom".into()
        }
    );

    link::unlink(&record).unwrap();
    assert!(read(&codex).contains("model_provider = \"custom\""));
}
