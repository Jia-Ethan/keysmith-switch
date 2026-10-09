use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use keysmith_switch_lib::paths::AppPaths;
use keysmith_switch_lib::rewrite::grok::{self, GrokLinkState, Unsupported, BEGIN, END};
use keysmith_switch_lib::rewrite::{self, service};
use pretty_assertions::assert_eq;

const CONFIG: &str = "[ui]\npermission_mode = \"ask\"\n\n[models]\ndefault = \"grok-4.6\"\n";

fn catalog(models: &[(&str, &str)]) -> String {
    let entries: Vec<String> = models
        .iter()
        .map(|(id, url)| {
            format!(r#""{id}":{{"info":{{"base_url":"{url}","api_backend":"responses"}},"api_key":"secret","env_key":null,"api_base_url":null}}"#)
        })
        .collect();
    format!(
        r#"{{"auth_method":"session","models":{{{}}}}}"#,
        entries.join(",")
    )
}

fn setup(config: &str, models: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let grok = dir.path().join(".grok");
    std::fs::create_dir_all(&grok).unwrap();
    std::fs::write(grok.join("config.toml"), config).unwrap();
    std::fs::write(grok.join("models_cache.json"), catalog(models)).unwrap();
    (dir, grok)
}

fn read(grok: &Path) -> String {
    std::fs::read_to_string(grok.join("config.toml")).unwrap()
}

const PROXY: &str = "https://cli-chat-proxy.grok.com/v1";

fn table(ids: &[&str]) -> BTreeMap<String, String> {
    ids.iter()
        .map(|id| {
            (
                id.to_string(),
                format!("http://127.0.0.1:4000/t/tok/grok/u1/{id}"),
            )
        })
        .collect()
}

#[test]
fn links_every_catalog_model_in_one_region_and_unlinks_byte_for_byte() {
    let (_tmp, grok) = setup(CONFIG, &[("grok-4.6", PROXY), ("grok-4.7", PROXY)]);
    let routable = grok::routable(&grok).unwrap();
    assert_eq!(routable.len(), 2);
    let record = grok::link(&grok, table(&["grok-4.6", "grok-4.7"])).unwrap();
    let linked = read(&grok);
    assert!(linked.starts_with(CONFIG), "{linked}");
    assert_eq!(linked.matches(BEGIN).count(), 1);
    assert!(linked.contains(
        "[model.\"grok-4.6\"]\nbase_url = \"http://127.0.0.1:4000/t/tok/grok/u1/grok-4.6\""
    ));
    linked.parse::<toml_edit::DocumentMut>().unwrap();
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );

    let report = grok::unlink(&record).unwrap();
    assert!(report.removed && !report.left_alone);
    assert_eq!(read(&grok), CONFIG);
}

#[test]
fn grok_appending_its_own_tables_after_the_region_does_not_bypass_it() {
    let (_tmp, grok) = setup(CONFIG, &[("grok-4.6", PROXY)]);
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    let appended = read(&grok) + "\n[marketplace]\ndefault_skills_installs_purged = true\n";
    std::fs::write(grok.join("config.toml"), &appended).unwrap();
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );
    grok::unlink(&record).unwrap();
    assert_eq!(
        read(&grok),
        CONFIG.to_string() + "\n[marketplace]\ndefault_skills_installs_purged = true\n"
    );
}

#[test]
fn a_model_the_person_configured_is_left_alone() {
    let own = format!("{CONFIG}\n[model.\"grok-4.6\"]\nbase_url = \"https://my.gateway/v1\"\n");
    let (_tmp, grok) = setup(&own, &[("grok-4.6", PROXY), ("grok-4.7", PROXY)]);
    let ids: Vec<String> = grok::routable(&grok)
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, vec!["grok-4.7".to_string()]);
}

/// A custom model that sends `grok-4.7` to its own address with its own key. Grok looks the
/// model up by that id and, finding a `[model."grok-4.7"]` table with no key, would send the
/// sign-in token to this address.
const CUSTOM: &str = "\n[model.\"my-grok-4.7\"]\nmodel = \"grok-4.7\"\nbase_url = \"https://gateway.example/v1\"\napi_key = \"sk-test\"\n";

fn ids(models: Vec<grok::Model>) -> Vec<String> {
    models.into_iter().map(|model| model.id).collect()
}

