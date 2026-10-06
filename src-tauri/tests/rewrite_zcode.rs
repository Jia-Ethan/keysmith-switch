use std::path::{Path, PathBuf};

use keysmith_switch_lib::rewrite::zcode::{self, Unsupported, ZcodeLinkState};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};

/// The shape of a real provider_config.json (ZCode 3.14.4), with made-up hosts and keys.
fn config() -> Value {
    json!({
      "schemaVersion": 1,
      "config": {
        "providerOrder": ["da6c547d-4600-4948-bd09-8d0ee622d0a2", "new-provider", "account:zai-individual-coding-plan"],
        "providerConfigRules": {
          "providerRules": [
            {
              "providerId": "da6c547d-4600-4948-bd09-8d0ee622d0a2",
              "providerName": "Gateway",
              "config": {
                "group": "standard-personal",
                "access": {"type": "api-key", "apiKey": "sk-one"},
                "api": {"type": "openai-chat-completions", "baseUrl": "https://gateway.example/v1"},
                "personalModelIds": ["m1"],
                "modelOrder": ["m1"]
              }
            },
            {
              "providerId": "new-provider",
              "providerName": "Other",
              "config": {
                "group": "standard-personal",
                "access": {"type": "api-key", "apiKey": "sk-two"},
                "api": {"type": "openai-responses", "baseUrl": "https://other.example/v1"},
                "personalModelIds": ["m2"],
                "modelOrder": ["m2"]
              }
            },
            {
              "providerId": "account:zai-individual-coding-plan",
              "config": {"api": {"baseUrl": "https://open.bigmodel.example/api/anthropic"}}
            }
          ]
        },
        "modelConfigRules": {"providerModelRules": [], "manualProviderModelRules": []}
      }
    })
}

fn setup(value: &Value) -> (tempfile::TempDir, PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let zcode = dir.path().join(".zcode").join("v2");
    std::fs::create_dir_all(&zcode).unwrap();
    let text = serde_json::to_string_pretty(value).unwrap() + "\n";
    std::fs::write(zcode.join("provider_config.json"), &text).unwrap();
    (dir, zcode, text)
}

fn read(zcode: &Path) -> String {
    std::fs::read_to_string(zcode.join("provider_config.json")).unwrap()
}

fn relay(id: &str) -> String {
    format!("http://127.0.0.1:4000/t/tok/zcode/{id}")
}

fn url_of(text: &str, index: usize) -> String {
    let value: Value = serde_json::from_str(text).unwrap();
    value["config"]["providerConfigRules"]["providerRules"][index]["config"]["api"]["baseUrl"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn links_every_personal_provider_and_restores_the_file_byte_for_byte() {
    let (_tmp, zcode, original) = setup(&config());
    assert_eq!(zcode::providers(&zcode).unwrap().len(), 2);
    let record = zcode::link(&zcode, None, relay).unwrap();
    assert_eq!(record.providers.len(), 2);
    assert_eq!(record.providers[0].previous, "https://gateway.example/v1");

    let linked = read(&zcode);
    assert_eq!(
        url_of(&linked, 0),
        relay("da6c547d-4600-4948-bd09-8d0ee622d0a2")
    );
    assert_eq!(url_of(&linked, 1), relay("new-provider"));
    // The account provider is not touched.
    assert_eq!(
        url_of(&linked, 2),
        "https://open.bigmodel.example/api/anthropic"
    );
    // Only baseUrl values changed.
    assert_eq!(
        linked
            .replace(
                &relay("da6c547d-4600-4948-bd09-8d0ee622d0a2"),
                "https://gateway.example/v1"
            )
            .replace(&relay("new-provider"), "https://other.example/v1"),
        original
    );
    assert_eq!(
        zcode::state(Some(&record), &zcode),
        ZcodeLinkState::Linked { unrouted: 0 }
    );

    let report = zcode::unlink(&record).unwrap();
    assert_eq!(report.restored.len(), 2);
    assert_eq!(read(&zcode), original);
}

#[test]
fn a_provider_edited_in_zcode_is_left_alone_and_shows_as_bypassed() {
    let (_tmp, zcode, _) = setup(&config());
    let record = zcode::link(&zcode, None, relay).unwrap();
    let edited = read(&zcode).replace(&relay("new-provider"), "https://changed.example/v1");
    std::fs::write(zcode.join("provider_config.json"), &edited).unwrap();
    assert_eq!(
        zcode::state(Some(&record), &zcode),
        ZcodeLinkState::Bypassed
    );
    let report = zcode::unlink(&record).unwrap();
    assert_eq!(
        report.restored,
        vec!["da6c547d-4600-4948-bd09-8d0ee622d0a2".to_string()]
    );
    assert_eq!(report.left_alone, vec!["new-provider".to_string()]);
    let after = read(&zcode);
    assert_eq!(url_of(&after, 0), "https://gateway.example/v1");
    assert_eq!(url_of(&after, 1), "https://changed.example/v1");
}

#[test]
fn a_provider_added_later_is_counted_as_unrouted_and_relinking_picks_it_up() {
    let (_tmp, zcode, _) = setup(&config());
    let first = zcode::link(&zcode, None, relay).unwrap();
    let mut value: Value = serde_json::from_str(&read(&zcode)).unwrap();
    value["config"]["providerConfigRules"]["providerRules"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "providerId": "new-provider-2", "providerName": "Third",
            "config": {"group": "standard-personal", "access": {"type": "api-key", "apiKey": "k"},
                       "api": {"type": "openai-chat-completions", "baseUrl": "https://third.example/v1"}}
        }));
    std::fs::write(
        zcode.join("provider_config.json"),
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();
    assert_eq!(
        zcode::state(Some(&first), &zcode),
        ZcodeLinkState::Linked { unrouted: 1 }
    );
    let second = zcode::link(&zcode, Some(&first), relay).unwrap();
    assert_eq!(second.providers.len(), 3);
    // Already-linked providers keep the address recorded the first time.
    assert_eq!(second.providers[0].previous, "https://gateway.example/v1");
    assert_eq!(
        zcode::state(Some(&second), &zcode),
        ZcodeLinkState::Linked { unrouted: 0 }
    );
}

#[test]
fn relay_addresses_without_a_record_are_refused() {
    let (_tmp, zcode, _) = setup(&config());
    zcode::link(&zcode, None, relay).unwrap();
    assert!(zcode::link(&zcode, None, relay).is_err());
}

#[test]
fn missing_broken_and_account_only_configs_are_refused_without_writing() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        zcode::providers(&tmp.path().join("nope")),
        Err(Unsupported::NoConfig)
    );
    let (_tmp, zcode, _) = setup(&json!({"schemaVersion": 1}));
    assert_eq!(zcode::providers(&zcode), Err(Unsupported::BadConfig));
    let mut account_only = config();
    account_only["config"]["providerConfigRules"]["providerRules"]
        .as_array_mut()
        .unwrap()
        .drain(0..2);
    let (_tmp, zcode, original) = setup(&account_only);
    assert_eq!(zcode::providers(&zcode), Err(Unsupported::NoProvider));
    assert!(zcode::link(&zcode, None, relay).is_err());
    assert_eq!(read(&zcode), original);
}

#[test]
fn zcode_keysmith_never_writes_the_provider_config() {
    // The vendored ZCode adapter patches the app bundle and its own folder only.
    let script = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../third_party/keysmith/zcode/zcode-keysmith.py"),
    )
    .unwrap();
    assert!(!script.contains("provider_config.json"));
}
