use std::io::Write;
use std::net::{Shutdown, TcpListener};
use std::sync::{Mutex, OnceLock};
use std::thread;

use httpmock::prelude::*;
use keysmith_switch_lib::updater::{
    bootstrap_reason_for_metadata, check_update, fixture_manifest, install_update,
    resolve_update_endpoint, runtime_update_config, updater_error_install, updater_fixture_dir,
    verify_minisign, InstallMode, InstallRequest, UpdateChannel, UpdateReason, UpdateRequest,
    APP_VERSION, BETA_ENDPOINT, FIXTURE_PUBKEY, RELEASE_PAGE, STABLE_ENDPOINT,
};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load_bytes(name: &str) -> Vec<u8> {
    std::fs::read(updater_fixture_dir().join(name)).expect(name)
}

fn load_text(name: &str) -> String {
    std::fs::read_to_string(updater_fixture_dir().join(name))
        .expect(name)
        .trim()
        .to_string()
}

fn pubkey() -> String {
    load_text("TEST_ONLY.minisign.key.pub")
}

fn base_req(server: &MockServer) -> UpdateRequest {
    UpdateRequest {
        current_version: Some(APP_VERSION.to_string()),
        endpoint_base: Some(server.base_url()),
        pubkey: Some(pubkey()),
        platform_key: Some("darwin-aarch64".to_string()),
        ..UpdateRequest::default()
    }
}

fn serve_json<'a>(server: &'a MockServer, path: &str, body: &str) -> httpmock::Mock<'a> {
    server.mock(|when, then| {
        when.method(GET).path(path);
        then.status(200)
            .header("content-type", "application/json")
            .body(body);
    })
}

fn serve_artifact<'a>(server: &'a MockServer, path: &str, name: &str) -> httpmock::Mock<'a> {
    let bytes = load_bytes(name);
    server.mock(|when, then| {
        when.method(GET).path(path);
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(bytes);
    })
}

fn manifest_with_policy(
    version: &str,
    url: &str,
    signature: &str,
    minimum_updater_version: Option<&str>,
    size: Option<serde_json::Value>,
) -> String {
    let mut asset = serde_json::json!({ "url": url, "signature": signature });
    if let Some(size) = size {
        asset["size"] = size;
    }
    let mut manifest = serde_json::json!({
        "version": version,
        "notes": format!("Keysmith Switch {version}"),
        "pub_date": "2026-08-19T00:00:00Z",
        "platforms": { "darwin-aarch64": asset },
    });
    if let Some(minimum) = minimum_updater_version {
        manifest["minimum_updater_version"] = serde_json::json!(minimum);
    }
    manifest.to_string()
}

#[test]
fn verify_minisign_accepts_fixture_artifact() {
    let data = load_bytes("artifact-0.2.0.bin");
    let sig = load_text("artifact-0.2.0.bin.sig");
    verify_minisign(&pubkey(), &data, &sig).expect("fixture signature must verify");
    verify_minisign(FIXTURE_PUBKEY, &data, &sig).expect("tauri.conf fixture pubkey must verify");
}

#[test]
fn resolve_update_endpoint_selects_stable_and_beta() {
    assert_eq!(
        resolve_update_endpoint(UpdateChannel::Stable, None, None),
        STABLE_ENDPOINT
    );
    assert_eq!(
        resolve_update_endpoint(UpdateChannel::Beta, None, None),
        BETA_ENDPOINT
    );
    assert_eq!(
        resolve_update_endpoint(
            UpdateChannel::Beta,
            Some("http://127.0.0.1:9/latest.json"),
            None
        ),
        "http://127.0.0.1:9/latest.json"
    );
    assert_eq!(
        resolve_update_endpoint(UpdateChannel::Beta, None, Some("http://example.invalid")),
        "http://example.invalid/beta/latest.json"
    );
}

