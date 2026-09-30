use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::extensions::{
    check_archive, clear_state, install, refresh, state_view, uninstall, ExtError, Fetch,
    IndexPack, Source, OFFICIAL_URL_PREFIX, PACK_TAG,
};
use keysmith_switch_lib::models::{Activation, PromptSort, Scope, ToolKind, ToolStatus};
use keysmith_switch_lib::paths::AppPaths;
use sha2::{Digest, Sha256};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn test_key() -> String {
    std::fs::read_to_string(fixtures().join("extensions/TEST_ONLY.key.pub"))
        .unwrap()
        .trim()
        .to_string()
}

fn wrong_key() -> String {
    std::fs::read_to_string(fixtures().join("updater/TEST_ONLY.wrong.key.pub"))
        .unwrap()
        .trim()
        .to_string()
}

/// A source whose index is the fixture release `name` (v1, v2, v3).
fn source(name: &str) -> Source {
    Source {
        id: "official".into(),
        index_url: format!("{OFFICIAL_URL_PREFIX}test-{name}/index.json"),
        url_prefix: OFFICIAL_URL_PREFIX.into(),
        pubkey: test_key(),
        official: true,
    }
}

/// The network, served from the fixture releases. Anything not served is "offline".
struct Fixture {
    files: RefCell<HashMap<String, Vec<u8>>>,
    up: RefCell<bool>,
    asked: RefCell<Vec<String>>,
}

impl Fixture {
    fn with(releases: &[&str]) -> Self {
        let fixture = Self {
            files: RefCell::new(HashMap::new()),
            up: RefCell::new(true),
            asked: RefCell::new(Vec::new()),
        };
        for name in releases {
            fixture.load(name);
        }
        fixture
    }

    fn load(&self, name: &str) {
        let dir = fixtures().join("extensions").join(name);
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let file = path.file_name().unwrap().to_string_lossy().to_string();
            self.files.borrow_mut().insert(
                format!("{OFFICIAL_URL_PREFIX}test-{name}/{file}"),
                std::fs::read(&path).unwrap(),
            );
        }
    }

    fn set(&self, url: &str, bytes: Vec<u8>) {
        self.files.borrow_mut().insert(url.to_string(), bytes);
    }

    fn url(name: &str, file: &str) -> String {
        format!("{OFFICIAL_URL_PREFIX}test-{name}/{file}")
    }
}

impl Fetch for Fixture {
    fn get(&self, url: &str, max: u64) -> Result<Vec<u8>, String> {
        self.asked.borrow_mut().push(url.to_string());
        if !*self.up.borrow() {
            return Err("could not resolve host".into());
        }
        let bytes = self.files.borrow().get(url).cloned().ok_or("404")?;
        if bytes.len() as u64 > max {
            return Err("too large".into());
        }
        Ok(bytes)
    }
}

fn store() -> (tempfile::TempDir, Store) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();
    (tmp, store)
}

fn prompts(store: &Store, tool: ToolKind) -> Vec<keysmith_switch_lib::models::PromptSummary> {
    store
        .list_prompts(tool, None, None, PromptSort::Title)
        .unwrap()
}

// ----- trust -----------------------------------------------------------------------

#[test]
fn a_verified_index_lists_packs_as_official() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let view = refresh(&store, &fetch, &source("v1"), "zh-CN");
    assert!(view.configured);
    assert_eq!(view.error, None);
    assert_eq!(view.packs.len(), 1);
    let pack = &view.packs[0];
    assert_eq!(pack.id, "fixture.pack");
    assert!(pack.official);
    assert!(pack.compatible);
    assert_eq!(pack.installed_version, None);
    assert_eq!(pack.name, "测试包 fixture.pack");
    assert_eq!(pack.item_count, 2);
}

#[test]
fn a_source_without_a_key_never_touches_the_network() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let mut unconfigured = source("v1");
    unconfigured.pubkey = String::new();
    let view = refresh(&store, &fetch, &unconfigured, "en");
    assert!(!view.configured);
    assert!(view.packs.is_empty());
    assert!(fetch.asked.borrow().is_empty());
    let result = install(&store, &fetch, &unconfigured, "fixture.pack", "en");
    assert!(matches!(result, Err(ExtError::NotConfigured)));
    assert!(fetch.asked.borrow().is_empty());
}

