//! App updater policy for Keysmith Switch.
//!
//! Runtime install of a running bundle is performed by `tauri-plugin-updater`.
//! This module parses `latest.json`, enforces channel / platform / semver /
//! signature policy, and never applies an update unless `confirmed=true`.
//!
//! The default pubkey is the **TEST ONLY** fixture key in
//! `fixtures/updater/TEST_ONLY.minisign.key.pub`. Production must replace it
//! via `KEYSMITH_SWITCH_UPDATER_PUBKEY` or a rebuilt `tauri.conf.json`.
//! The matching private key must never be committed or uploaded to GitHub Releases.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(dead_code)]
#[path = "../fixtures/updater/minisign_verify/mod.rs"]
mod minisign_verify;

use minisign_verify::{PublicKey, Signature};

pub const APP_VERSION: &str = "0.1.3";
pub const RELEASE_PAGE: &str = "https://github.com/Jia-Ethan/keysmith-switch-releases/releases";
pub const STABLE_ENDPOINT: &str =
    "https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json";
pub const BETA_ENDPOINT: &str =
    "https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/beta/latest.json";
pub const RELEASES_BASE: &str = "https://github.com/Jia-Ethan/keysmith-switch-releases";

/// TEST ONLY fixture public key (base64 of the minisign `.pub` file).
/// Matches `tauri.conf.json` `plugins.updater.pubkey`. Not a production key.
pub const FIXTURE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQ5M0UyRThDQ0REODc4QkQKUldTOWVOak5qQzQrMlZEMllhUStoZ29ldmY3Yjl2TUVWcTBzOWd6cGxaS2drNnQvKzBscUR4NmgK";

pub fn compiled_updater_pubkey() -> &'static str {
    option_env!("KEYSMITH_SWITCH_UPDATER_PUBKEY").unwrap_or(FIXTURE_PUBKEY)
}

pub const ENV_ENDPOINT: &str = "KEYSMITH_SWITCH_UPDATER_ENDPOINT";
pub const ENV_ENDPOINT_BASE: &str = "KEYSMITH_SWITCH_UPDATER_ENDPOINT_BASE";
pub const ENV_PUBKEY: &str = "KEYSMITH_SWITCH_UPDATER_PUBKEY";
pub const ENV_CHANNEL: &str = "KEYSMITH_SWITCH_UPDATE_CHANNEL";
pub const ENV_PLATFORM: &str = "KEYSMITH_SWITCH_UPDATER_PLATFORM";
pub const ENV_APP_VERSION: &str = "KEYSMITH_SWITCH_APP_VERSION";
pub const ENV_APPLY: &str = "KEYSMITH_SWITCH_UPDATER_APPLY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyMode {
    Real,
    Simulate,
    Fail,
}

pub fn apply_mode() -> ApplyMode {
    match std::env::var(ENV_APPLY).ok().as_deref() {
        Some("simulate") => ApplyMode::Simulate,
        Some("fail") => ApplyMode::Fail,
        _ => ApplyMode::Real,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stable" => Some(Self::Stable),
            "beta" => Some(Self::Beta),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }

    pub fn default_endpoint(self) -> String {
        match self {
            Self::Stable => STABLE_ENDPOINT.to_string(),
            Self::Beta => BETA_ENDPOINT.to_string(),
        }
    }
}