#[test]
fn runtime_config_uses_the_same_beta_endpoint_and_platform() {
    let req = UpdateRequest {
        channel: Some(UpdateChannel::Beta),
        pubkey: Some("fixture-pubkey".into()),
        platform_key: Some("darwin-aarch64".into()),
        ..UpdateRequest::default()
    };
    let runtime = runtime_update_config(&req);
    assert_eq!(runtime.endpoint, BETA_ENDPOINT);
    assert_eq!(runtime.pubkey, "fixture-pubkey");
    assert_eq!(runtime.platform_key, "darwin-aarch64");
}

#[test]
fn check_update_selects_stable_endpoint_by_default() {
    let _guard = env_lock();
    let server = MockServer::start();
    let artifact = "/artifact-0.2.0.bin";
    let sig = load_text("artifact-0.2.0.bin.sig");
    let body = fixture_manifest("0.2.0", &server.url(artifact), &sig);
    let stable = serve_json(&server, "/releases/latest/download/latest.json", &body);
    let beta = serve_json(
        &server,
        "/beta/latest.json",
        &fixture_manifest(
            "0.2.0-beta.1",
            &server.url("/artifact-0.2.0-beta.1.bin"),
            &load_text("artifact-0.2.0-beta.1.bin.sig"),
        ),
    );
    let check = check_update(&base_req(&server));
    assert!(check.error.is_none(), "{:?}", check.error);
    assert!(check.available);
    assert_eq!(check.channel, UpdateChannel::Stable);
    assert_eq!(check.latest_version.as_deref(), Some("0.2.0"));
    assert_eq!(check.current_version, APP_VERSION);
    assert_eq!(check.release_page, RELEASE_PAGE);
    assert_eq!(check.install_mode, InstallMode::InApp);
    assert_eq!(check.reason, None);
    stable.assert();
    assert_eq!(beta.hits(), 0);
}

#[test]
fn check_update_selects_beta_channel() {
    let _guard = env_lock();
    let server = MockServer::start();
    let sig = load_text("artifact-0.2.0-beta.1.bin.sig");
    let body = fixture_manifest(
        "0.2.0-beta.1",
        &server.url("/artifact-0.2.0-beta.1.bin"),
        &sig,
    );
    let beta = serve_json(&server, "/beta/latest.json", &body);
    let stable = serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest(
            "0.2.0",
            &server.url("/artifact-0.2.0.bin"),
            &load_text("artifact-0.2.0.bin.sig"),
        ),
    );
    let mut req = base_req(&server);
    req.channel = Some(UpdateChannel::Beta);
    let check = check_update(&req);
    assert!(check.error.is_none(), "{:?}", check.error);
    assert_eq!(check.channel, UpdateChannel::Beta);
    assert_eq!(check.latest_version.as_deref(), Some("0.2.0-beta.1"));
    beta.assert();
    assert_eq!(stable.hits(), 0);
}

#[test]
fn check_update_env_channel_selects_beta() {
    let _guard = env_lock();
    let server = MockServer::start();
    let body = fixture_manifest(
        "0.2.0-beta.1",
        &server.url("/artifact-0.2.0-beta.1.bin"),
        &load_text("artifact-0.2.0-beta.1.bin.sig"),
    );
    let beta = serve_json(&server, "/beta/latest.json", &body);
    std::env::set_var("KEYSMITH_SWITCH_UPDATE_CHANNEL", "beta");
    let check = check_update(&base_req(&server));
    std::env::remove_var("KEYSMITH_SWITCH_UPDATE_CHANNEL");
    assert_eq!(check.channel, UpdateChannel::Beta);
    assert_eq!(check.latest_version.as_deref(), Some("0.2.0-beta.1"));
    beta.assert();
}

#[test]
fn check_update_rejects_downgrade() {
    let _guard = env_lock();
    let server = MockServer::start();
    let sig = load_text("artifact-0.0.9.bin.sig");
    let body = fixture_manifest("0.0.9", &server.url("/artifact-0.0.9.bin"), &sig);
    serve_json(&server, "/releases/latest/download/latest.json", &body);
    let check = check_update(&base_req(&server));
    assert!(!check.available);
    assert_eq!(check.current_version, APP_VERSION);
    assert_eq!(check.latest_version.as_deref(), Some("0.0.9"));
    assert!(
        check
            .error
            .as_deref()
            .unwrap_or("")
            .contains("downgrade rejected"),
        "{:?}",
        check.error
    );
    assert!(!check.restart_required);
}

