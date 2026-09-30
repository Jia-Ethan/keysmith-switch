//! Extension packs: signed, data-only bundles of prompts that update on their own,
//! separately from the app.
//!
//! Trust runs one way: a public key compiled into the app verifies `index.json`, the
//! index carries the size and SHA-256 of every archive, and each archive is unpacked by
//! this module and re-checked rule by rule. "Official" is decided here, by which key
//! verified the index; nothing a pack or an index says about itself counts.
//!
//! Packs only ever write to the prompt library. They never deploy anything, and a prompt
//! the person has edited is never overwritten.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::db::markdown::content_sha;
use crate::db::Store;
use crate::error::{Error, Result};
use crate::lock::HomeLock;
use crate::models::{now_rfc3339, PromptSort, ToolKind, ToolStatus, APP_VERSION};
use crate::paths::atomic_write;
use crate::updater::verify_minisign;

pub const OFFICIAL_INDEX_URL: &str =
    "https://github.com/Jia-Ethan/keysmith-switch-extensions/releases/latest/download/index.json";
pub const OFFICIAL_URL_PREFIX: &str =
    "https://github.com/Jia-Ethan/keysmith-switch-extensions/releases/download/";
const OFFICIAL_PUBKEY: &str = include_str!("../extensions/OFFICIAL_PUBKEY.txt");

const SCHEMA: i64 = 1;
const MAX_INDEX_BYTES: u64 = 512 * 1024;
const MAX_SIG_BYTES: u64 = 4 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_PACK_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ITEMS: usize = 200;
const MAX_TAGS: usize = 8;
const MAX_TAG_LEN: usize = 24;
const LOCALES: [&str; 3] = ["zh-CN", "zh-TW", "en"];
const TOOLS: [&str; 4] = ["claude", "codex", "grok", "zcode"];
/// Every prompt a pack adds carries this tag, so they can be found in the library.
pub const PACK_TAG: &str = "extension";

/// Where packs come from and which key vouches for them.
#[derive(Debug, Clone)]
pub struct Source {
    pub id: String,
    pub index_url: String,
    /// Archive URLs must start with this, whatever the (signed) index says.
    pub url_prefix: String,
    pub pubkey: String,
    /// Only the source compiled into the app is official.
    pub official: bool,
}

impl Source {
    pub fn official() -> Self {
        Self {
            id: "official".into(),
            index_url: OFFICIAL_INDEX_URL.into(),
            url_prefix: OFFICIAL_URL_PREFIX.into(),
            pubkey: OFFICIAL_PUBKEY.trim().to_string(),
            official: true,
        }
    }

    pub fn configured(&self) -> bool {
        !self.pubkey.trim().is_empty()
    }
}

/// The network, so tests can stand in for it.
pub trait Fetch {
    fn get(&self, url: &str, max_bytes: u64) -> std::result::Result<Vec<u8>, String>;
}

/// `curl`, like the app updater: present on macOS and on current Windows.
pub struct CurlFetch;

impl Fetch for CurlFetch {
    fn get(&self, url: &str, max_bytes: u64) -> std::result::Result<Vec<u8>, String> {
        let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
        let body = dir.path().join("body");
        let output = Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--fail",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--max-time",
                "30",
                "--max-filesize",
                &max_bytes.to_string(),
                "-o",
                body.to_str().unwrap_or("body"),
                url,
            ])
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let bytes = std::fs::read(&body).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > max_bytes {
            return Err("response is larger than allowed".into());
        }
        Ok(bytes)
    }
}

/// Why something failed, in terms the interface can turn into a sentence.
#[derive(Debug)]
pub enum ExtError {
    /// The network did not answer.
    Offline(String),
    /// The index is not signed by this source's key.
    Signature,
    /// The index is from a newer format than this app understands.
    TooNew,
    /// Something signed but malformed, or an archive that breaks the rules.
    Invalid(String),
    /// The pack needs a newer app.
    Incompatible(String),
    /// The source has no key yet.
    NotConfigured,
    /// The pack is not in the index.
    Unknown,
    Store(Error),
}