impl Default for UpdateChannel {
    fn default() -> Self {
        Self::Stable
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    pub channel: Option<UpdateChannel>,
    pub current_version: Option<String>,
    pub endpoint: Option<String>,
    pub endpoint_base: Option<String>,
    pub pubkey: Option<String>,
    pub platform_key: Option<String>,
    pub settings_channel: Option<UpdateChannel>,
    pub settings_endpoint_override: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    #[serde(default)]
    pub confirmed: bool,
    #[serde(flatten)]
    pub check: UpdateRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub size: Option<u64>,
    pub channel: UpdateChannel,
    pub restart_required: bool,
    pub progress: Option<f64>,
    pub error: Option<String>,
    pub release_page: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstall {
    pub ok: bool,
    pub restart_required: bool,
    pub error: Option<String>,
    pub release_page: String,
}

#[derive(Debug, Clone)]
struct ResolvedUpdate {
    channel: UpdateChannel,
    current_version: String,
    endpoint: String,
    pubkey: String,
    platform_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeUpdateConfig {
    pub endpoint: String,
    pub pubkey: String,
    pub platform_key: String,
}

#[derive(Debug, Clone)]
struct ParsedManifest {
    version: String,
    notes: Option<String>,
    platforms: HashMap<String, PlatformAsset>,
}

#[derive(Debug, Clone)]
struct PlatformAsset {
    url: String,
    signature: String,
}

#[derive(Debug)]
enum FetchError {
    Offline(String),
    Interrupted,
    Http(u16, String),
    Corrupt(String),
}

impl FetchError {
    fn message(&self) -> String {
        match self {
            FetchError::Offline(e) => format!("offline: {e}"),
            FetchError::Interrupted => "download interrupted".to_string(),
            FetchError::Http(code, body) => format!("http {code}: {body}"),
            FetchError::Corrupt(e) => format!("corrupt metadata: {e}"),
        }
    }
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

#[derive(Clone, Copy)]
enum PlatformSupport {
    Supported,
    Unsupported(&'static str),
}

fn platform_catalog() -> HashMap<&'static str, PlatformSupport> {
    use PlatformSupport::*;
    HashMap::from([
        ("darwin-aarch64", Supported),
        ("darwin-x86_64", Supported),
        ("windows-x86_64", Supported),
        (
            "windows-aarch64",
            Unsupported("Windows ARM64 is not a first-ship desktop target"),
        ),
        (
            "linux-x86_64",
            Unsupported("Linux is not a first-ship desktop target"),
        ),
        (
            "linux-aarch64",
            Unsupported("Linux is not a first-ship desktop target"),
        ),
        (
            "linux-i686",
            Unsupported("Linux is not a first-ship desktop target"),
        ),
        (
            "linux-armv7",
            Unsupported("Linux is not a first-ship desktop target"),
        ),
    ])
}

pub fn current_platform_key() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let os = match os {
        "macos" => "darwin",
        other => other,
    };
    format!("{os}-{arch}")
}

pub fn updater_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/updater")
}

pub fn resolve_update_endpoint(
    channel: UpdateChannel,
    endpoint_override: Option<&str>,
    endpoint_base: Option<&str>,
) -> String {
    if let Some(ep) = nonempty(endpoint_override) {
        return ep.to_string();
    }
    if let Some(base) = nonempty(endpoint_base) {
        let suffix = match channel {
            UpdateChannel::Stable => "/releases/latest/download/latest.json",
            UpdateChannel::Beta => "/beta/latest.json",
        };
        return format!("{}{suffix}", trim_slash(base));
    }
    channel.default_endpoint()
}

pub fn runtime_update_config(req: &UpdateRequest) -> RuntimeUpdateConfig {
    let resolved = resolve_request(req);
    RuntimeUpdateConfig {
        endpoint: resolved.endpoint,
        pubkey: resolved.pubkey,
        platform_key: resolved.platform_key,
    }
}

pub fn check_update(req: &UpdateRequest) -> UpdateCheck {
    let resolved = resolve_request(req);
    if let Some(reason) = unsupported_reason(&resolved.platform_key) {
        return keep_current(
            &resolved,
            None,
            Some(format!(
                "platform {} is unsupported ({reason})",
                resolved.platform_key
            )),
        );
    }
    match load_manifest(&resolved) {
        Ok((manifest, asset, cmp)) => finish_check(&resolved, manifest, asset, cmp),
        Err(err) => keep_current(&resolved, None, Some(err)),
    }
}

pub fn install_update(req: &InstallRequest) -> UpdateInstall {
    if !req.confirmed {
        return UpdateInstall {
            ok: false,
            restart_required: false,
            error: Some("confirmation required".to_string()),
            release_page: RELEASE_PAGE.to_string(),
        };
    }
    let check = check_update(&req.check);
    if let Some(err) = check.error.as_deref() {
        return UpdateInstall {
            ok: false,
            restart_required: false,
            error: Some(err.to_string()),
            release_page: RELEASE_PAGE.to_string(),
        };
    }
    if !check.available {
        return UpdateInstall {
            ok: false,
            restart_required: false,
            error: Some("no update available".to_string()),
            release_page: RELEASE_PAGE.to_string(),
        };
    }
    let resolved = resolve_request(&req.check);
    match download_and_verify(&resolved) {
        Ok(()) => UpdateInstall {
            ok: true,
            restart_required: true,
            error: None,
            release_page: RELEASE_PAGE.to_string(),
        },
        Err(err) => UpdateInstall {
            ok: false,
            restart_required: false,
            error: Some(err),
            release_page: RELEASE_PAGE.to_string(),
        },
    }
}

pub fn verify_minisign(pubkey: &str, data: &[u8], signature_field: &str) -> Result<(), String> {
    let pk = parse_pubkey(pubkey)?;
    let sig = parse_signature(signature_field)?;
    pk.verify(data, &sig, false)
        .map_err(|e| format!("signature verification failed: {e}"))
}

pub fn fixture_manifest(version: &str, url: &str, signature: &str) -> String {
    let platforms = serde_json::json!({
        "darwin-aarch64": { "url": url, "signature": signature },
        "darwin-x86_64": { "url": url, "signature": signature },
        "windows-x86_64": { "url": url, "signature": signature },
        "linux-x86_64": { "url": url, "signature": signature },
        "linux-aarch64": { "url": url, "signature": signature },
    });
    serde_json::json!({
        "version": version,
        "notes": format!("Keysmith Switch {version}"),
        "pub_date": "2026-08-19T00:00:00Z",
        "platforms": platforms,
    })
    .to_string()
}

fn finish_check(
    resolved: &ResolvedUpdate,
    manifest: ParsedManifest,
    asset: PlatformAsset,
    cmp: std::cmp::Ordering,
) -> UpdateCheck {
    match cmp {
        std::cmp::Ordering::Less => keep_current(
            resolved,
            Some(manifest.version.clone()),
            Some(format!(
                "downgrade rejected: {} is not newer than {}",
                manifest.version, resolved.current_version
            )),
        ),
        std::cmp::Ordering::Equal => UpdateCheck {
            available: false,
            current_version: resolved.current_version.clone(),
            latest_version: Some(manifest.version),
            notes: manifest.notes,
            size: None,
            channel: resolved.channel,
            restart_required: false,
            progress: None,
            error: None,
            release_page: RELEASE_PAGE.to_string(),
        },
        std::cmp::Ordering::Greater => {
            let size = head_size(&asset.url);
            UpdateCheck {
                available: true,
                current_version: resolved.current_version.clone(),
                latest_version: Some(manifest.version),
                notes: manifest.notes,
                size,
                channel: resolved.channel,
                restart_required: true,
                progress: None,
                error: None,
                release_page: RELEASE_PAGE.to_string(),
            }
        }
    }
}

fn load_manifest(
    resolved: &ResolvedUpdate,
) -> Result<(ParsedManifest, PlatformAsset, std::cmp::Ordering), String> {
    parse_pubkey(&resolved.pubkey)?;
    let fetched = fetch_url(&resolved.endpoint).map_err(|e| e.message())?;
    if !(200..300).contains(&fetched.status) {
        return Err(format!(
            "http {}: {}",
            fetched.status,
            String::from_utf8_lossy(&fetched.body)
        ));
    }
    let text = String::from_utf8(fetched.body)
        .map_err(|_| "corrupt metadata: latest.json is not utf-8".to_string())?;
    let manifest = parse_latest_json(&text)?;
    let asset = manifest
        .platforms
        .get(&resolved.platform_key)
        .cloned()
        .ok_or_else(|| {
            format!(
                "corrupt metadata: missing platform {}",
                resolved.platform_key
            )
        })?;
    parse_signature(&asset.signature)?;
    let cmp = compare_semver(&manifest.version, &resolved.current_version)?;
    Ok((manifest, asset, cmp))
}

fn download_and_verify(resolved: &ResolvedUpdate) -> Result<(), String> {
    let (manifest, asset, cmp) = load_manifest(resolved)?;
    if cmp != std::cmp::Ordering::Greater {
        return Err(if cmp == std::cmp::Ordering::Less {
            format!(
                "downgrade rejected: {} is not newer than {}",
                manifest.version, resolved.current_version
            )
        } else {
            "no update available".to_string()
        });
    }
    let artifact = fetch_url(&asset.url).map_err(|e| e.message())?;
    if !(200..300).contains(&artifact.status) {
        return Err(format!("download failed: http {}", artifact.status));
    }
    verify_minisign(&resolved.pubkey, &artifact.body, &asset.signature)?;
    let _ = manifest;
    Ok(())
}

fn keep_current(
    resolved: &ResolvedUpdate,
    latest: Option<String>,
    error: Option<String>,
) -> UpdateCheck {
    UpdateCheck {
        available: false,
        current_version: resolved.current_version.clone(),
        latest_version: latest,
        notes: None,
        size: None,
        channel: resolved.channel,
        restart_required: false,
        progress: None,
        error,
        release_page: RELEASE_PAGE.to_string(),
    }
}

fn resolve_request(req: &UpdateRequest) -> ResolvedUpdate {
    let channel = req
        .channel
        .or_else(|| env_nonempty(ENV_CHANNEL).and_then(|v| UpdateChannel::parse(&v)))
        .or(req.settings_channel)
        .unwrap_or(UpdateChannel::Stable);
    let full_override = nonempty(req.endpoint.as_deref())
        .map(|s| s.to_string())
        .or_else(|| env_nonempty(ENV_ENDPOINT))
        .or_else(|| nonempty(req.settings_endpoint_override.as_deref()).map(|s| s.to_string()));
    let base = nonempty(req.endpoint_base.as_deref())
        .map(|s| s.to_string())
        .or_else(|| env_nonempty(ENV_ENDPOINT_BASE));
    let endpoint = resolve_update_endpoint(channel, full_override.as_deref(), base.as_deref());
    let pubkey = req
        .pubkey
        .clone()
        .or_else(|| env_nonempty(ENV_PUBKEY))
        .unwrap_or_else(|| compiled_updater_pubkey().to_string());
    let platform_key = req
        .platform_key
        .clone()
        .or_else(|| env_nonempty(ENV_PLATFORM))
        .unwrap_or_else(current_platform_key);
    let current_version = req
        .current_version
        .clone()
        .or_else(|| env_nonempty(ENV_APP_VERSION))
        .unwrap_or_else(|| APP_VERSION.to_string());
    ResolvedUpdate {
        channel,
        current_version: strip_v(&current_version).to_string(),
        endpoint,
        pubkey,
        platform_key,
    }
}

fn unsupported_reason(platform_key: &str) -> Option<&'static str> {
    match platform_catalog().get(platform_key) {
        Some(PlatformSupport::Unsupported(reason)) => Some(*reason),
        Some(PlatformSupport::Supported) => None,
        None => None,
    }
}

fn parse_latest_json(text: &str) -> Result<ParsedManifest, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("corrupt metadata: invalid json ({e})"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "corrupt metadata: root must be an object".to_string())?;
    let version = obj
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "corrupt metadata: missing version".to_string())?;
    let version = strip_v(version).to_string();
    if parse_semver(&version).is_err() {
        return Err(format!("corrupt metadata: invalid semver {version}"));
    }
    if let Some(pub_date) = obj.get("pub_date") {
        if !pub_date.is_null() {
            let raw = pub_date
                .as_str()
                .ok_or_else(|| "corrupt metadata: pub_date must be a string".to_string())?;
            chrono::DateTime::parse_from_rfc3339(raw)
                .map_err(|_| "corrupt metadata: pub_date must be RFC 3339".to_string())?;
        }
    }
    let platforms_value = obj
        .get("platforms")
        .ok_or_else(|| "corrupt metadata: missing platforms".to_string())?;
    let platforms_obj = platforms_value
        .as_object()
        .ok_or_else(|| "corrupt metadata: platforms must be an object".to_string())?;
    if platforms_obj.is_empty() {
        return Err("corrupt metadata: platforms is empty".to_string());
    }
    let mut platforms = HashMap::new();
    for (key, entry) in platforms_obj {
        let asset_obj = entry
            .as_object()
            .ok_or_else(|| format!("corrupt metadata: platform {key} must be an object"))?;
        let url = asset_obj
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("corrupt metadata: platform {key} missing url"))?;
        let signature = asset_obj
            .get("signature")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("corrupt metadata: platform {key} missing signature"))?;
        if url.trim().is_empty() {
            return Err(format!("corrupt metadata: platform {key} has empty url"));
        }
        if signature.trim().is_empty() {
            return Err(format!(
                "corrupt metadata: platform {key} has empty signature"
            ));
        }
        platforms.insert(
            key.clone(),
            PlatformAsset {
                url: url.to_string(),
                signature: signature.to_string(),
            },
        );
    }
    let notes = obj
        .get("notes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(ParsedManifest {
        version,
        notes,
        platforms,
    })
}