#[test]
fn check_update_rejects_corrupt_metadata() {
    let _guard = env_lock();
    let server = MockServer::start();
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        "{not-json",
    );
    let check = check_update(&base_req(&server));
    assert!(!check.available);
    assert_eq!(check.current_version, APP_VERSION);
    assert!(
        check
            .error
            .as_deref()
            .unwrap_or("")
            .contains("corrupt metadata"),
        "{:?}",
        check.error
    );

    let server = MockServer::start();
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        r#"{"version":"0.2.0"}"#,
    );
    let check = check_update(&base_req(&server));
    assert!(
        check
            .error
            .as_deref()
            .unwrap_or("")
            .contains("corrupt metadata"),
        "{:?}",
        check.error
    );

    let server = MockServer::start();
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        r#"{"version":"nope","platforms":{"darwin-aarch64":{"url":"http://x","signature":"aaaa"}}}"#,
    );
    let check = check_update(&base_req(&server));
    assert!(
        check
            .error
            .as_deref()
            .unwrap_or("")
            .contains("corrupt metadata"),
        "{:?}",
        check.error
    );
}

#[test]
fn check_update_below_minimum_is_manual_and_install_skips_artifact() {
    let _guard = env_lock();
    let server = MockServer::start();
    let signature = load_text("artifact-0.2.0.bin.sig");
    let artifact_url = server.url("/artifact-0.2.0.bin");
    let body = manifest_with_policy(
        "0.2.0",
        &artifact_url,
        &signature,
        Some("0.1.3"),
        Some(serde_json::json!(40_446_842_u64)),
    );
    let metadata = serve_json(&server, "/releases/latest/download/latest.json", &body);
    let artifact = serve_artifact(&server, "/artifact-0.2.0.bin", "artifact-0.2.0.bin");
    let artifact_head = server.mock(|when, then| {
        when.method("HEAD").path("/artifact-0.2.0.bin");
        then.status(200).header("content-length", "40446842");
    });
    let mut req = base_req(&server);
    req.current_version = Some("0.1.2".into());

    let check = check_update(&req);
    assert!(check.available);
    assert_eq!(check.install_mode, InstallMode::Manual);
    assert_eq!(check.reason, Some(UpdateReason::BootstrapRequired));
    assert!(!check.restart_required);
    assert!(check.error.is_none());
    assert_eq!(check.size, Some(40_446_842));
    assert_eq!(
        check.release_page,
        "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.0"
    );

    let install = install_update(&InstallRequest {
        confirmed: true,
        check: req,
    });
    assert!(!install.ok);
    assert_eq!(install.install_mode, InstallMode::Manual);
    assert_eq!(install.reason, Some(UpdateReason::BootstrapRequired));
    assert!(!install.restart_required);
    assert!(install.error.is_none());
    assert_eq!(artifact.hits(), 0, "manual policy must not fetch artifact");
    assert_eq!(
        artifact_head.hits(),
        0,
        "manual policy must not probe artifact"
    );
    assert_eq!(metadata.hits(), 2);
}

#[test]
fn check_update_at_minimum_keeps_in_app_install() {
    let _guard = env_lock();
    let server = MockServer::start();
    let signature = load_text("artifact-0.2.0.bin.sig");
    let body = manifest_with_policy(
        "0.2.0",
        &server.url("/artifact-0.2.0.bin"),
        &signature,
        Some("0.1.3"),
        Some(serde_json::json!(1234)),
    );
    serve_json(&server, "/releases/latest/download/latest.json", &body);
    let mut req = base_req(&server);
    req.current_version = Some("0.1.3".into());

    let check = check_update(&req);
    assert!(check.available);
    assert_eq!(check.install_mode, InstallMode::InApp);
    assert_eq!(check.reason, None);
    assert!(check.restart_required);
    assert_eq!(check.size, Some(1234));
}