#[test]
fn an_index_signed_by_another_key_is_thrown_away() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let mut other = source("v1");
    other.pubkey = wrong_key();
    let view = refresh(&store, &fetch, &other, "en");
    assert_eq!(view.error.as_deref(), Some("invalid-signature"));
    assert!(view.packs.is_empty());
}

#[test]
fn a_changed_index_fails_its_signature() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let url = Fixture::url("v1", "index.json");
    let mut bytes = fetch.files.borrow()[&url].clone();
    let at = bytes.iter().position(|b| *b == b'0').unwrap();
    bytes[at] = b'9';
    fetch.set(&url, bytes);
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.error.as_deref(), Some("invalid-signature"));
    assert!(view.packs.is_empty());
}

#[test]
fn a_signature_from_another_release_does_not_carry_over() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1", "v2"]);
    let sig = fetch.files.borrow()[&Fixture::url("v2", "index.json.sig")].clone();
    fetch.set(&Fixture::url("v1", "index.json.sig"), sig);
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.error.as_deref(), Some("invalid-signature"));
}

#[test]
fn an_index_from_a_newer_format_is_refused_without_harm() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v3"]);
    let view = refresh(&store, &fetch, &source("v3"), "en");
    assert_eq!(view.error.as_deref(), Some("too-new"));
    assert!(view.packs.is_empty());
}

#[test]
fn being_offline_keeps_the_last_verified_list() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    assert_eq!(refresh(&store, &fetch, &source("v1"), "en").packs.len(), 1);
    *fetch.up.borrow_mut() = false;
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.error.as_deref(), Some("offline"));
    assert_eq!(view.packs.len(), 1);
    // And the cache alone, with no network at all, gives the same list.
    assert_eq!(state_view(&store, &source("v1"), "en").packs.len(), 1);
}

#[test]
fn a_damaged_cache_is_ignored_not_trusted() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    refresh(&store, &fetch, &source("v1"), "en");
    let cache = store
        .paths()
        .home
        .join("extensions/cache/official.index.json");
    let mut bytes = std::fs::read(&cache).unwrap();
    bytes[10] ^= 0x01;
    std::fs::write(&cache, bytes).unwrap();
    assert!(state_view(&store, &source("v1"), "en").packs.is_empty());
}

// ----- archives ----------------------------------------------------------------------

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

fn entry_for(archive: &[u8]) -> IndexPack {
    IndexPack {
        id: "hostile.pack".into(),
        version: "1.0.0".into(),
        min_app_version: "0.2.5".into(),
        name: Default::default(),
        description: Default::default(),
        tools: vec!["claude".into()],
        item_count: 1,
        url: format!("{OFFICIAL_URL_PREFIX}x/hostile.pack-1.0.0.zip"),
        sha256: sha(archive),
        size: archive.len() as u64,
    }
}

fn manifest(file: &str, text: &[u8]) -> Vec<u8> {
    serde_json::json!({
        "schema": 1, "id": "hostile.pack", "version": "1.0.0", "min_app_version": "0.2.5",
        "kind": "prompts", "name": {"en": "H"}, "description": {"en": "H"},
        "tools": ["claude"],
        "items": [{"id": "one", "tool": "claude", "title": {"en": "One"}, "tags": [],
                   "file": file, "sha256": sha(text)}]
    })
    .to_string()
    .into_bytes()
}

#[test]
fn an_honest_archive_loads() {
    let text = b"hello\n";
    let archive = zip_of(&[
        ("pack.json", &manifest("prompts/one.md", text)),
        ("prompts/one.md", text),
    ]);
    let loaded = check_archive(&archive, &entry_for(&archive)).unwrap();
    assert_eq!(loaded.items.len(), 1);
    assert_eq!(loaded.items[0].content, "hello\n");
}