impl ExtError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Offline(_) => "offline",
            Self::Signature => "invalid-signature",
            Self::TooNew => "too-new",
            Self::Invalid(_) => "invalid",
            Self::Incompatible(_) => "incompatible",
            Self::NotConfigured => "not-configured",
            Self::Unknown => "unknown-pack",
            Self::Store(_) => "store",
        }
    }
}

impl From<Error> for ExtError {
    fn from(error: Error) -> Self {
        Self::Store(error)
    }
}

impl From<ExtError> for Error {
    fn from(error: ExtError) -> Self {
        let detail = match &error {
            ExtError::Offline(text) | ExtError::Invalid(text) | ExtError::Incompatible(text) => {
                text.clone()
            }
            ExtError::Store(inner) => inner.to_string(),
            _ => String::new(),
        };
        Error::command_failed(format!("extensions:{}:{}", error.code(), detail))
    }
}

type Loc = BTreeMap<String, String>;

#[derive(Debug, Clone)]
pub struct IndexPack {
    pub id: String,
    pub version: String,
    pub min_app_version: String,
    pub name: Loc,
    pub description: Loc,
    pub tools: Vec<String>,
    pub item_count: u32,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct VerifiedIndex {
    pub packs: Vec<IndexPack>,
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static pattern")
}

fn semver(text: &str) -> Option<(u64, u64, u64)> {
    let re = regex(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$");
    let caps = re.captures(text)?;
    Some((
        caps[1].parse().ok()?,
        caps[2].parse().ok()?,
        caps[3].parse().ok()?,
    ))
}

fn loc(value: &Value) -> Option<Loc> {
    let map = value.as_object()?;
    if map.is_empty() {
        return None;
    }
    let mut out = Loc::new();
    for (key, text) in map {
        if !LOCALES.contains(&key.as_str()) {
            return None;
        }
        let text = text.as_str()?.trim();
        if text.is_empty() {
            return None;
        }
        out.insert(key.clone(), text.to_string());
    }
    Some(out)
}

/// One index entry, or `None` when it is not one this app can use (a pack of a kind it
/// has never heard of must not break the ones it can).
fn parse_index_pack(value: &Value, source: &Source) -> Option<IndexPack> {
    let object = value.as_object()?;
    if object.get("kind")?.as_str()? != "prompts" {
        return None;
    }
    let id = object.get("id")?.as_str()?.to_string();
    if !regex(r"^[a-z0-9]+([.-][a-z0-9]+)*$").is_match(&id) || id.len() > 64 {
        return None;
    }
    let version = object.get("version")?.as_str()?.to_string();
    let min_app_version = object.get("min_app_version")?.as_str()?.to_string();
    semver(&version)?;
    semver(&min_app_version)?;
    let url = object.get("url")?.as_str()?.to_string();
    if !url.starts_with(&source.url_prefix) {
        return None;
    }
    let sha256 = object.get("sha256")?.as_str()?.to_string();
    if !regex(r"^[0-9a-f]{64}$").is_match(&sha256) {
        return None;
    }
    let size = object.get("size")?.as_u64()?;
    if size == 0 || size > MAX_ARCHIVE_BYTES {
        return None;
    }
    let tools: Vec<String> = object
        .get("tools")?
        .as_array()?
        .iter()
        .filter_map(|tool| tool.as_str().map(str::to_string))
        .filter(|tool| TOOLS.contains(&tool.as_str()))
        .collect();
    Some(IndexPack {
        id,
        version,
        min_app_version,
        name: loc(object.get("name")?)?,
        description: loc(object.get("description")?)?,
        tools,
        item_count: object.get("item_count")?.as_u64()? as u32,
        url,
        sha256,
        size,
    })
}

/// Fetch and verify the index. Nothing in it is used before the signature checks out.
pub fn fetch_index(
    fetch: &dyn Fetch,
    source: &Source,
) -> std::result::Result<(VerifiedIndex, Vec<u8>, String), ExtError> {
    if !source.configured() {
        return Err(ExtError::NotConfigured);
    }
    let bytes = fetch
        .get(&source.index_url, MAX_INDEX_BYTES)
        .map_err(ExtError::Offline)?;
    let signature = fetch
        .get(&format!("{}.sig", source.index_url), MAX_SIG_BYTES)
        .map_err(ExtError::Offline)?;
    let signature = String::from_utf8(signature).map_err(|_| ExtError::Signature)?;
    let index = verify_index(&bytes, &signature, source)?;
    Ok((index, bytes, signature))
}

pub fn verify_index(
    bytes: &[u8],
    signature: &str,
    source: &Source,
) -> std::result::Result<VerifiedIndex, ExtError> {
    verify_minisign(&source.pubkey, bytes, signature).map_err(|_| ExtError::Signature)?;
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| ExtError::Invalid(format!("index.json: {error}")))?;
    match value.get("schema").and_then(Value::as_i64) {
        Some(SCHEMA) => {}
        Some(other) if other > SCHEMA => return Err(ExtError::TooNew),
        _ => return Err(ExtError::Invalid("index.json: unsupported schema".into())),
    }
    let packs = value
        .get("packs")
        .and_then(Value::as_array)
        .ok_or_else(|| ExtError::Invalid("index.json: packs missing".into()))?
        .iter()
        .filter_map(|entry| parse_index_pack(entry, source))
        .collect::<Vec<_>>();
    let ids: BTreeSet<_> = packs.iter().map(|pack| &pack.id).collect();
    if ids.len() != packs.len() {
        return Err(ExtError::Invalid("index.json: duplicate pack id".into()));
    }
    Ok(VerifiedIndex { packs })
}

fn compatible(pack: &IndexPack) -> bool {
    match (semver(&pack.min_app_version), semver(APP_VERSION)) {
        (Some(needs), Some(have)) => have >= needs,
        _ => false,
    }
}

// ----- archives -------------------------------------------------------------------

#[derive(Debug)]
pub struct PackItem {
    pub id: String,
    pub tool: ToolKind,
    pub title: Loc,
    pub tags: Vec<String>,
    pub content: String,
    pub sha256: String,
}

#[derive(Debug)]
pub struct LoadedPack {
    pub id: String,
    pub version: String,
    pub items: Vec<PackItem>,
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.contains('\0')
        && !name.starts_with('/')
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn invalid<T>(message: impl Into<String>) -> std::result::Result<T, ExtError> {
    Err(ExtError::Invalid(message.into()))
}

/// Open an archive the way a hostile one deserves: check size and hash against the signed
/// index, distrust every name and every declared size, then re-apply the pack rules.
pub fn check_archive(
    bytes: &[u8],
    expected: &IndexPack,
) -> std::result::Result<LoadedPack, ExtError> {
    if bytes.len() as u64 != expected.size {
        return invalid("archive size differs from the index");
    }
    let digest = hex::encode(Sha256::digest(bytes));
    if digest != expected.sha256 {
        return invalid("archive hash differs from the index");
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|error| ExtError::Invalid(format!("not a zip archive: {error}")))?;
    if archive.len() == 0 || archive.len() > MAX_ITEMS + 1 {
        return invalid("archive has the wrong number of entries");
    }
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ExtError::Invalid(format!("unreadable entry: {error}")))?;
        let name = entry.name().to_string();
        if !safe_name(&name) {
            return invalid(format!("unsafe entry name {name:?}"));
        }
        if entry.is_dir() || entry.is_symlink() {
            return invalid(format!("entry {name:?} is a directory or a link"));
        }
        if files.contains_key(&name) {
            return invalid(format!("duplicate entry {name:?}"));
        }
        if entry.size() > MAX_FILE_BYTES {
            return invalid(format!("entry {name:?} is too large"));
        }
        // The declared size is a claim; only what is actually read counts.
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|error| ExtError::Invalid(format!("unreadable entry {name:?}: {error}")))?;
        if data.len() as u64 > MAX_FILE_BYTES {
            return invalid(format!("entry {name:?} is too large"));
        }
        total += data.len() as u64;
        if total > MAX_PACK_BYTES {
            return invalid("archive is too large once unpacked");
        }
        files.insert(name, data);
    }
    let manifest_bytes = files
        .remove("pack.json")
        .ok_or_else(|| ExtError::Invalid("archive has no pack.json".into()))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| ExtError::Invalid(format!("pack.json: {error}")))?;
    let loaded = load_manifest(&manifest, &mut files, expected)?;
    if !files.is_empty() {
        return invalid("archive has files pack.json does not list");
    }
    Ok(loaded)
}