#[test]
fn check_update_legacy_metadata_keeps_in_app_install() {
    let _guard = env_lock();
    let server = MockServer::start();
    let body = fixture_manifest(
        "0.2.0",
        &server.url("/artifact-0.2.0.bin"),
        &load_text("artifact-0.2.0.bin.sig"),
    );
    serve_json(&server, "/releases/latest/download/latest.json", &body);

    let check = check_update(&base_req(&server));
    assert!(check.available);
    assert_eq!(check.install_mode, InstallMode::InApp);
    assert_eq!(check.reason, None);
}

#[test]
fn plugin_raw_metadata_gate_preserves_legacy_and_enforces_minimum() {
    let legacy = serde_json::json!({"version": "0.2.0"});
    assert_eq!(
        bootstrap_reason_for_metadata("0.1.1", &legacy).unwrap(),
        None
    );

    let policy = serde_json::json!({
        "version": "0.2.0",
        "minimum_updater_version": "0.1.3"
    });
    assert_eq!(
        bootstrap_reason_for_metadata("0.1.2", &policy).unwrap(),
        Some(UpdateReason::BootstrapRequired)
    );
    assert_eq!(
        bootstrap_reason_for_metadata("0.1.3", &policy).unwrap(),
        None
    );

    let invalid = serde_json::json!({
        "version": "0.2.0",
        "minimum_updater_version": "0.1"
    });
    assert!(bootstrap_reason_for_metadata("0.1.1", &invalid).is_err());
}

#[test]
fn check_update_rejects_zero_manifest_size() {
    let _guard = env_lock();
    let server = MockServer::start();
    let body = manifest_with_policy(
        "0.2.0",
        &server.url("/artifact-0.2.0.bin"),
        &load_text("artifact-0.2.0.bin.sig"),
        Some("0.1.3"),
        Some(serde_json::json!(0)),
    );
    serve_json(&server, "/releases/latest/download/latest.json", &body);

    let check = check_update(&base_req(&server));
    assert!(!check.available);
    assert_eq!(check.install_mode, InstallMode::None);
    assert!(
        check
            .error
            .as_deref()
            .unwrap_or("")
            .contains("positive integer"),
        "{:?}",
        check.error
    );
}

#[test]
fn check_update_rejects_invalid_minimum_updater_version() {
    let _guard = env_lock();
    let server = MockServer::start();
    let body = manifest_with_policy(
        "0.2.0",
        &server.url("/artifact-0.2.0.bin"),
        &load_text("artifact-0.2.0.bin.sig"),
        Some("0.1"),
        Some(serde_json::json!(1234)),
    );
    serve_json(&server, "/releases/latest/download/latest.json", &body);

    let check = check_update(&base_req(&server));
    assert!(!check.available);
    assert_eq!(check.install_mode, InstallMode::None);
    assert!(
        check
            .error
            .as_deref()
            .unwrap_or("")
            .contains("invalid minimum_updater_version"),
        "{:?}",
        check.error
    );
}

#[test]
fn check_update_prefers_manifest_size_without_head_request() {
    let _guard = env_lock();
    let server = MockServer::start();
    let artifact_url = server.url("/artifact-0.2.0.bin");
    let body = manifest_with_policy(
        "0.2.0",
        &artifact_url,
        &load_text("artifact-0.2.0.bin.sig"),
        None,
        Some(serde_json::json!(98_765)),
    );
    serve_json(&server, "/releases/latest/download/latest.json", &body);
    let head = server.mock(|when, then| {
        when.method("HEAD").path("/artifact-0.2.0.bin");
        then.status(200).header("content-length", "111");
    });

    let check = check_update(&base_req(&server));
    assert_eq!(check.size, Some(98_765));
    assert_eq!(head.hits(), 0);
}

#[test]
fn check_update_head_fallback_follows_redirect_and_ignores_zero_length() {
    let _guard = env_lock();
    let server = MockServer::start();
    let artifact_url = server.url("/artifact-redirect");
    let body = manifest_with_policy(
        "0.2.0",
        &artifact_url,
        &load_text("artifact-0.2.0.bin.sig"),
        None,
        None,
    );
    serve_json(&server, "/releases/latest/download/latest.json", &body);
    let redirect = server.mock(|when, then| {
        when.method("HEAD").path("/artifact-redirect");
        then.status(302)
            .header("content-length", "0")
            .header("location", "/artifact-final");
    });
    let final_head = server.mock(|when, then| {
        when.method("HEAD").path("/artifact-final");
        then.status(200).header("content-length", "40446842");
    });

    let check = check_update(&base_req(&server));
    assert_eq!(check.size, Some(40_446_842));
    redirect.assert();
    final_head.assert();
}