fn parse_pubkey(input: &str) -> Result<PublicKey, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("missing updater pubkey".to_string());
    }
    if trimmed.contains("untrusted comment:") || trimmed.contains("minisign public key") {
        return PublicKey::decode(trimmed).map_err(|e| format!("invalid pubkey: {e}"));
    }
    if let Ok(bytes) = minisign_verify::decode_base64(trimmed) {
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            let text = text.trim().to_string();
            if text.contains("untrusted comment:") || text.contains("minisign public key") {
                return PublicKey::decode(&text).map_err(|e| format!("invalid pubkey: {e}"));
            }
            if let Ok(pk) = PublicKey::from_base64(text.trim()) {
                return Ok(pk);
            }
        }
        if bytes.len() == 42 {
            let b64 = trimmed.lines().last().unwrap_or(trimmed).trim().to_string();
            if let Ok(pk) = PublicKey::from_base64(&b64) {
                return Ok(pk);
            }
        }
    }
    PublicKey::from_base64(trimmed).map_err(|e| format!("invalid pubkey: {e}"))
}

fn parse_signature(field: &str) -> Result<Signature, String> {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return Err("corrupt metadata: empty signature".to_string());
    }
    if trimmed.contains("untrusted comment:") {
        return Signature::decode(trimmed)
            .map_err(|e| format!("corrupt metadata: signature ({e})"));
    }
    if let Ok(bytes) = minisign_verify::decode_base64(trimmed) {
        if let Ok(text) = String::from_utf8(bytes) {
            let text = text.trim().to_string();
            if text.contains("untrusted comment:") {
                return Signature::decode(&text)
                    .map_err(|e| format!("corrupt metadata: signature ({e})"));
            }
        }
    }
    Signature::decode(trimmed).map_err(|e| format!("corrupt metadata: signature ({e})"))
}