fn load_manifest(
    manifest: &Value,
    files: &mut BTreeMap<String, Vec<u8>>,
    expected: &IndexPack,
) -> std::result::Result<LoadedPack, ExtError> {
    let text = |key: &str| manifest.get(key).and_then(Value::as_str);
    if manifest.get("schema").and_then(Value::as_i64) != Some(SCHEMA) {
        return invalid("pack.json: unsupported schema");
    }
    if text("id") != Some(expected.id.as_str())
        || text("version") != Some(expected.version.as_str())
    {
        return invalid("pack.json does not match the index entry");
    }
    if text("kind") != Some("prompts") {
        return invalid("pack.json: unsupported kind");
    }
    semver(text("min_app_version").unwrap_or(""))
        .ok_or_else(|| ExtError::Invalid("pack.json: bad min_app_version".into()))?;
    if loc(manifest.get("name").unwrap_or(&Value::Null)).is_none()
        || loc(manifest.get("description").unwrap_or(&Value::Null)).is_none()
    {
        return invalid("pack.json: name and description are required");
    }
    let tools: Vec<&str> = manifest
        .get("tools")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let unique: BTreeSet<_> = tools.iter().collect();
    if tools.is_empty() || unique.len() != tools.len() || tools.iter().any(|t| !TOOLS.contains(t)) {
        return invalid("pack.json: bad tools");
    }
    let entries = manifest
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| ExtError::Invalid("pack.json: items missing".into()))?;
    if entries.is_empty() || entries.len() > MAX_ITEMS {
        return invalid("pack.json: wrong number of items");
    }
    let item_id = regex(r"^[a-z0-9]+(-[a-z0-9]+)*$");
    let hash = regex(r"^[0-9a-f]{64}$");
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for entry in entries {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        if !item_id.is_match(id) || id.len() > 64 || !seen.insert(id.to_string()) {
            return invalid(format!("pack.json: bad or duplicate item id {id:?}"));
        }
        let tool = entry.get("tool").and_then(Value::as_str).unwrap_or("");
        if !tools.contains(&tool) {
            return invalid(format!("item {id}: tool is not one of the pack's tools"));
        }
        let title = loc(entry.get("title").unwrap_or(&Value::Null))
            .ok_or_else(|| ExtError::Invalid(format!("item {id}: bad title")))?;
        let tags: Vec<String> = match entry.get("tags") {
            None => Vec::new(),
            Some(Value::Array(list)) => list
                .iter()
                .map(|tag| tag.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| ExtError::Invalid(format!("item {id}: bad tags")))?,
            Some(_) => return invalid(format!("item {id}: bad tags")),
        };
        if tags.len() > MAX_TAGS
            || tags
                .iter()
                .any(|tag| tag.trim().is_empty() || tag.chars().count() > MAX_TAG_LEN)
        {
            return invalid(format!("item {id}: bad tags"));
        }
        let file = entry.get("file").and_then(Value::as_str).unwrap_or("");
        if !safe_name(file) || !file.starts_with("prompts/") || !file.ends_with(".md") {
            return invalid(format!("item {id}: bad file path"));
        }
        let declared = entry.get("sha256").and_then(Value::as_str).unwrap_or("");
        if !hash.is_match(declared) {
            return invalid(format!("item {id}: bad sha256"));
        }
        let data = files.remove(file).ok_or_else(|| {
            ExtError::Invalid(format!("item {id}: {file} is missing or used twice"))
        })?;
        if hex::encode(Sha256::digest(&data)) != declared {
            return invalid(format!("item {id}: file does not match its sha256"));
        }
        let content = String::from_utf8(data)
            .map_err(|_| ExtError::Invalid(format!("item {id}: file is not UTF-8")))?;
        if content.trim().is_empty() {
            return invalid(format!("item {id}: file is empty"));
        }
        items.push(PackItem {
            id: id.to_string(),
            tool: tool
                .parse()
                .map_err(|_| ExtError::Invalid(format!("item {id}: unknown tool")))?,
            title,
            tags,
            content,
            sha256: declared.to_string(),
        });
    }
    Ok(LoadedPack {
        id: expected.id.clone(),
        version: expected.version.clone(),
        items,
    })
}

