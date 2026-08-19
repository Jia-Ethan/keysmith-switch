use std::io::Write;
use std::net::{Shutdown, TcpListener};
use std::sync::{Mutex, OnceLock};
use std::thread;

use httpmock::prelude::*;
use keysmith_switch_lib::updater::{
    check_update, fixture_manifest, install_update, resolve_update_endpoint, updater_fixture_dir,
    verify_minisign, InstallRequest, UpdateChannel, UpdateRequest, APP_VERSION, BETA_ENDPOINT,
    FIXTURE_PUBKEY, RELEASE_PAGE, STABLE_ENDPOINT,
};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
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
        "http://example.invalid/releases/download/beta-latest/latest.json"
    );
}

#[test]
fn check_update_selects_stable_endpoint_by_default() {
    let server = MockServer::start();
    let artifact = "/artifact-0.2.0.bin";
    let sig = load_text("artifact-0.2.0.bin.sig");
    let body = fixture_manifest("0.2.0", &server.url(artifact), &sig);
    let stable = serve_json(&server, "/releases/latest/download/latest.json", &body);
    let beta = serve_json(
        &server,
        "/releases/download/beta-latest/latest.json",
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
    stable.assert();
    assert_eq!(beta.hits(), 0);
}

#[test]
fn check_update_selects_beta_channel() {
    let server = MockServer::start();
    let sig = load_text("artifact-0.2.0-beta.1.bin.sig");
    let body = fixture_manifest(
        "0.2.0-beta.1",
        &server.url("/artifact-0.2.0-beta.1.bin"),
        &sig,
    );
    let beta = serve_json(&server, "/releases/download/beta-latest/latest.json", &body);
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
    let beta = serve_json(&server, "/releases/download/beta-latest/latest.json", &body);
    std::env::set_var("KEYSMITH_SWITCH_UPDATE_CHANNEL", "beta");
    let check = check_update(&base_req(&server));
    std::env::remove_var("KEYSMITH_SWITCH_UPDATE_CHANNEL");
    assert_eq!(check.channel, UpdateChannel::Beta);
    assert_eq!(check.latest_version.as_deref(), Some("0.2.0-beta.1"));
    beta.assert();
}

#[test]
fn check_update_rejects_downgrade() {
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
fn check_update_offline_keeps_current_version() {
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
    assert_eq!(install.release_page, RELEASE_PAGE);
    assert!(
        install.error.as_deref().unwrap_or("").contains("signature"),
        "{:?}",
        install.error
    );
}

#[test]
fn install_update_download_interrupt_keeps_current_version() {
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
    assert!(
        install.error.as_deref().unwrap_or("").contains("interrupt"),
        "{:?}",
        install.error
    );
    assert_eq!(check.current_version, APP_VERSION);
}

#[test]
fn install_update_failure_keeps_current_version() {
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