fn fetch_url(url: &str) -> Result<HttpResponse, FetchError> {
    parse_http_url(url).map_err(FetchError::Corrupt)?;
    curl_get(url)
}

fn parse_http_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    let rest = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else {
        return Err("unsupported url scheme".to_string());
    };
    let hostport = rest.split_once('/').map(|(h, _)| h).unwrap_or(rest);
    if hostport.is_empty() || hostport.starts_with('[') {
        return Err("invalid host".to_string());
    }
    if let Some((_, port)) = hostport.rsplit_once(':') {
        let _: u16 = port.parse().map_err(|_| "invalid port".to_string())?;
    }
    Ok(())
}

fn curl_get(url: &str) -> Result<HttpResponse, FetchError> {
    let dir = tempfile::tempdir().map_err(|e| FetchError::Offline(e.to_string()))?;
    let header_path = dir.path().join("headers");
    let body_path = dir.path().join("body");
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "--noproxy",
            "*",
            "--http1.1",
            "--max-time",
            "20",
            "-D",
            header_path.to_str().unwrap_or("headers"),
            "-o",
            body_path.to_str().unwrap_or("body"),
            "-w",
            "%{http_code}",
            url,
        ])
        .output()
        .map_err(|e| FetchError::Offline(e.to_string()))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_l = stderr.to_ascii_lowercase();
    if is_interrupt_error(output.status.code(), &stderr_l) {
        return Err(FetchError::Interrupted);
    }
    if !output.status.success() && is_offline_error(&stderr_l) {
        return Err(FetchError::Offline(stderr.trim().to_string()));
    }
    let status: u16 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    let header_text = std::fs::read_to_string(&header_path).unwrap_or_default();
    let body = std::fs::read(&body_path).unwrap_or_default();
    let mut headers = HashMap::new();
    for line in header_text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    if let Some(cl) = headers.get("content-length") {
        if let Ok(n) = cl.parse::<usize>() {
            if body.len() < n {
                return Err(FetchError::Interrupted);
            }
        }
    }
    if status == 0 && body.is_empty() {
        return Err(FetchError::Offline(if stderr.trim().is_empty() {
            "empty response".to_string()
        } else {
            stderr.trim().to_string()
        }));
    }
    if !(200..300).contains(&status) {
        return Err(FetchError::Http(
            status,
            String::from_utf8_lossy(&body).chars().take(180).collect(),
        ));
    }
    Ok(HttpResponse { status, body })
}