#[test]
fn hostile_archives_are_rejected() {
    let text = b"hello\n";
    let good_manifest = manifest("prompts/one.md", text);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "zip slip",
            zip_of(&[
                ("pack.json", &good_manifest),
                ("prompts/one.md", text),
                ("../evil.md", b"x"),
            ]),
        ),
        (
            "absolute name",
            zip_of(&[
                ("pack.json", &good_manifest),
                ("prompts/one.md", text),
                ("/etc/evil.md", b"x"),
            ]),
        ),
        (
            "unlisted file",
            zip_of(&[
                ("pack.json", &good_manifest),
                ("prompts/one.md", text),
                ("prompts/two.md", b"x"),
            ]),
        ),
        ("missing file", zip_of(&[("pack.json", &good_manifest)])),
        (
            "changed file",
            zip_of(&[
                ("pack.json", &good_manifest),
                ("prompts/one.md", b"other\n"),
            ]),
        ),
        ("no manifest", zip_of(&[("prompts/one.md", text)])),
        (
            "not utf-8",
            zip_of(&[
                ("pack.json", &manifest("prompts/one.md", b"\xff\xfe")),
                ("prompts/one.md", b"\xff\xfe"),
            ]),
        ),
        (
            "empty prompt",
            zip_of(&[
                ("pack.json", &manifest("prompts/one.md", b"  \n")),
                ("prompts/one.md", b"  \n"),
            ]),
        ),
        (
            "path outside prompts",
            zip_of(&[
                ("pack.json", &manifest("other/one.md", text)),
                ("other/one.md", text),
            ]),
        ),
        ("oversized file", {
            let big = vec![b'a'; 256 * 1024 + 1];
            zip_of(&[
                ("pack.json", &manifest("prompts/one.md", &big)),
                ("prompts/one.md", &big),
            ])
        }),
        ("not a zip", b"just some bytes".to_vec()),
    ];
    for (label, archive) in cases {
        let result = check_archive(&archive, &entry_for(&archive));
        assert!(
            matches!(result, Err(ExtError::Invalid(_))),
            "{label}: {result:?}"
        );
    }
}

#[test]
fn an_archive_must_match_the_signed_index_entry() {
    let text = b"hello\n";
    let archive = zip_of(&[
        ("pack.json", &manifest("prompts/one.md", text)),
        ("prompts/one.md", text),
    ]);
    let mut wrong_hash = entry_for(&archive);
    wrong_hash.sha256 = "0".repeat(64);
    assert!(matches!(
        check_archive(&archive, &wrong_hash),
        Err(ExtError::Invalid(_))
    ));
    let mut wrong_size = entry_for(&archive);
    wrong_size.size += 1;
    assert!(matches!(
        check_archive(&archive, &wrong_size),
        Err(ExtError::Invalid(_))
    ));
    let mut wrong_id = entry_for(&archive);
    wrong_id.id = "someone.else".into();
    assert!(matches!(
        check_archive(&archive, &wrong_id),
        Err(ExtError::Invalid(_))
    ));
}

#[test]
fn installing_a_tampered_archive_writes_nothing() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let url = Fixture::url("v1", "fixture.pack-0.1.0.zip");
    let mut bytes = fetch.files.borrow()[&url].clone();
    let last = bytes.len() - 30;
    bytes[last] ^= 0xff;
    fetch.set(&url, bytes);
    let result = install(&store, &fetch, &source("v1"), "fixture.pack", "en");
    assert!(matches!(result, Err(ExtError::Invalid(_))), "{result:?}");
    assert!(prompts(&store, ToolKind::Claude).is_empty());
    assert!(prompts(&store, ToolKind::Codex).is_empty());
}

// ----- installing --------------------------------------------------------------------

#[test]
fn installing_fills_the_library_and_deploys_nothing() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let (view, report) = install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    assert_eq!(
        (report.added, report.updated, report.copied, report.kept),
        (2, 0, 0, 0)
    );
    assert_eq!(view.packs[0].installed_version.as_deref(), Some("0.1.0"));

    let claude = prompts(&store, ToolKind::Claude);
    assert_eq!(claude.len(), 1);
    assert_eq!(claude[0].title, "Alpha");
    assert!(claude[0].tags.contains(&PACK_TAG.to_string()));
    assert!(claude[0].tags.contains(&"fixture".to_string()));
    assert_eq!(
        store.get_prompt(&claude[0].id).unwrap().content,
        "Alpha, first text.\n"
    );
    assert_eq!(prompts(&store, ToolKind::Codex).len(), 1);
    for tool in [ToolKind::Claude, ToolKind::Codex] {
        assert!(
            store.list_activations(tool).unwrap().is_empty(),
            "nothing is deployed"
        );
    }
}

#[test]
fn text_the_library_already_has_is_linked_not_duplicated() {
    let (_tmp, store) = store();
    store
        .insert_prompt(
            "mine",
            ToolKind::Claude,
            "My own",
            "Alpha, first text.\n",
            &[],
            false,
        )
        .unwrap();
    let fetch = Fixture::with(&["v1"]);
    let (_, report) = install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    assert_eq!((report.added, report.linked), (1, 1));
    assert_eq!(prompts(&store, ToolKind::Claude).len(), 1);
}

