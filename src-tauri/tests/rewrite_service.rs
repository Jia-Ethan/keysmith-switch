//! Installs the relay as a LaunchAgent under a throwaway label and folder, connects
//! temporary Codex and Claude Code configs to it, and sends a request through. macOS only, and only when
//! KEYSMITH_SWITCH_LAUNCHD_TEST=1, because it talks to the real launchd.

#![cfg(target_os = "macos")]

use std::io::{Read, Write};
use std::net::TcpListener;

use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::models::SettingsPatch;
use keysmith_switch_lib::paths::AppPaths;
use keysmith_switch_lib::rewrite::{self, claude::ClaudeLinkState, link::LinkState, service};

#[test]
fn connect_send_disconnect() {
    if std::env::var("KEYSMITH_SWITCH_LAUNCHD_TEST").as_deref() != Ok("1") {
        eprintln!("set KEYSMITH_SWITCH_LAUNCHD_TEST=1 to run");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let agents = tmp.path().join("LaunchAgents");
    std::env::set_var("KEYSMITH_SWITCH_LAUNCH_AGENTS_DIR", &agents);
    std::env::set_var(
        "KEYSMITH_SWITCH_RELAY_LABEL",
        format!(
            "com.jia-ethan.keysmith-switch.relay.test-{}",
            std::process::id()
        ),
    );

    // Upstream that echoes the body it received.
    let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
    let up_port = upstream.local_addr().unwrap().port();
    let received = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let sink = received.clone();
    std::thread::spawn(move || {
        for stream in upstream.incoming() {
            let mut stream = stream.unwrap();
            let mut buf = vec![0u8; 65536];
            let mut total = Vec::new();
            loop {
                let n = stream.read(&mut buf).unwrap();
                total.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&total).to_string();
                if let Some(split) = text.find("\r\n\r\n") {
                    let len = text[..split]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if total.len() >= split + 4 + len {
                        *sink.lock().unwrap() =
                            String::from_utf8_lossy(&total[split + 4..]).to_string();
                        break;
                    }
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 11\r\n\r\ndata: ok\n\n")
                .unwrap();
        }
    });

    let home = tmp.path().join("home");
    let codex = home.join(".codex");
    std::fs::create_dir_all(&codex).unwrap();
    std::fs::write(
        codex.join("config.toml"),
        format!("model_provider = \"c\"\n[model_providers.c]\nname = \"c\"\nbase_url = \"http://127.0.0.1:{up_port}/v1\"\nwire_api = \"responses\"\n"),
    )
    .unwrap();

    let claude = home.join(".claude");
    std::fs::create_dir_all(&claude).unwrap();
    let claude_settings = format!(
        "{{\n  \"env\": {{\n    \"ANTHROPIC_BASE_URL\": \"http://127.0.0.1:{up_port}\"\n  }}\n}}\n"
    );
    std::fs::write(claude.join("settings.json"), &claude_settings).unwrap();

    let paths = AppPaths::from_home(tmp.path().join(".keysmith-switch"));
    let store = Store::open(&paths).unwrap();
    rewrite::change(&store, |store| {
        store.save_user_rules(&[keysmith_rewrite::Rule::new("提示词", "指令")])?;
        store.update_settings(SettingsPatch {
            rewrite_enabled: Some(true),
            ..Default::default()
        })?;
        Ok(())
    })
    .unwrap();

    let log = paths.logs.join("relay.log");
    let result = std::panic::catch_unwind(|| {
        rewrite::connect_codex(&paths, Some(&home)).unwrap();
        let view = rewrite::view_with(&store, Some(&home)).unwrap();
        assert_eq!(
            view.codex.link,
            LinkState::Linked {
                provider: "c".into()
            }
        );
        assert!(view.codex.service.installed && view.codex.service.running);

        let config = service::read_config(&paths).unwrap();
        let url = format!("{}/v1/responses", service::base_url(&config, "c"));
        let out = std::process::Command::new("curl")
            .args([
                "-sS",
                "-X",
                "POST",
                &url,
                "-H",
                "content-type: application/json",
                "--data-binary",
            ])
            .arg(r#"{"input":[{"role":"user","content":"改提示词"}]}"#)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "data: ok\n\n");
        assert!(received.lock().unwrap().contains("改指令"));

        // Claude Code shares the relay. Its settings point at the same echoing upstream.
        rewrite::connect_claude(&paths, Some(&home)).unwrap();
        let view = rewrite::view_with(&store, Some(&home)).unwrap();
        assert_eq!(view.claude.link, ClaudeLinkState::Linked);
        assert_eq!(view.claude.upstream_host.as_deref(), Some("127.0.0.1"));
        let settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(claude.join("settings.json")).unwrap())
                .unwrap();
        let base = settings["env"]["ANTHROPIC_BASE_URL"]
            .as_str()
            .unwrap()
            .to_string();
        let out = std::process::Command::new("curl")
            .args(["-sS", "-X", "POST", &format!("{base}/v1/messages?beta=true")])
            .args(["-H", "content-type: application/json", "--data-binary"])
            .arg(r#"{"messages":[{"role":"user","content":[{"type":"text","text":"<system-reminder>提示词</system-reminder>"},{"type":"text","text":"改提示词"}]}]}"#)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "data: ok\n\n");
        let body = received.lock().unwrap().clone();
        assert!(
            body.contains("<system-reminder>提示词</system-reminder>") && body.contains("改指令"),
            "{body}"
        );

        // Disconnecting Codex leaves the relay running for Claude Code.
        rewrite::disconnect_codex(&paths).unwrap();
        assert!(service::status(&paths).running);
    });

    rewrite::disconnect_codex(&paths).unwrap();
    rewrite::disconnect_claude(&paths).unwrap();
    let after = std::fs::read_to_string(codex.join("config.toml")).unwrap();
    assert!(after.contains("model_provider = \"c\""));
    assert!(!after.contains("keysmith-relay"));
    assert_eq!(
        std::fs::read_to_string(claude.join("settings.json")).unwrap(),
        claude_settings
    );
    assert!(!service::status(&paths).running);
    if result.is_err() {
        eprintln!(
            "relay.log:\n{}",
            std::fs::read_to_string(&log).unwrap_or_default()
        );
    }
    result.unwrap();
}
