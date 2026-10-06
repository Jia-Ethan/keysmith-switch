use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::extensions::{
    check_archive, clear_state, hidden_prompt_ids, install, is_hidden_prompt, refresh, state_view,
    uninstall, ExtError, Fetch, IndexPack, PackKind, Source, OFFICIAL_URL_PREFIX, PACK_TAG,
};
use keysmith_switch_lib::models::{
    Activation, PromptSort, Scope, ToolKind, ToolStatus, UpdatePromptInput,
};
use keysmith_switch_lib::paths::AppPaths;
use sha2::{Digest, Sha256};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// A source whose index is the fixture release `name` (v1, v2, v3).
fn source(name: &str) -> Source {
    Source {
        id: "official".into(),
        index_url: format!("{OFFICIAL_URL_PREFIX}test-{name}/index.json"),
        url_prefix: OFFICIAL_URL_PREFIX.into(),
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
fn a_fetched_index_lists_packs_as_official() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let view = refresh(&store, &fetch, &source("v1"), "zh-CN");
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
fn an_index_that_is_not_json_is_refused() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    fetch.set(
        &Fixture::url("v1", "index.json"),
        b"<html>not an index</html>".to_vec(),
    );
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.error.as_deref(), Some("invalid"));
    assert!(view.packs.is_empty());
}

#[test]
fn the_official_source_is_a_fixed_https_address_in_the_publishers_repo() {
    let official = Source::official();
    assert!(official.official);
    assert!(official
        .index_url
        .starts_with("https://github.com/Jia-Ethan/keysmith-switch-extensions/"));
    assert_eq!(official.url_prefix, OFFICIAL_URL_PREFIX);
    assert!(official.url_prefix.starts_with("https://"));
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
    std::fs::write(&cache, "{ not an index").unwrap();
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
        kind: PackKind::Prompts,
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
    let result = install(&store, &fetch, &source("v1"), "fixture.pack", "en", false);
    assert!(matches!(result, Err(ExtError::Invalid(_))), "{result:?}");
    assert!(prompts(&store, ToolKind::Claude).is_empty());
    assert!(prompts(&store, ToolKind::Codex).is_empty());
}

// ----- installing --------------------------------------------------------------------

#[test]
fn installing_fills_the_library_and_deploys_nothing() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    let (view, report) =
        install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
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
fn what_a_pack_wrote_is_locked_but_the_persons_own_text_is_not() {
    let (_tmp, store) = store();
    // The person already had the text of the first claude item: that one stays theirs.
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
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
    assert!(
        !is_hidden_prompt(&store, "mine"),
        "linked text is the person's"
    );
    let codex = prompts(&store, ToolKind::Codex);
    assert_eq!(codex.len(), 1);
    assert!(
        is_hidden_prompt(&store, &codex[0].id),
        "a pack-added prompt is locked"
    );
    assert_eq!(hidden_prompt_ids(&store).len(), 1);

    // Dropping the tag changes nothing: the lock does not come from the tag.
    store
        .update_prompt(&codex[0].id, None, None, Some(&[]))
        .unwrap();
    assert!(is_hidden_prompt(&store, &codex[0].id));
}

#[test]
fn a_locked_prompt_cannot_be_edited_copied_diffed_or_restored() {
    use keysmith_switch_lib::ops;
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
    let id = prompts(&store, ToolKind::Codex)[0].id.clone();

    let edit = ops::update_prompt(
        &store,
        UpdatePromptInput {
            id: id.clone(),
            title: None,
            content: Some("changed".into()),
            tags: None,
        },
    );
    assert!(edit.is_err());
    assert!(ops::copy_prompt(&store, &id, ToolKind::Claude).is_err());
    assert!(ops::restore_prompt_version(&store, &id, 1).is_err());
    assert!(ops::prompt_diff(&store, &id, 1, 1).is_err());
    assert_ne!(store.get_prompt(&id).unwrap().content, "changed");
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
    let (_, report) = install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
    assert_eq!((report.added, report.linked), (1, 1));
    assert_eq!(prompts(&store, ToolKind::Claude).len(), 1);
}