// ----- what is installed ------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    packs: BTreeMap<String, InstalledPack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledPack {
    version: String,
    source: String,
    installed_at: String,
    items: BTreeMap<String, InstalledItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledItem {
    prompt_id: String,
    /// The hash of the text this app wrote. A prompt whose hash still equals this has not
    /// been edited by the person.
    sha256: String,
}

fn extensions_dir(store: &Store) -> PathBuf {
    store.paths().home.join("extensions")
}

fn state_path(store: &Store) -> PathBuf {
    extensions_dir(store).join("installed.json")
}

fn cache_paths(store: &Store, source: &Source) -> (PathBuf, PathBuf) {
    let dir = extensions_dir(store).join("cache");
    (
        dir.join(format!("{}.index.json", source.id)),
        dir.join(format!("{}.index.json.sig", source.id)),
    )
}

/// A missing or damaged record means "nothing installed", never an error: it only ever
/// describes what could be rebuilt by installing again.
fn load_state(store: &Store) -> State {
    std::fs::read_to_string(state_path(store))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_state(store: &Store, state: &State) -> Result<()> {
    atomic_write(&state_path(store), &serde_json::to_string_pretty(state)?)
}

/// Forget links to prompts that no longer exist (data cleared, restored from an older
/// backup), and packs left with nothing.
fn prune(store: &Store, state: &mut State) {
    for pack in state.packs.values_mut() {
        pack.items
            .retain(|_, item| store.get_prompt(&item.prompt_id).is_ok());
    }
    state.packs.retain(|_, pack| !pack.items.is_empty());
}

/// What a clear of all data must also remove.
pub fn clear_state(store: &Store) {
    let _ = std::fs::remove_dir_all(extensions_dir(store));
}

// ----- the view the interface shows --------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackView {
    pub id: String,
    pub version: String,
    pub min_app_version: String,
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub item_count: u32,
    pub size: u64,
    pub official: bool,
    pub compatible: bool,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsView {
    pub configured: bool,
    pub packs: Vec<PackView>,
    pub updates: u32,
    /// Set when the last look at the network failed; `packs` then come from the cache.
    pub error: Option<String>,
    pub checked_at: Option<String>,
}

fn pick(map: &Loc, language: &str) -> String {
    let order: &[&str] = if language.starts_with("zh-TW") || language.starts_with("zh-HK") {
        &["zh-TW", "zh-CN", "en"]
    } else if language.starts_with("zh") {
        &["zh-CN", "zh-TW", "en"]
    } else {
        &["en", "zh-CN", "zh-TW"]
    };
    order
        .iter()
        .find_map(|key| map.get(*key))
        .or_else(|| map.values().next())
        .cloned()
        .unwrap_or_default()
}

fn build_view(
    store: &Store,
    source: &Source,
    index: Option<&VerifiedIndex>,
    language: &str,
    error: Option<String>,
    checked_at: Option<String>,
) -> ExtensionsView {
    let mut state = load_state(store);
    prune(store, &mut state);
    let mut packs = Vec::new();
    for pack in index.map(|index| index.packs.as_slice()).unwrap_or(&[]) {
        let installed = state.packs.get(&pack.id).map(|item| item.version.clone());
        let newer = match (installed.as_deref().and_then(semver), semver(&pack.version)) {
            (Some(have), Some(offered)) => offered > have,
            _ => false,
        };
        let ok = compatible(pack);
        packs.push(PackView {
            id: pack.id.clone(),
            version: pack.version.clone(),
            min_app_version: pack.min_app_version.clone(),
            name: pick(&pack.name, language),
            description: pick(&pack.description, language),
            tools: pack.tools.clone(),
            item_count: pack.item_count,
            size: pack.size,
            official: source.official,
            compatible: ok,
            installed_version: installed,
            update_available: newer && ok,
        });
    }
    let updates = packs.iter().filter(|pack| pack.update_available).count() as u32;
    ExtensionsView {
        configured: source.configured(),
        packs,
        updates,
        error,
        checked_at,
    }
}

fn load_cached(store: &Store, source: &Source) -> Option<(VerifiedIndex, String)> {
    let (index_path, sig_path) = cache_paths(store, source);
    let bytes = std::fs::read(&index_path).ok()?;
    let signature = std::fs::read_to_string(&sig_path).ok()?;
    let index = verify_index(&bytes, &signature, source).ok()?;
    let stamp = std::fs::metadata(&index_path)
        .and_then(|meta| meta.modified())
        .ok()
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
    Some((index, stamp.unwrap_or_default()))
}

fn write_cache(store: &Store, source: &Source, bytes: &[u8], signature: &str) {
    let (index_path, sig_path) = cache_paths(store, source);
    if let Some(parent) = index_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&index_path, bytes);
    let _ = std::fs::write(&sig_path, signature);
}