#[test]
fn a_model_a_custom_model_uses_as_its_upstream_is_left_out() {
    let (_tmp, grok) = setup(
        &format!("{CONFIG}{CUSTOM}"),
        &[("grok-4.6", PROXY), ("grok-4.7", PROXY)],
    );
    assert_eq!(ids(grok::routable(&grok).unwrap()), vec!["grok-4.6"]);
    assert_eq!(grok::left_out(&grok), vec!["grok-4.7"]);
}

#[test]
fn upstreams_are_found_however_the_custom_model_is_written() {
    // Dotted keys and inline tables only belong to `model` before the first table header.
    let configs = [
        format!("{CONFIG}\n[model.mine]\nmodel = \"grok-4.7\"\n"),
        format!("{CONFIG}\n[model.\"mine\"]\nbase_url = \"https://x/v1\"\nmodel = 'grok-4.7'\n"),
        format!("model.mine.model = \"grok-4.7\"\n{CONFIG}"),
        format!("model = {{ mine = {{ model = \"grok-4.7\" }} }}\n{CONFIG}"),
    ];
    for config in configs {
        let (_tmp, grok) = setup(&config, &[("grok-4.6", PROXY), ("grok-4.7", PROXY)]);
        assert_eq!(
            ids(grok::routable(&grok).unwrap()),
            vec!["grok-4.6"],
            "{config}"
        );
    }
}

#[test]
fn a_custom_model_with_another_upstream_changes_nothing() {
    let other = CUSTOM.replace("\"grok-4.7\"", "\"some-other-model\"");
    let (_tmp, grok) = setup(
        &format!("{CONFIG}{other}"),
        &[("grok-4.6", PROXY), ("grok-4.7", PROXY)],
    );
    assert_eq!(ids(grok::routable(&grok).unwrap()).len(), 2);
    assert!(grok::left_out(&grok).is_empty());
}

#[test]
fn a_table_of_the_persons_own_is_not_also_listed_as_left_out() {
    let own =
        format!("{CONFIG}{CUSTOM}\n[model.\"grok-4.7\"]\nbase_url = \"https://my.gateway/v1\"\n");
    let (_tmp, grok) = setup(&own, &[("grok-4.6", PROXY), ("grok-4.7", PROXY)]);
    assert_eq!(ids(grok::routable(&grok).unwrap()), vec!["grok-4.6"]);
    assert!(grok::left_out(&grok).is_empty());
}

#[test]
fn a_custom_model_added_after_linking_is_reported_until_reconnecting() {
    let (_tmp, grok) = setup(CONFIG, &[("grok-4.6", PROXY), ("grok-4.7", PROXY)]);
    let record = grok::link(&grok, table(&["grok-4.6", "grok-4.7"])).unwrap();
    let with_custom = read(&grok) + CUSTOM;
    std::fs::write(grok.join("config.toml"), &with_custom).unwrap();
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 1
        }
    );

    // Reconnecting leaves the model out; the custom model stays as the person wrote it.
    let routed: Vec<String> = ids(grok::routable(&grok).unwrap());
    assert_eq!(routed, vec!["grok-4.6"]);
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );
    grok::unlink(&record).unwrap();
    assert_eq!(read(&grok), format!("{CONFIG}{CUSTOM}"));
}

#[test]
fn a_model_that_cannot_be_routed_is_not_counted_as_unrouted() {
    let (_tmp, grok) = setup(
        &format!("{CONFIG}{CUSTOM}"),
        &[("grok-4.6", PROXY), ("grok-4.7", PROXY)],
    );
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );
}

#[test]
fn nothing_is_connected_when_no_model_can_be_routed() {
    let (tmp, grok) = setup(&format!("{CONFIG}{CUSTOM}"), &[("grok-4.7", PROXY)]);
    assert_eq!(grok::routable(&grok).unwrap_err(), Unsupported::NoModels);

    // Never touch the real LaunchAgent or relay, whatever this test ends up calling.
    std::env::set_var(
        "KEYSMITH_SWITCH_RELAY_LABEL",
        format!(
            "com.jia-ethan.keysmith-switch.relay.grok-test-{}",
            std::process::id()
        ),
    );
    std::env::set_var(
        "KEYSMITH_SWITCH_LAUNCH_AGENTS_DIR",
        tmp.path().join("LaunchAgents"),
    );

    // The relay's routes are left as they are, so a link made earlier keeps working.
    let paths = AppPaths::from_home(tmp.path().join(".keysmith-switch"));
    let mut config = service::ensure_config(&paths).unwrap();
    config.routes.insert(
        "grok/u1".into(),
        "https://cli-chat-proxy.grok.com/v1".into(),
    );
    service::write_config(&paths, &config).unwrap();

    let error = rewrite::connect_grok(&paths, Some(tmp.path())).unwrap_err();
    assert!(error.to_string().contains("no-models"), "{error}");
    assert_eq!(service::read_config(&paths).unwrap(), config);
    assert_eq!(read(&grok), format!("{CONFIG}{CUSTOM}"));
}