#[test]
fn installing_twice_changes_nothing() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
    let (_, report) = install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
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
    let (view, _) = install(
        &store,
        &fetch,
        &source("v1"),
        "fixture.pack",
        "zh-TW",
        false,
    )
    .unwrap();
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
    let result = install(&store, &fetch, &source("v2"), "fixture.future", "en", false);
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
        install(&store, &fetch, &source("v1"), "no.such.pack", "en", false),
        Err(ExtError::Unknown)
    ));
}

// ----- updating ----------------------------------------------------------------------

fn install_v1_then_see_v2() -> (tempfile::TempDir, Store, Fixture) {
    let (tmp, store) = store();
    let fetch = Fixture::with(&["v1", "v2"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
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
    let (view, report) =
        install(&store, &fetch, &source("v2"), "fixture.pack", "en", false).unwrap();
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
    let (_, report) = install(&store, &fetch, &source("v2"), "fixture.pack", "en", false).unwrap();
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
    let (_, report) = install(&store, &fetch, &source("v2"), "fixture.pack", "en", false).unwrap();
    assert_eq!(report.copied, 0);
    assert_eq!(store.get_prompt(&beta).unwrap().content, "Edited beta.\n");
    assert_eq!(prompts(&store, ToolKind::Codex).len(), 1);
}

#[test]
fn a_prompt_the_person_deleted_does_not_come_back() {
    let (_tmp, store, fetch) = install_v1_then_see_v2();
    let alpha = prompts(&store, ToolKind::Claude)[0].id.clone();
    store.soft_delete_prompt(&alpha).unwrap();
    install(&store, &fetch, &source("v2"), "fixture.pack", "en", false).unwrap();
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
    install(&store, &fetch, &source("v2"), "fixture.pack", "en", false).unwrap();
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
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
    store.clear_all().unwrap();
    assert!(!store.paths().home.join("extensions").exists());
    let view = refresh(&store, &fetch, &source("v1"), "en");
    assert_eq!(view.packs[0].installed_version, None);
}

#[test]
fn a_record_pointing_at_missing_prompts_heals_itself() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&["v1"]);
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
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
    install(&store, &fetch, &source("v1"), "fixture.pack", "en", false).unwrap();
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

// ----- rule packs ------------------------------------------------------------------

/// A release with one rule pack at `version`, served at test-`release`.
fn rule_release(fetch: &Fixture, release: &str, version: &str, rules: serde_json::Value) {
    let rules_bytes = serde_json::to_vec(&serde_json::json!({ "rules": rules })).unwrap();
    let manifest = serde_json::json!({
        "schema": 1, "id": "fixture.rules", "version": version, "min_app_version": "0.2.5",
        "kind": "rules", "name": {"en": "Fixture rules", "zh-CN": "测试规则"},
        "description": {"en": "Rules for tests"}, "tools": ["codex"],
        "rules": {"file": "rules.json", "sha256": sha(&rules_bytes)}
    })
    .to_string();
    let archive = zip_of(&[
        ("pack.json", manifest.as_bytes()),
        ("rules.json", &rules_bytes),
    ]);
    let url = Fixture::url(release, &format!("fixture.rules-{version}.zip"));
    let index = serde_json::json!({
        "schema": 1,
        "packs": [{
            "id": "fixture.rules", "kind": "rules", "version": version, "min_app_version": "0.2.5",
            "name": {"en": "Fixture rules", "zh-CN": "测试规则"}, "description": {"en": "Rules"},
            "tools": ["codex"], "item_count": 0, "url": url, "sha256": sha(&archive),
            "size": archive.len()
        }]
    });
    fetch.set(&url, archive);
    fetch.set(
        &Fixture::url(release, "index.json"),
        serde_json::to_vec(&index).unwrap(),
    );
}

fn pack_table(store: &Store) -> Option<keysmith_switch_lib::db::rules::RuleTable> {
    store
        .list_rule_tables()
        .unwrap()
        .into_iter()
        .find(|table| table.pack_id.as_deref() == Some("fixture.rules"))
}

#[test]
fn a_rule_pack_installs_as_a_read_only_table_and_never_touches_prompts() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&[]);
    rule_release(
        &fetch,
        "r1",
        "0.1.0",
        serde_json::json!([{"from": "a", "to": "b"}]),
    );
    let view = refresh(&store, &fetch, &source("r1"), "en");
    assert_eq!(view.packs.len(), 1);
    assert_eq!(view.packs[0].kind, PackKind::Rules);

    let preview =
        keysmith_switch_lib::extensions::preview_rules(&fetch, &source("r1"), "fixture.rules")
            .unwrap();
    assert_eq!(preview, vec![keysmith_rewrite::Rule::new("a", "b")]);
    assert!(pack_table(&store).is_none(), "preview installs nothing");

    let (view, report) = install(
        &store,
        &fetch,
        &source("r1"),
        "fixture.rules",
        "zh-CN",
        true,
    )
    .unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(view.packs[0].installed_version.as_deref(), Some("0.1.0"));
    let table = pack_table(&store).unwrap();
    assert!(table.enabled);
    assert_eq!(table.title, "测试规则");
    assert!(prompts(&store, ToolKind::Codex).is_empty());
    // The snapshot the relay reads now carries the rule.
    let snapshot =
        std::fs::read(keysmith_switch_lib::rewrite::snapshot_path(store.paths())).unwrap();
    assert!(String::from_utf8_lossy(&snapshot).contains("\"from\": \"a\""));
}

