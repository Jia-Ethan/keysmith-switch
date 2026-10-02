//! Announcements: short notes the publisher posts without shipping a release (what this version
//! does, what comes next, how an agent signs in).
//!
//! The feed is one JSON file on the `announcements` branch of the public releases repository.
//! It is read over HTTPS only, from that one address, and is size-capped. Every field is checked
//! here: text is plain (no HTML is ever rendered), links must be HTTPS on a short allow-list, and
//! anything malformed is dropped rather than shown. A feed that cannot be read leaves the last
//! good copy in place, and the app works the same without one.
//!
//! What the person has read or closed is kept locally, by announcement id.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::Store;
use crate::error::{Error, Result};
use crate::extensions::Fetch;
use crate::models::{ToolKind, APP_VERSION};

pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/announcements/announcements.json";
const MAX_FEED_BYTES: u64 = 256 * 1024;
const MAX_ITEMS: usize = 50;
const MAX_TITLE_CHARS: usize = 120;
const MAX_BODY_CHARS: usize = 4000;
/// The newest feed format this app understands.
const FEED_SCHEMA: u64 = 1;
/// Links in an announcement may only open these sites: the project's own repositories and the
/// agents' official sites.
const LINK_HOSTS: [&str; 5] = ["github.com", "anthropic.com", "openai.com", "x.ai", "z.ai"];
const LANGUAGES: [&str; 3] = ["zh-CN", "en", "zh-TW"];

/// Text in each language the publisher wrote; a missing one falls back to Simplified Chinese.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Localized {
    #[serde(rename = "zh-CN")]
    pub zh_cn: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub en: Option<String>,
    #[serde(rename = "zh-TW", default, skip_serializing_if = "Option::is_none")]
    pub zh_tw: Option<String>,
}

impl Localized {
    pub fn pick(&self, language: &str) -> &str {
        let chosen = match language {
            "en" => self.en.as_deref(),
            "zh-TW" => self.zh_tw.as_deref(),
            _ => None,
        };
        chosen
            .filter(|text| !text.is_empty())
            .unwrap_or(&self.zh_cn)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    /// `news` (general), `release` (what a version brings) or `preview` (what comes next).
    pub kind: String,
    pub published_at: String,
    pub title: Localized,
    pub body: Localized,
    /// Shown as a card at the top of the agent pages until closed.
    #[serde(default)]
    pub pinned: bool,
    /// Only shown on these agents' pages; empty means everywhere.
    #[serde(default)]
    pub tools: Vec<ToolKind>,
    /// Only shown to apps in this version range (inclusive); either end may be missing.
    #[serde(default)]
    pub min_app_version: Option<String>,
    #[serde(default)]
    pub max_app_version: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
}

/// One announcement ready for the interface: text in the person's language, and its local state.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementView {
    pub id: String,
    pub kind: String,
    pub published_at: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub tools: Vec<ToolKind>,
    pub link: Option<String>,
    pub read: bool,
    pub dismissed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementsView {
    pub items: Vec<AnnouncementView>,
    pub unread: usize,
    /// When the feed was last read successfully, if ever.
    pub fetched_at: Option<String>,
    /// Why the last look failed (`offline`, `invalid`, `too-new`), when it did.
    pub error: Option<String>,
}

/// What is kept on disk: the last good feed, and what the person has read or closed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Local {
    #[serde(default)]
    items: Vec<Announcement>,
    #[serde(default)]
    fetched_at: Option<String>,
    #[serde(default)]
    read: BTreeSet<String>,
    #[serde(default)]
    dismissed: BTreeSet<String>,
}

fn local_path(store: &Store) -> PathBuf {
    store.paths().home.join("announcements.json")
}