fn is_interrupt_error(code: Option<i32>, stderr: &str) -> bool {
    code == Some(18)
        || stderr.contains("transfer closed")
        || stderr.contains("bytes remaining")
        || stderr.contains("unexpected eof")
}

fn is_offline_error(stderr: &str) -> bool {
    stderr.contains("connect")
        || stderr.contains("resolve")
        || stderr.contains("timed out")
        || stderr.contains("timeout")
        || stderr.contains("could not")
        || stderr.contains("couldn't")
        || stderr.contains("refused")
        || stderr.contains("unreachable")
}

fn head_size(url: &str) -> Option<u64> {
    let output = Command::new("curl")
        .args([
            "-sS",
            "-I",
            "--noproxy",
            "*",
            "--http1.1",
            "--max-time",
            "10",
            "-w",
            "\n%{http_code}",
            url,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            return v.trim().parse().ok();
        }
    }
    None
}

fn compare_semver(left: &str, right: &str) -> Result<std::cmp::Ordering, String> {
    let l = parse_semver(left)?;
    let r = parse_semver(right)?;
    Ok(l.cmp(&r))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<PrePart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrePart {
    Num(u64),
    Id(String),
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (
            self.major.cmp(&other.major),
            self.minor.cmp(&other.minor),
            self.patch.cmp(&other.patch),
        ) {
            (Equal, Equal, Equal) => match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Equal,
                (true, false) => Greater,
                (false, true) => Less,
                (false, false) => cmp_pre(&self.pre, &other.pre),
            },
            (Equal, Equal, patch) => patch,
            (Equal, minor, _) => minor,
            (major, _, _) => major,
        }
    }
}