#[test]
fn an_update_to_an_enabled_rule_pack_waits_for_the_person() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&[]);
    rule_release(
        &fetch,
        "r1",
        "0.1.0",
        serde_json::json!([{"from": "a", "to": "1"}]),
    );
    install(&store, &fetch, &source("r1"), "fixture.rules", "en", true).unwrap();

    rule_release(
        &fetch,
        "r2",
        "0.2.0",
        serde_json::json!([{"from": "a", "to": "2"}]),
    );
    refresh(&store, &fetch, &source("r2"), "en");
    keysmith_switch_lib::extensions::follow_rule_packs(&store, &fetch, &source("r2"), "en");
    let table = pack_table(&store).unwrap();
    assert_eq!(table.rules, vec![keysmith_rewrite::Rule::new("a", "1")]);
    assert_eq!(table.pending.as_ref().unwrap().version, "0.2.0");
    // Still recorded at the version in use, so the extensions page keeps offering it.
    let view = state_view(&store, &source("r2"), "en");
    assert_eq!(view.packs[0].installed_version.as_deref(), Some("0.1.0"));

    // Following again does not churn.
    let asked = fetch.asked.borrow().len();
    keysmith_switch_lib::extensions::follow_rule_packs(&store, &fetch, &source("r2"), "en");
    assert_eq!(fetch.asked.borrow().len(), asked);

    store.accept_pack_update(&table.id).unwrap();
    assert_eq!(
        pack_table(&store).unwrap().rules,
        vec![keysmith_rewrite::Rule::new("a", "2")]
    );
}

#[test]
fn an_update_to_a_disabled_rule_pack_follows_silently() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&[]);
    rule_release(
        &fetch,
        "r1",
        "0.1.0",
        serde_json::json!([{"from": "a", "to": "1"}]),
    );
    install(&store, &fetch, &source("r1"), "fixture.rules", "en", false).unwrap();
    assert!(!pack_table(&store).unwrap().enabled);

    rule_release(
        &fetch,
        "r2",
        "0.2.0",
        serde_json::json!([{"from": "a", "to": "2"}]),
    );
    refresh(&store, &fetch, &source("r2"), "en");
    keysmith_switch_lib::extensions::follow_rule_packs(&store, &fetch, &source("r2"), "en");
    let table = pack_table(&store).unwrap();
    assert_eq!(table.rules, vec![keysmith_rewrite::Rule::new("a", "2")]);
    assert!(table.pending.is_none());
    assert!(!table.enabled);
}