#[test]
fn installing_twice_changes_nothing() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    let (_, report) = install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    assert_eq!(
        (report.added, report.updated, report.copied, report.kept),
        (0, 0, 0, 2)
    );
    assert_eq!(prompts(&store, ToolKind::Claude).len(), 1);
}

#[test]
fn the_title_follows_the_app_language() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let (view, _) = install(&store, &fetch, &source("v1"), "fixture.pack", "zh-TW").unwrap();
    assert_eq!(view.packs[0].name, "测试包 fixture.pack");
    let en = state_view(&store, &source("v1"), "en");
    assert_eq!(en.packs[0].name, "Fixture fixture.pack");
}

#[test]
fn a_pack_that_needs_a_newer_app_is_listed_but_not_installable() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v2"]);
    let view = refresh(&store, &fetch, &source("v2"), "en");
    let future = view
        .packs
        .iter()
        .find(|p| p.id == "fixture.future")
        .unwrap();
    assert!(!future.compatible);
    assert!(!future.update_available);
    let result = install(&store, &fetch, &source("v2"), "fixture.future", "en");
    assert!(
        matches!(result, Err(ExtError::Incompatible(_))),
        "{result:?}"
    );
    assert!(prompts(&store, ToolKind::Claude).is_empty());
}

#[test]
fn an_unknown_pack_id_is_an_error() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    assert!(matches!(
        install(&store, &fetch, &source("v1"), "no.such.pack", "en"),
        Err(ExtError::Unknown)
    ));
}

// ----- updating ----------------------------------------------------------------------