/// The last verified answer, without touching the network.
pub fn state_view(store: &Store, source: &Source, language: &str) -> ExtensionsView {
    match load_cached(store, source) {
        Some((index, stamp)) => {
            build_view(store, source, Some(&index), language, None, Some(stamp))
        }
        None => build_view(store, source, None, language, None, None),
    }
}

/// Ask the network. A failure is a view with an error and the cached packs, not an error:
/// being offline must never make the page useless.
pub fn refresh(
    store: &Store,
    fetch: &dyn Fetch,
    source: &Source,
    language: &str,
) -> ExtensionsView {
    if !source.configured() {
        return build_view(store, source, None, language, None, None);
    }
    match fetch_index(fetch, source) {
        Ok((index, bytes, signature)) => {
            write_cache(store, source, &bytes, &signature);
            build_view(
                store,
                source,
                Some(&index),
                language,
                None,
                Some(now_rfc3339()),
            )
        }
        Err(error) => {
            let cached = load_cached(store, source);
            let stamp = cached.as_ref().map(|(_, stamp)| stamp.clone());
            build_view(
                store,
                source,
                cached.as_ref().map(|(index, _)| index),
                language,
                Some(error.code().to_string()),
                stamp,
            )
        }
    }
}

// ----- installing, updating, removing ---------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// New prompts written to the library.
    pub added: u32,
    /// Prompts rewritten in place because the person had not edited them.
    pub updated: u32,
    /// New prompts made beside one the person had edited, which is left alone.
    pub copied: u32,
    /// Prompts the library already had, with identical text, now linked to the pack.
    pub linked: u32,
    /// Prompts left exactly as they were.
    pub kept: u32,
    /// Prompts removed by an uninstall.
    pub removed: u32,
}