#[test]
fn a_new_catalog_model_counts_as_unrouted() {
    let (_tmp, grok) = setup(CONFIG, &[("grok-4.6", PROXY)]);
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    std::fs::write(
        grok.join("models_cache.json"),
        catalog(&[("grok-4.6", PROXY), ("grok-5", PROXY)]),
    )
    .unwrap();
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 1,
            clashing: 0
        }
    );
}

#[test]
fn an_edited_region_is_bypassed_and_left_alone() {
    let (_tmp, grok) = setup(CONFIG, &[("grok-4.6", PROXY)]);
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    let edited = read(&grok).replace("4000", "5000");
    std::fs::write(grok.join("config.toml"), &edited).unwrap();
    assert_eq!(grok::state(Some(&record), &grok), GrokLinkState::Bypassed);
    let report = grok::unlink(&record).unwrap();
    assert!(!report.removed && report.left_alone);
    assert_eq!(read(&grok), edited);
}

#[test]
fn damaged_markers_and_a_missing_catalog_are_refused_without_writing() {
    let damaged = format!("{CONFIG}{BEGIN}\n[model.\"x\"]\n");
    let (_tmp, grok) = setup(&damaged, &[("grok-4.6", PROXY)]);
    assert_eq!(grok::routable(&grok).unwrap_err(), Unsupported::BadRegion);
    assert!(grok::link(&grok, table(&["grok-4.6"])).is_err());
    assert_eq!(read(&grok), damaged);

    let (_tmp, grok) = setup(CONFIG, &[]);
    assert_eq!(grok::routable(&grok).unwrap_err(), Unsupported::NoCatalog);
    let _ = END;
}

fn python() -> Option<String> {
    ["python3", "python"]
        .into_iter()
        .find(|name| {
            std::process::Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
        .map(str::to_string)
}

/// grok-keysmith deploys and uninstalls around the link; the link must survive both, and the
/// adapter must not see it as drift.
#[test]
fn prompt_deploy_and_uninstall_keep_the_link() {
    let Some(python) = python() else {
        eprintln!("python not found; skipping");
        return;
    };
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/keysmith/grok/grok-keysmith.py");
    let (tmp, grok) = setup(CONFIG, &[("grok-4.6", PROXY)]);
    let prompt = tmp.path().join("p.md");
    std::fs::write(&prompt, "# Test\nhello\n").unwrap();
    let run = |args: &[&str]| {
        let out = std::process::Command::new(&python)
            .arg(&script)
            .args([
                "--json",
                "--lang",
                "en",
                "--grok-dir",
                grok.to_str().unwrap(),
            ])
            .args(args)
            .env("HOME", tmp.path())
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(
            out.status.success(),
            "{args:?}\n{text}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        text
    };
    // Linked first, then deploy, then uninstall.
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    run(&["--file", prompt.to_str().unwrap(), "--name", "t", "--yes"]);
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );
    let status = run(&["--status"]);
    assert!(
        !status.contains("does not match managed after-state"),
        "{status}"
    );
    run(&["--uninstall", "--yes"]);
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );

    // Deploy first, then link, then uninstall.
    grok::unlink(&record).unwrap();
    run(&["--file", prompt.to_str().unwrap(), "--name", "t", "--yes"]);
    let record = grok::link(&grok, table(&["grok-4.6"])).unwrap();
    run(&["--uninstall", "--yes"]);
    assert_eq!(
        grok::state(Some(&record), &grok),
        GrokLinkState::Linked {
            unrouted: 0,
            clashing: 0
        }
    );
    grok::unlink(&record).unwrap();
    assert_eq!(read(&grok), CONFIG);
}