fn install_v1_then_see_v2() -> (tempfile::TempDir, Store, Fixture) {
    let (tmp, store) = store();
    let fetch = Fixture::with(&["v1", "v2"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    let view = refresh(&store, &fetch, &source("v2"), "en");
    let pack = view.packs.iter().find(|p| p.id == "fixture.pack").unwrap();
    assert!(pack.update_available);
    assert_eq!(view.updates, 1, "the incompatible pack is not an update");
    (tmp, store, fetch)
}

#[test]
fn an_unedited_prompt_follows_the_pack_and_keeps_its_history() {
    let (_tmp, store, fetch) = install_v1_then_see_v2();
    let alpha = prompts(&store, ToolKind::Claude)[0].id.clone();
    let (view, report) = install(&store, &fetch, &source("v2"), "fixture.pack", "en").unwrap();
    assert_eq!(
        (report.updated, report.kept, report.added, report.copied),
        (1, 1, 1, 0)
    );
    assert_eq!(view.updates, 0);
    let now = store.get_prompt(&alpha).unwrap();
    assert_eq!(now.content, "Alpha, second text.\n");
    assert_eq!(now.version, 2);
    let history = store.list_versions(&alpha).unwrap();
    assert_eq!(history.len(), 2, "the earlier text stays in the history");
    assert_eq!(prompts(&store, ToolKind::Claude).len(), 2);
}

#[test]
fn an_edited_prompt_is_never_overwritten() {
    let (_tmp, store, fetch) = install_v1_then_see_v2();
    let alpha = prompts(&store, ToolKind::Claude)[0].id.clone();
    store
        .update_prompt(&alpha, None, Some("My own rules.\n"), None)
        .unwrap();
    let (_, report) = install(&store, &fetch, &source("v2"), "fixture.pack", "en").unwrap();
    assert_eq!((report.copied, report.updated, report.added), (1, 0, 1));
    assert_eq!(store.get_prompt(&alpha).unwrap().content, "My own rules.\n");
    let claude = prompts(&store, ToolKind::Claude);
    assert_eq!(claude.len(), 3);
    let copy = claude
        .iter()
        .find(|p| p.title == "Alpha · 0.2.0")
        .expect("the pack's text is added beside it");
    assert_eq!(
        store.get_prompt(&copy.id).unwrap().content,
        "Alpha, second text.\n"
    );
}

#[test]
fn an_edited_prompt_the_pack_did_not_change_is_simply_kept() {
    let (_tmp, store, fetch) = install_v1_then_see_v2();
    let beta = prompts(&store, ToolKind::Codex)[0].id.clone();
    store
        .update_prompt(&beta, None, Some("Edited beta.\n"), None)
        .unwrap();
    let (_, report) = install(&store, &fetch, &source("v2"), "fixture.pack", "en").unwrap();
    assert_eq!(report.copied, 0);
    assert_eq!(store.get_prompt(&beta).unwrap().content, "Edited beta.\n");
    assert_eq!(prompts(&store, ToolKind::Codex).len(), 1);
}

#[test]
fn a_prompt_the_person_deleted_does_not_come_back() {
    let (_tmp, store, fetch) = install_v1_then_see_v2();
    let alpha = prompts(&store, ToolKind::Claude)[0].id.clone();
    store.soft_delete_prompt(&alpha).unwrap();
    install(&store, &fetch, &source("v2"), "fixture.pack", "en").unwrap();
    let titles: Vec<_> = prompts(&store, ToolKind::Claude)
        .into_iter()
        .map(|p| p.title)
        .collect();
    assert_eq!(titles, vec!["Gamma".to_string()]);
}

// ----- removing ----------------------------------------------------------------------

#[test]
fn uninstalling_removes_only_what_is_safe_to_remove() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v2"]);
    install(&store, &fetch, &source("v2"), "fixture.pack", "en").unwrap();
    let claude = prompts(&store, ToolKind::Claude);
    let alpha = claude
        .iter()
        .find(|p| p.title == "Alpha")
        .unwrap()
        .id
        .clone();
    let gamma = claude
        .iter()
        .find(|p| p.title == "Gamma")
        .unwrap()
        .id
        .clone();
    let beta = prompts(&store, ToolKind::Codex)[0].id.clone();

    // Alpha is running in an agent; beta was edited; gamma is untouched.
    store
        .upsert_activation(&Activation {
            id: "a1".into(),
            prompt_id: Some(alpha.clone()),
            tool: ToolKind::Claude,
            scope: Scope::User,
            project_dir: None,
            status: ToolStatus::Active,
            fingerprint: None,
            operation_id: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        })
        .unwrap();
    store
        .update_prompt(&beta, None, Some("Edited.\n"), None)
        .unwrap();

    let (view, report) = uninstall(&store, &source("v2"), "fixture.pack", "en").unwrap();
    assert_eq!((report.removed, report.kept), (1, 2));
    assert!(store.get_prompt(&gamma).unwrap().deleted_at.is_some());
    assert!(store.get_prompt(&alpha).unwrap().deleted_at.is_none());
    assert!(store.get_prompt(&beta).unwrap().deleted_at.is_none());
    let listed = view.packs.iter().find(|p| p.id == "fixture.pack");
    assert!(listed.map_or(true, |p| p.installed_version.is_none()));
    assert!(matches!(
        uninstall(&store, &source("v2"), "fixture.pack", "en"),
        Err(ExtError::Unknown)
    ));
}

#[test]
fn clearing_all_data_also_forgets_installed_packs() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    store.clear_all().unwrap();
    assert!(!store.paths().home.join("extensions").exists());
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.packs[0].installed_version, None);
}

#[test]
fn a_record_pointing_at_missing_prompts_heals_itself() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    // The prompts are gone (restored from an older backup, say) but the record is not.
    store.clear_all().unwrap();
    std::fs::create_dir_all(store.paths().home.join("extensions")).unwrap();
    std::fs::write(
        store.paths().home.join("extensions/installed.json"),
        r#"{"packs":{"fixture.pack":{"version":"0.1.0","source":"official","installed_at":"x","items":{"alpha":{"prompt_id":"gone","sha256":"x"}}}}}"#,
    )
    .unwrap();
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.packs[0].installed_version, None);
    clear_state(&store);
}

#[test]
fn a_damaged_record_means_nothing_is_installed() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en").unwrap();
    std::fs::write(
        store.paths().home.join("extensions/installed.json"),
        "{ not json",
    )
    .unwrap();
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.packs[0].installed_version, None);
}

#[test]
fn packs_hosted_outside_the_sources_own_address_are_ignored() {
    // Even a signed index cannot send the app to fetch archives from somewhere else.
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let mut pinned = source("v1");
    pinned.url_prefix = "https://elsewhere.example/".into();
    let view = refresh(&store, &fetch, &pinned, "en");
    assert_eq!(view.error, None);
    assert!(view.packs.is_empty());
}

#[test]
fn the_official_source_never_trusts_the_public_test_key() {
    // The test key is committed on purpose, so anyone can sign with it. An app that
    // trusted it would trust everyone.
    assert_ne!(Source::official().pubkey, test_key());
    assert!(Source::official().official);
}