/// Install a pack, or bring an installed one up to the indexed version.
pub fn install(
    store: &Store,
    fetch: &dyn Fetch,
    source: &Source,
    pack_id: &str,
    language: &str,
) -> std::result::Result<(ExtensionsView, Report), ExtError> {
    let (index, bytes, signature) = fetch_index(fetch, source)?;
    let entry = index
        .packs
        .iter()
        .find(|pack| pack.id == pack_id)
        .ok_or(ExtError::Unknown)?;
    if !compatible(entry) {
        return Err(ExtError::Incompatible(entry.min_app_version.clone()));
    }
    let archive = fetch
        .get(&entry.url, MAX_ARCHIVE_BYTES)
        .map_err(ExtError::Offline)?;
    let loaded = check_archive(&archive, entry)?;
    write_cache(store, source, &bytes, &signature);

    let _lock = HomeLock::acquire(store.paths())?;
    let mut state = load_state(store);
    prune(store, &mut state);
    let report = apply(store, source, &loaded, &mut state, language)?;
    save_state(store, &state)?;
    let view = build_view(
        store,
        source,
        Some(&index),
        language,
        None,
        Some(now_rfc3339()),
    );
    Ok((view, report))
}

fn apply(
    store: &Store,
    source: &Source,
    pack: &LoadedPack,
    state: &mut State,
    language: &str,
) -> std::result::Result<Report, ExtError> {
    let mut report = Report::default();
    let previous = state.packs.get(&pack.id).cloned();
    let mut items = BTreeMap::new();
    for item in &pack.items {
        let title = pick(&item.title, language);
        let mut tags = item.tags.clone();
        tags.push(PACK_TAG.to_string());
        let sha = content_sha(&item.content);
        let link = previous.as_ref().and_then(|p| p.items.get(&item.id));
        let installed = match link {
            None => install_item(store, item, &title, &tags, &sha, &mut report)?,
            Some(link) => match store.get_prompt(&link.prompt_id) {
                Ok(current) if current.deleted_at.is_none() => {
                    if current.sha256 == link.sha256 {
                        // Not edited: follow the pack.
                        if link.sha256 != sha || current.title != title {
                            store.update_prompt(
                                &current.id,
                                Some(&title),
                                Some(&item.content),
                                Some(&tags),
                            )?;
                            report.updated += 1;
                        } else {
                            report.kept += 1;
                        }
                        InstalledItem {
                            prompt_id: current.id,
                            sha256: sha,
                        }
                    } else if link.sha256 != sha {
                        // Edited by the person and changed upstream: keep theirs, add the new one.
                        let copy_title = format!("{title} · {}", pack.version);
                        let id = uuid::Uuid::new_v4().to_string();
                        store.insert_prompt(
                            &id,
                            item.tool,
                            &copy_title,
                            &item.content,
                            &tags,
                            false,
                        )?;
                        report.copied += 1;
                        InstalledItem {
                            prompt_id: id,
                            sha256: sha,
                        }
                    } else {
                        report.kept += 1;
                        link.clone()
                    }
                }
                // Deleted by the person: never brought back.
                _ => {
                    report.kept += 1;
                    link.clone()
                }
            },
        };
        items.insert(item.id.clone(), installed);
    }
    state.packs.insert(
        pack.id.clone(),
        InstalledPack {
            version: pack.version.clone(),
            source: source.id.clone(),
            installed_at: previous.map(|p| p.installed_at).unwrap_or_else(now_rfc3339),
            items,
        },
    );
    Ok(report)
}

