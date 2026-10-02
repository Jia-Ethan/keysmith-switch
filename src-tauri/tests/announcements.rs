//! The announcements feed with a stand-in network: nothing here reads the real internet.

use std::cell::RefCell;

use keysmith_switch_lib::announcements::{mark, refresh, state_view, FEED_URL};
use keysmith_switch_lib::db::Store;
use keysmith_switch_lib::extensions::Fetch;
use keysmith_switch_lib::paths::AppPaths;

struct Stub {
    answer: RefCell<Result<Vec<u8>, String>>,
    asked: RefCell<Vec<String>>,
}

impl Fetch for Stub {
    fn get(&self, url: &str, _max_bytes: u64) -> Result<Vec<u8>, String> {
        self.asked.borrow_mut().push(url.to_string());
        self.answer.borrow().clone()
    }
}

fn store() -> (tempfile::TempDir, Store) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_home(tmp.path().join("switch"));
    paths.ensure().unwrap();
    let store = Store::open(&paths).unwrap();
    (tmp, store)
}

fn feed(ids: &[&str]) -> Vec<u8> {
    let items: Vec<_> = ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id, "kind": "news", "publishedAt": "2026-10-02T08:00:00Z",
                "title": { "zh-CN": format!("标题 {id}"), "en": format!("Title {id}") },
                "body": { "zh-CN": "正文" }, "pinned": true, "tools": ["zcode"],
            })
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({ "schema": 1, "announcements": items })).unwrap()
}

#[test]
fn refresh_reads_only_the_feed_keeps_the_last_good_copy_and_remembers_what_was_read() {
    let (_tmp, store) = store();
    assert!(state_view(&store, "zh-CN").items.is_empty());

    let stub = Stub {
        answer: RefCell::new(Ok(feed(&["a", "b"]))),
        asked: RefCell::new(Vec::new()),
    };
    let view = refresh(&store, &stub, "en");
    assert_eq!(stub.asked.borrow().as_slice(), [FEED_URL]);
    assert_eq!((view.items.len(), view.unread), (2, 2));
    assert_eq!(view.items[0].title, "Title a");
    assert!(view.items[0].pinned && view.error.is_none());

    let view = mark(&store, &["a".into()], true, "zh-CN").unwrap();
    assert_eq!(view.unread, 1);
    assert!(view
        .items
        .iter()
        .any(|i| i.id == "a" && i.read && i.dismissed));

    // Offline: the last good copy and the marks stay.
    *stub.answer.borrow_mut() = Err("no network".into());
    let view = refresh(&store, &stub, "zh-CN");
    assert_eq!(view.error.as_deref(), Some("offline"));
    assert_eq!((view.items.len(), view.unread), (2, 1));

    // Garbage: refused, the copy stays.
    *stub.answer.borrow_mut() = Ok(b"<html>not json</html>".to_vec());
    let view = refresh(&store, &stub, "zh-CN");
    assert_eq!(view.error.as_deref(), Some("invalid"));
    assert_eq!(view.items.len(), 2);

    // An announcement taken down disappears, with its marks.
    *stub.answer.borrow_mut() = Ok(feed(&["b"]));
    let view = refresh(&store, &stub, "zh-CN");
    let ids: Vec<&str> = view.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["b"]);

    assert!(mark(&store, &["../x".into()], false, "zh-CN").is_err());
}