#[test]
fn check_update_offline_keeps_current_version() {
    let _guard = env_lock();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let req = UpdateRequest {
        current_version: Some(APP_VERSION.to_string()),
        endpoint: Some(format!("http://127.0.0.1:{port}/latest.json")),
        pubkey: Some(pubkey()),
        platform_key: Some("darwin-aarch64".to_string()),
        ..UpdateRequest::default()
    };
    let check = check_update(&req);
    assert!(!check.available);
    assert_eq!(check.current_version, APP_VERSION);
    assert!(
        check.error.as_deref().unwrap_or("").contains("offline"),
        "{:?}",
        check.error
    );
}

#[test]
fn check_update_linux_unsupported_keeps_current_version() {
    let _guard = env_lock();
    let req = UpdateRequest {
        current_version: Some(APP_VERSION.to_string()),
        platform_key: Some("linux-x86_64".to_string()),
        endpoint: Some("http://127.0.0.1:1/latest.json".to_string()),
        ..UpdateRequest::default()
    };
    let check = check_update(&req);
    assert!(!check.available);
    assert_eq!(check.current_version, APP_VERSION);
    assert!(
        check.error.as_deref().unwrap_or("").contains("unsupported"),
        "{:?}",
        check.error
    );
}

#[test]
fn install_update_requires_confirmed() {
    let _guard = env_lock();
    let server = MockServer::start();
    let sig = load_text("artifact-0.2.0.bin.sig");
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest("0.2.0", &server.url("/artifact-0.2.0.bin"), &sig),
    );
    serve_artifact(&server, "/artifact-0.2.0.bin", "artifact-0.2.0.bin");
    let install = install_update(&InstallRequest {
        confirmed: false,
        check: base_req(&server),
    });
    assert!(!install.ok);
    assert_eq!(install.error.as_deref(), Some("confirmation required"));
    assert!(!install.restart_required);
}

#[test]
fn install_update_rejects_signature_error() {
    let _guard = env_lock();
    let server = MockServer::start();
    let wrong = load_text("artifact-0.2.0.bin.sig.wrong");
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest("0.2.0", &server.url("/artifact-0.2.0.bin"), &wrong),
    );
    serve_artifact(&server, "/artifact-0.2.0.bin", "artifact-0.2.0.bin");
    let install = install_update(&InstallRequest {
        confirmed: true,
        check: base_req(&server),
    });
    assert!(!install.ok);
    assert_eq!(install.install_mode, InstallMode::Manual);
    assert_eq!(install.reason, Some(UpdateReason::SignatureKeyMismatch));
    assert_eq!(
        install.release_page,
        "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.0"
    );
    assert!(install.error.is_none());
}

#[test]
fn install_update_other_signature_failure_remains_closed() {
    let _guard = env_lock();
    let server = MockServer::start();
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest(
            "0.2.0",
            &server.url("/artifact-corrupt.bin"),
            &load_text("artifact-0.2.0.bin.sig"),
        ),
    );
    serve_artifact(&server, "/artifact-corrupt.bin", "artifact-0.0.9.bin");

    let install = install_update(&InstallRequest {
        confirmed: true,
        check: base_req(&server),
    });
    assert!(!install.ok);
    assert_eq!(install.install_mode, InstallMode::None);
    assert_eq!(install.reason, None);
    assert_eq!(install.error.as_deref(), Some("update verification failed"));
}

#[test]
fn tauri_unexpected_key_id_maps_to_manual_without_raw_error() {
    let error =
        tauri_plugin_updater::Error::Minisign(updater_minisign_verify::Error::UnexpectedKeyId);
    let install = updater_error_install(&error, Some("0.2.0"));
    assert!(!install.ok);
    assert_eq!(install.install_mode, InstallMode::Manual);
    assert_eq!(install.reason, Some(UpdateReason::SignatureKeyMismatch));
    assert!(install.error.is_none());
}