fn install_item(
    store: &Store,
    item: &PackItem,
    title: &str,
    tags: &[String],
    sha: &str,
    report: &mut Report,
) -> std::result::Result<InstalledItem, ExtError> {
    // Text the library already has is reused, so installing twice (or after a restore)
    // does not fill it with duplicates.
    if let Some(existing) = store
        .list_prompts(item.tool, None, None, PromptSort::Updated)?
        .into_iter()
        .find(|prompt| prompt.sha256 == sha)
    {
        report.linked += 1;
        return Ok(InstalledItem {
            prompt_id: existing.id,
            sha256: sha.to_string(),
        });
    }
    let id = uuid::Uuid::new_v4().to_string();
    store.insert_prompt(&id, item.tool, title, &item.content, tags, false)?;
    report.added += 1;
    Ok(InstalledItem {
        prompt_id: id,
        sha256: sha.to_string(),
    })
}

/// Take a pack out. Prompts the person edited, or that an agent is running right now, stay.
pub fn uninstall(
    store: &Store,
    source: &Source,
    pack_id: &str,
    language: &str,
) -> std::result::Result<(ExtensionsView, Report), ExtError> {
    let _lock = HomeLock::acquire(store.paths())?;
    let mut state = load_state(store);
    let pack = state.packs.remove(pack_id).ok_or(ExtError::Unknown)?;
    let mut report = Report::default();
    for link in pack.items.values() {
        let Ok(current) = store.get_prompt(&link.prompt_id) else {
            continue;
        };
        if current.deleted_at.is_some() {
            continue;
        }
        let running = store.list_activations(current.tool)?.iter().any(|a| {
            a.prompt_id.as_deref() == Some(current.id.as_str()) && a.status == ToolStatus::Active
        });
        if current.sha256 == link.sha256 && !running {
            store.soft_delete_prompt(&current.id)?;
            report.removed += 1;
        } else {
            report.kept += 1;
        }
    }
    save_state(store, &state)?;
    Ok((state_view(store, source, language), report))
}