impl PartialOrd for PrePart {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrePart {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (PrePart::Num(a), PrePart::Num(b)) => a.cmp(b),
            (PrePart::Num(_), PrePart::Id(_)) => std::cmp::Ordering::Less,
            (PrePart::Id(_), PrePart::Num(_)) => std::cmp::Ordering::Greater,
            (PrePart::Id(a), PrePart::Id(b)) => a.cmp(b),
        }
    }
}

fn cmp_pre(a: &[PrePart], b: &[PrePart]) -> std::cmp::Ordering {
    for (x, y) in a.iter().zip(b.iter()) {
        let c = x.cmp(y);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
    }
    a.len().cmp(&b.len())
}

fn parse_semver(raw: &str) -> Result<SemVer, String> {
    let raw = strip_v(raw);
    let (core, pre) = match raw.split_once('-') {
        Some((core, pre)) => (core, Some(pre.split('+').next().unwrap_or(pre))),
        None => (raw.split('+').next().unwrap_or(raw), None),
    };
    let mut parts = core.split('.');
    let major = parts
        .next()
        .ok_or_else(|| format!("invalid semver {raw}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid semver {raw}"))?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| format!("invalid semver {raw}"))?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| format!("invalid semver {raw}"))?;
    if parts.next().is_some() {
        return Err(format!("invalid semver {raw}"));
    }
    let pre = match pre {
        Some(p) if !p.is_empty() => p
            .split('.')
            .map(|id| {
                if let Ok(n) = id.parse::<u64>() {
                    PrePart::Num(n)
                } else {
                    PrePart::Id(id.to_string())
                }
            })
            .collect(),
        _ => Vec::new(),
    };
    Ok(SemVer {
        major,
        minor,
        patch,
        pre,
    })
}

fn strip_v(raw: &str) -> &str {
    raw.trim().strip_prefix('v').unwrap_or(raw.trim())
}

fn env_nonempty(name: &str) -> Option<String> {
    if matches!(
        name,
        "GITHUB_TOKEN"
            | "GH_TOKEN"
            | "TAURI_SIGNING_PRIVATE_KEY"
            | "TAURI_SIGNING_PRIVATE_KEY_PATH"
    ) {
        return None;
    }
    std::env::var(name).ok().and_then(|v| {
        let t = v.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

fn nonempty(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

fn trim_slash(base: &str) -> &str {
    base.trim().trim_end_matches('/')
}