#[test]
fn uninstalling_a_rule_pack_removes_its_table() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&[]);
    rule_release(
        &fetch,
        "r1",
        "0.1.0",
        serde_json::json!([{"from": "a", "to": "1"}]),
    );
    install(&store, &fetch, &source("r1"), "fixture.rules", "en", true).unwrap();
    let (view, report) = uninstall(&store, &source("r1"), "fixture.rules", "en").unwrap();
    assert_eq!(report.removed, 1);
    assert!(view.packs[0].installed_version.is_none());
    assert!(pack_table(&store).is_none());
    assert!(store.active_rules().unwrap().is_empty());
}

#[test]
fn hostile_rule_packs_are_rejected() {
    let (_tmp, store) = store();
    for (name, rules) in [
        ("empty-from", serde_json::json!([{"from": "", "to": "x"}])),
        (
            "dupe",
            serde_json::json!([{"from": "a", "to": "1"}, {"from": "a", "to": "2"}]),
        ),
        ("no-rules", serde_json::json!([])),
        ("control", serde_json::json!([{"from": "a\nb", "to": "x"}])),
    ] {
        let fetch = Fixture::with(&[]);
        rule_release(&fetch, name, "0.1.0", rules);
        let error =
            install(&store, &fetch, &source(name), "fixture.rules", "en", true).unwrap_err();
        assert_eq!(error.code(), "invalid", "{name}");
        assert!(pack_table(&store).is_none(), "{name}");
    }
}

#[test]
fn a_rule_pack_for_another_tool_is_not_listed() {
    let (_tmp, store) = store();
    let fetch = Fixture::with(&[]);
    let index = serde_json::json!({
        "schema": 1,
        "packs": [{
            "id": "claude.rules", "kind": "rules", "version": "0.1.0", "min_app_version": "0.2.5",
            "name": {"en": "R"}, "description": {"en": "R"}, "tools": ["claude"], "item_count": 0,
            "url": Fixture::url("rc", "claude.rules-0.1.0.zip"), "sha256": "0".repeat(64), "size": 10
        }]
    });
    fetch.set(
        &Fixture::url("rc", "index.json"),
        serde_json::to_vec(&index).unwrap(),
    );
    assert!(refresh(&store, &fetch, &source("rc"), "en")
        .packs
        .is_empty());
}

#[test]
fn an_old_app_style_index_entry_kind_mismatch_is_refused() {
    // The index says rules but the archive says prompts: refuse rather than guess.
    let (_tmp, store) = store();
    let fetch = Fixture::with(&[]);
    rule_release(
        &fetch,
        "rk",
        "0.1.0",
        serde_json::json!([{"from": "a", "to": "1"}]),
    );
    let url = Fixture::url("rk", "fixture.rules-0.1.0.zip");
    let manifest = serde_json::json!({
        "schema": 1, "id": "fixture.rules", "version": "0.1.0", "min_app_version": "0.2.5",
        "kind": "prompts", "name": {"en": "x"}, "description": {"en": "x"}, "tools": ["codex"],
        "items": []
    })
    .to_string();
    let archive = zip_of(&[("pack.json", manifest.as_bytes())]);
    let mut index: serde_json::Value =
        serde_json::from_slice(&fetch.files.borrow()[&Fixture::url("rk", "index.json")]).unwrap();
    index["packs"][0]["sha256"] = sha(&archive).into();
    index["packs"][0]["size"] = archive.len().into();
    fetch.set(&url, archive);
    fetch.set(
        &Fixture::url("rk", "index.json"),
        serde_json::to_vec(&index).unwrap(),
    );
    let error = install(&store, &fetch, &source("rk"), "fixture.rules", "en", true).unwrap_err();
    assert_eq!(error.code(), "invalid");
}