#[test]
fn tauri_other_signature_error_is_not_misclassified_or_exposed() {
    let error =
        tauri_plugin_updater::Error::Minisign(updater_minisign_verify::Error::InvalidSignature);
    let install = updater_error_install(&error, Some("0.2.0"));
    assert!(!install.ok);
    assert_eq!(install.install_mode, InstallMode::None);
    assert_eq!(install.reason, None);
    assert_eq!(install.error.as_deref(), Some("update verification failed"));
    assert!(!install.error.as_deref().unwrap().contains("signature was"));
}

#[test]
fn install_update_download_interrupt_keeps_current_version() {
    let _guard = env_lock();
    let interrupt = spawn_interrupt_server();
    let server = MockServer::start();
    let sig = load_text("artifact-0.2.0.bin.sig");
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest("0.2.0", &interrupt, &sig),
    );
    let check = check_update(&base_req(&server));
    assert!(check.available, "{:?}", check);
    let install = install_update(&InstallRequest {
        confirmed: true,
        check: base_req(&server),
    });
    assert!(!install.ok);
    assert_eq!(install.error.as_deref(), Some("update download failed"));
    assert_eq!(check.current_version, APP_VERSION);
}

#[test]
fn install_update_failure_keeps_current_version() {
    let _guard = env_lock();
    let server = MockServer::start();
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest(
            "0.2.0",
            &server.url("/artifact-0.2.0.bin"),
            &load_text("artifact-0.2.0.bin.sig.wrong"),
        ),
    );
    serve_artifact(&server, "/artifact-0.2.0.bin", "artifact-0.2.0.bin");
    let before = check_update(&base_req(&server));
    let install = install_update(&InstallRequest {
        confirmed: true,
        check: base_req(&server),
    });
    let after = check_update(&base_req(&server));
    assert!(!install.ok);
    assert_eq!(before.current_version, APP_VERSION);
    assert_eq!(after.current_version, APP_VERSION);
    assert!(!after.restart_required || after.error.is_some() || after.available);
}

#[test]
fn install_update_success_when_confirmed() {
    let _guard = env_lock();
    let server = MockServer::start();
    let sig = load_text("artifact-0.2.0.bin.sig");
    serve_json(
        &server,
        "/releases/latest/download/latest.json",
        &fixture_manifest("0.2.0", &server.url("/artifact-0.2.0.bin"), &sig),
    );
    serve_artifact(&server, "/artifact-0.2.0.bin", "artifact-0.2.0.bin");
    let install = install_update(&InstallRequest {
        confirmed: true,
        check: base_req(&server),
    });
    assert!(install.ok, "{:?}", install.error);
    assert!(install.restart_required);
    assert!(install.error.is_none());
}

#[test]
fn apply_mode_reads_env() {
    let _guard = env_lock();
    std::env::remove_var("KEYSMITH_SWITCH_UPDATER_APPLY");
    assert_eq!(
        keysmith_switch_lib::updater::apply_mode(),
        keysmith_switch_lib::updater::ApplyMode::Real
    );
    std::env::set_var("KEYSMITH_SWITCH_UPDATER_APPLY", "simulate");
    assert_eq!(
        keysmith_switch_lib::updater::apply_mode(),
        keysmith_switch_lib::updater::ApplyMode::Simulate
    );
    std::env::set_var("KEYSMITH_SWITCH_UPDATER_APPLY", "fail");
    assert_eq!(
        keysmith_switch_lib::updater::apply_mode(),
        keysmith_switch_lib::updater::ApplyMode::Fail
    );
    std::env::remove_var("KEYSMITH_SWITCH_UPDATER_APPLY");
}

fn spawn_interrupt_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(false).unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for _ in 0..6 {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 50000\r\nConnection: close\r\n\r\nPARTIAL",
                );
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
    });
    format!("http://127.0.0.1:{}/artifact.bin", addr.port())
}