fn load(store: &Store) -> Local {
    std::fs::read(local_path(store))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(store: &Store, local: &Local) -> Result<()> {
    let path = local_path(store);
    let tmp = path.with_extension("json.writing");
    std::fs::write(&tmp, serde_json::to_vec_pretty(local)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

fn semver(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.');
    let parsed = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(parsed)
}

/// An id is short and plain, so it is safe to keep and to use as a key.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// HTTPS on an allowed host (or one of its subdomains), with no credentials in it.
fn valid_link(link: &str) -> bool {
    let Ok(url) = url::Url::parse(link) else {
        return false;
    };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    LINK_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
}

/// Characters that change how text is laid out without showing anything: direction overrides
/// and isolates (which can make text read differently from what it is), zero-width marks and
/// joiners, and the byte-order mark.
fn invisible_format(c: char) -> bool {
    matches!(c,
        '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}')
}

/// Plain text only: control and invisible format characters are removed; line breaks and tabs
/// stay.
fn clean_text(text: &str, max_chars: usize) -> Option<String> {
    let cleaned: String = text
        .chars()
        .filter(|c| (!c.is_control() || matches!(c, '\n' | '\t')) && !invisible_format(*c))
        .collect();
    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty() && cleaned.chars().count() <= max_chars).then_some(cleaned)
}

fn clean_localized(value: &Localized, max_chars: usize) -> Option<Localized> {
    let optional = |text: &Option<String>| match text {
        Some(text) if !text.trim().is_empty() => clean_text(text, max_chars).map(Some),
        _ => Some(None),
    };
    Some(Localized {
        zh_cn: clean_text(&value.zh_cn, max_chars)?,
        en: optional(&value.en)?,
        zh_tw: optional(&value.zh_tw)?,
    })
}

/// One item as the publisher wrote it, checked field by field. `None` drops it.
fn clean_item(raw: &Value) -> Option<Announcement> {
    let item: Announcement = serde_json::from_value(raw.clone()).ok()?;
    if !valid_id(&item.id) || !matches!(item.kind.as_str(), "news" | "release" | "preview") {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(&item.published_at).ok()?;
    for version in [&item.min_app_version, &item.max_app_version]
        .into_iter()
        .flatten()
    {
        semver(version)?;
    }
    if item.link.as_deref().is_some_and(|link| !valid_link(link)) {
        return None;
    }
    Some(Announcement {
        title: clean_localized(&item.title, MAX_TITLE_CHARS)?,
        body: clean_localized(&item.body, MAX_BODY_CHARS)?,
        ..item
    })
}

/// The whole feed, checked. Unknown fields are ignored; bad items are dropped one by one.
pub fn parse_feed(bytes: &[u8]) -> std::result::Result<Vec<Announcement>, &'static str> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid")?;
    let schema = value
        .get("schema")
        .and_then(Value::as_u64)
        .ok_or("invalid")?;
    if schema > FEED_SCHEMA {
        return Err("too-new");
    }
    let raw = value
        .get("announcements")
        .and_then(Value::as_array)
        .ok_or("invalid")?;
    let mut items: Vec<Announcement> = Vec::new();
    for item in raw.iter().filter_map(clean_item) {
        if !items.iter().any(|known| known.id == item.id) {
            items.push(item);
        }
    }
    items.sort_by(|left, right| right.published_at.cmp(&left.published_at));
    items.truncate(MAX_ITEMS);
    Ok(items)
}

fn for_this_app(item: &Announcement) -> bool {
    let Some(app) = semver(APP_VERSION) else {
        return true;
    };
    let above_min = item
        .min_app_version
        .as_deref()
        .and_then(semver)
        .is_none_or(|min| app >= min);
    let below_max = item
        .max_app_version
        .as_deref()
        .and_then(semver)
        .is_none_or(|max| app <= max);
    above_min && below_max
}

fn view_of(local: &Local, language: &str, error: Option<String>) -> AnnouncementsView {
    let language = if LANGUAGES.contains(&language) {
        language
    } else {
        "zh-CN"
    };
    let items: Vec<AnnouncementView> = local
        .items
        .iter()
        .filter(|item| for_this_app(item))
        .map(|item| AnnouncementView {
            id: item.id.clone(),
            kind: item.kind.clone(),
            published_at: item.published_at.clone(),
            title: item.title.pick(language).to_string(),
            body: item.body.pick(language).to_string(),
            pinned: item.pinned,
            tools: item.tools.clone(),
            link: item.link.clone(),
            read: local.read.contains(&item.id),
            dismissed: local.dismissed.contains(&item.id),
        })
        .collect();
    let unread = items.iter().filter(|item| !item.read).count();
    AnnouncementsView {
        items,
        unread,
        fetched_at: local.fetched_at.clone(),
        error,
    }
}

/// What is known now, without the network.
pub fn state_view(store: &Store, language: &str) -> AnnouncementsView {
    view_of(&load(store), language, None)
}

/// Read the feed. A failure keeps the last good copy and says why.
pub fn refresh(store: &Store, fetch: &dyn Fetch, language: &str) -> AnnouncementsView {
    let mut local = load(store);
    let outcome = fetch
        .get(FEED_URL, MAX_FEED_BYTES)
        .map_err(|_| "offline")
        .and_then(|bytes| parse_feed(&bytes));
    match outcome {
        Ok(items) => {
            // Read and closed marks only matter for announcements that still exist.
            let ids: BTreeSet<String> = items.iter().map(|item| item.id.clone()).collect();
            local.read.retain(|id| ids.contains(id));
            local.dismissed.retain(|id| ids.contains(id));
            local.items = items;
            local.fetched_at = Some(crate::models::now_rfc3339());
            let error = save(store, &local).err().map(|_| "store".to_string());
            view_of(&local, language, error)
        }
        Err(code) => view_of(&local, language, Some(code.to_string())),
    }
}

/// Mark announcements as read (opened in the list) or closed (the pinned card was dismissed).
pub fn mark(
    store: &Store,
    ids: &[String],
    dismiss: bool,
    language: &str,
) -> Result<AnnouncementsView> {
    let mut local = load(store);
    for id in ids {
        if !valid_id(id) {
            return Err(Error::invalid("invalid announcement id"));
        }
        if local.items.iter().any(|item| &item.id == id) {
            local.read.insert(id.clone());
            if dismiss {
                local.dismissed.insert(id.clone());
            }
        }
    }
    save(store, &local)?;
    Ok(view_of(&local, language, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(items: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({ "schema": 1, "announcements": items })).unwrap()
    }

    fn item(id: &str) -> Value {
        serde_json::json!({
            "id": id,
            "kind": "news",
            "publishedAt": "2026-10-02T08:00:00Z",
            "title": { "zh-CN": "标题", "en": "Title" },
            "body": { "zh-CN": "正文" },
        })
    }

    #[test]
    fn a_good_feed_is_read_newest_first_and_falls_back_to_chinese() {
        let mut older = item("older");
        older["publishedAt"] = "2026-09-01T00:00:00Z".into();
        let items = parse_feed(&feed(serde_json::json!([older, item("newer")]))).unwrap();
        assert_eq!(items[0].id, "newer");
        assert_eq!(items[0].title.pick("en"), "Title");
        assert_eq!(
            items[0].body.pick("en"),
            "正文",
            "a missing language falls back"
        );
        assert_eq!(items[0].title.pick("zh-TW"), "标题");
    }

    #[test]
    fn bad_items_are_dropped_one_by_one() {
        let mut bad_link = item("bad-link");
        bad_link["link"] = "http://github.com/x".into();
        let mut evil_link = item("evil-link");
        evil_link["link"] = "https://github.com.evil.example/x".into();
        let mut bad_id = item("../x");
        bad_id["id"] = "../x".into();
        let mut bad_kind = item("bad-kind");
        bad_kind["kind"] = "script".into();
        let mut long = item("long");
        long["title"] = serde_json::json!({ "zh-CN": "x".repeat(MAX_TITLE_CHARS + 1) });
        let mut good_link = item("good-link");
        good_link["link"] = "https://github.com/Jia-Ethan/keysmith-switch-releases".into();
        let items = parse_feed(&feed(serde_json::json!([
            bad_link,
            evil_link,
            bad_id,
            bad_kind,
            long,
            good_link,
            item("good-link")
        ])))
        .unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["good-link"], "only the valid one, once");
    }

    #[test]
    fn a_newer_feed_format_or_garbage_is_refused() {
        assert_eq!(
            parse_feed(br#"{"schema":2,"announcements":[]}"#),
            Err("too-new")
        );
        assert_eq!(parse_feed(b"<html>"), Err("invalid"));
        assert_eq!(parse_feed(br#"{"announcements":[]}"#), Err("invalid"));
    }

    #[test]
    fn control_characters_are_removed_but_line_breaks_stay() {
        let mut raw = item("text");
        raw["body"] =
            serde_json::json!({ "zh-CN": "第一行\n第二行\u{0007}\u{202e}\u{200b}\u{2067}" });
        let items = parse_feed(&feed(serde_json::json!([raw]))).unwrap();
        assert_eq!(items[0].body.zh_cn, "第一行\n第二行");
    }

    #[test]
    fn version_ranges_limit_who_sees_an_announcement() {
        let mut old_only = item("old");
        old_only["maxAppVersion"] = "0.0.1".into();
        let mut everyone = item("all");
        everyone["minAppVersion"] = "0.0.1".into();
        let items = parse_feed(&feed(serde_json::json!([old_only, everyone]))).unwrap();
        let local = Local {
            items,
            ..Local::default()
        };
        let view = view_of(&local, "zh-CN", None);
        let ids: Vec<&str> = view.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["all"]);
        assert_eq!(view.unread, 1);
    }
}
