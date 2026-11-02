//! Harness packs (`kind: "harness"`): agent components written into each agent's own folders.
//!
//! A harness pack carries skills, slash commands, subagents and rule files for Claude Code,
//! Codex and Grok Build. Tier 1 only: nothing in it runs code, and its frontmatter is an
//! allowlist, so nothing in it widens what an agent may do. The checks here are the ones the
//! extensions repo's `packlib` applies (its SPEC §2.2); the app applies them again to every
//! archive, because where a pack came from says nothing about whether it is well formed.
//!
//! Writing follows the same rule as deploying a prompt: show it, let the person confirm, then
//! write, and keep a record so it can be undone.
//! - The ledger (`~/.keysmith-switch/components/ledger.json`) names every file this app wrote
//!   and the SHA-256 it wrote. It is the only proof that a file is ours. It lives outside the
//!   extensions folder, so clearing the app's data does not forget files that are still there.
//! - A target that exists and is not in the ledger is a conflict, and the install is refused.
//!   There is no "overwrite".
//! - Uninstalling removes a ledger file only while it still holds the bytes this app wrote; a
//!   file the person changed stays.
//! - An update never applies by itself. It is a new preview, with a diff per file, that the
//!   person confirms.
//! - Before anything is written, the intended changes go to a journal. An interrupted write is
//!   finished (or dropped where the disk no longer matches) the next time this module runs.
//!
//! Nothing here touches a file an adapter manages (`CLAUDE.md`'s managed block, Codex's
//! `model_instructions_file`, Grok's `rules/99-keysmith.md`), so deploying a prompt and
//! installing components never step on each other.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component as PathPart, Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::adapter::AdapterOptions;
use crate::db::Store;
use crate::error::{Error, Result};
use crate::lock::HomeLock;
use crate::models::{now_rfc3339, ToolKind};
use crate::paths::atomic_write;

type Loc = BTreeMap<String, String>;

/// The first app that installs harness packs.
pub const HARNESS_MIN_APP: (u64, u64, u64) = (0, 9, 0);
/// The agents harness packs can target. ZCode, pi and DSH are not checked well enough yet.
pub const HARNESS_TOOLS: [ToolKind; 3] = [ToolKind::Claude, ToolKind::Codex, ToolKind::Grok];
const MAX_COMPONENTS: usize = 50;
const MAX_SKILL_FILES: usize = 20;
const MAX_FILES: usize = 200;
const MAX_COMPONENT_NAME: usize = 40;
const MAX_FRONTMATTER_VALUE: usize = 1024;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_PACK_BYTES: usize = 4 * 1024 * 1024;
const COMPONENT_KEYS: [&str; 6] = ["id", "type", "name", "tools", "title", "files"];
const RESERVED_NAME_PREFIXES: [&str; 2] = ["ks-", "keysmith"];
const OFFICIAL_ID_PREFIX: &str = "keysmith.";
/// Names the supported agents already use (packlib's list, fixed for schema 1). A name an
/// agent adds later is caught by the install-time conflict check, not by the format.
const RESERVED_INVOCABLE_NAMES: [&str; 61] = [
    "add-dir",
    "agents",
    "approvals",
    "bug",
    "clear",
    "compact",
    "config",
    "context",
    "cost",
    "diff",
    "doctor",
    "exit",
    "export",
    "fast",
    "feedback",
    "fork",
    "help",
    "hooks",
    "ide",
    "init",
    "login",
    "logout",
    "mcp",
    "memory",
    "mention",
    "model",
    "new",
    "output-style",
    "permissions",
    "plan",
    "plugin",
    "plugins",
    "pr-comments",
    "prompts",
    "quit",
    "release-notes",
    "resume",
    "review",
    "rewind",
    "sandbox",
    "security-review",
    "settings",
    "skills",
    "status",
    "statusline",
    "theme",
    "todos",
    "undo",
    "upgrade",
    "usage",
    "vim",
    "batch",
    "claude-api",
    "code-review",
    "debug",
    "fewer-permission-prompts",
    "keybindings-help",
    "loop",
    "run",
    "simplify",
    "update-config",
];
const RESERVED_AGENT_NAMES: [&str; 7] = [
    "default",
    "explore",
    "explorer",
    "general-purpose",
    "plan",
    "statusline-setup",
    "worker",
];
/// Names Windows reserves for devices: `aux.md` there is the AUX device, not a file.
const WINDOWS_DEVICE_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];
/// Grok loads `~/.grok/rules/*.md` alphabetically; a pack's rule files sort before the
/// `99-keysmith.md` its adapter deploys and can never take that name.
const GROK_RULE_PREFIX: &str = "60-";

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static pattern")
}

// ----- the format ----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentType {
    Skill,
    Command,
    Agent,
    Rule,
}

impl ComponentType {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "skill" => Some(Self::Skill),
            "command" => Some(Self::Command),
            "agent" => Some(Self::Agent),
            "rule" => Some(Self::Rule),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Command => "command",
            Self::Agent => "agent",
            Self::Rule => "rule",
        }
    }

    /// Skills and commands are both called as `/name`, so they share one namespace.
    pub fn namespace(self) -> &'static str {
        match self {
            Self::Skill | Self::Command => "invocable",
            Self::Agent => "agent",
            Self::Rule => "rule",
        }
    }

    /// Whether this kind of component has a place on that agent.
    pub fn places_on(self, tool: ToolKind) -> bool {
        match self {
            Self::Skill | Self::Agent => HARNESS_TOOLS.contains(&tool),
            Self::Command | Self::Rule => matches!(tool, ToolKind::Claude | ToolKind::Grok),
        }
    }

    fn allowed_keys(self) -> &'static [&'static str] {
        match self {
            Self::Skill | Self::Agent => &["name", "description"],
            Self::Command => &["description", "argument-hint"],
            Self::Rule => &[],
        }
    }

    fn required_keys(self) -> &'static [&'static str] {
        match self {
            Self::Skill | Self::Agent => &["name", "description"],
            Self::Command | Self::Rule => &[],
        }
    }
}

fn harness_tool(name: &str) -> Option<ToolKind> {
    match name {
        "claude" => Some(ToolKind::Claude),
        "codex" => Some(ToolKind::Codex),
        "grok" => Some(ToolKind::Grok),
        _ => None,
    }
}

/// What the index says about one component: enough for a card, never file contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSummary {
    pub id: String,
    pub kind: ComponentType,
    pub name: String,
    pub tools: Vec<ToolKind>,
    pub title: Loc,
}

/// One file of a component, checked and with its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentFile {
    /// Its place in the pack, such as `components/review/SKILL.md`.
    pub path: String,
    pub content: String,
    pub sha256: String,
}

/// A component read from a verified archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub id: String,
    pub kind: ComponentType,
    pub name: String,
    pub tools: Vec<ToolKind>,
    pub title: Loc,
    pub files: Vec<ComponentFile>,
}

impl Component {
    pub fn summary(&self) -> ComponentSummary {
        ComponentSummary {
            id: self.id.clone(),
            kind: self.kind,
            name: self.name.clone(),
            tools: self.tools.clone(),
            title: self.title.clone(),
        }
    }
}

fn loc(value: Option<&Value>) -> Option<Loc> {
    let map = value?.as_object()?;
    if map.is_empty() {
        return None;
    }
    let mut out = Loc::new();
    for (key, text) in map {
        if !["zh-CN", "zh-TW", "en"].contains(&key.as_str()) {
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

fn component_name_ok(name: &str) -> bool {
    regex(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$").is_match(name)
        && name.len() <= MAX_COMPONENT_NAME
        && !WINDOWS_DEVICE_NAMES.contains(&name)
}

/// One segment of a path inside a harness pack. ASCII letters, digits, `.`, `_` and `-` only:
/// the file systems the agents run on fold names (APFS and NTFS ignore case, and APFS also
/// folds Unicode such as `ſ` onto `s` and NFD onto NFC), so two names that look different
/// here could be one file on disk, and the second written would replace the first after it
/// was checked. Plain ASCII folds only by case, which the duplicate check covers.
fn path_segment_ok(segment: &str) -> bool {
    let base = segment.split('.').next().unwrap_or("").to_ascii_lowercase();
    regex(r"^[A-Za-z0-9_][A-Za-z0-9._-]{0,63}$").is_match(segment)
        && !segment.ends_with('.')
        && !WINDOWS_DEVICE_NAMES.contains(&base.as_str())
}

/// The component list of an index entry. `None` when it is not one this app can show.
pub fn parse_summaries(
    value: Option<&Value>,
    pack_tools: &[String],
) -> Option<Vec<ComponentSummary>> {
    let list = value?.as_array()?;
    if list.is_empty() || list.len() > MAX_COMPONENTS {
        return None;
    }
    let mut out = Vec::new();
    for entry in list {
        let kind = ComponentType::parse(entry.get("type")?.as_str()?)?;
        let name = entry.get("name")?.as_str()?.to_string();
        if !component_name_ok(&name) {
            return None;
        }
        let mut tools = Vec::new();
        for tool in entry.get("tools")?.as_array()? {
            let tool = tool.as_str()?;
            if !pack_tools.iter().any(|known| known == tool) {
                return None;
            }
            let tool = harness_tool(tool)?;
            if !kind.places_on(tool) || tools.contains(&tool) {
                return None;
            }
            tools.push(tool);
        }
        if tools.is_empty() {
            return None;
        }
        out.push(ComponentSummary {
            id: entry.get("id")?.as_str()?.to_string(),
            kind,
            name,
            tools,
            title: loc(entry.get("title"))?,
        });
    }
    Some(out)
}

fn fail<T>(message: impl Into<String>) -> std::result::Result<T, String> {
    Err(message.into())
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

/// Read a harness pack's components from its manifest and the archive's files, checking every
/// rule of SPEC §2.2. Each file taken is removed from `files`, so leftovers are the caller's
/// "unlisted file" error. `tools` and `min_app_version` were checked by the caller already.
pub fn load(
    manifest: &Value,
    files: &mut BTreeMap<String, Vec<u8>>,
    pack_id: &str,
    tools: &[&str],
    min_app_version: (u64, u64, u64),
) -> std::result::Result<Vec<Component>, String> {
    let mut pack_tools = Vec::new();
    for tool in tools {
        match harness_tool(tool) {
            Some(tool) => pack_tools.push(tool),
            None => return fail("pack.json: harness pack tools must be claude, codex or grok"),
        }
    }
    if min_app_version < HARNESS_MIN_APP {
        return fail("pack.json: a harness pack needs min_app_version 0.9.0 or later");
    }
    for key in ["items", "rules"] {
        if manifest.get(key).is_some() {
            return fail(format!("pack.json: a harness pack has no {key}"));
        }
    }
    if manifest.get("tier").and_then(Value::as_i64) != Some(1)
        || manifest.get("tier").is_some_and(Value::is_boolean)
    {
        return fail("pack.json: tier must be 1");
    }
    let entries = manifest
        .get("components")
        .and_then(Value::as_array)
        .ok_or("pack.json: components missing")?;
    if entries.is_empty() || entries.len() > MAX_COMPONENTS {
        return fail("pack.json: wrong number of components");
    }
    let official = pack_id.starts_with(OFFICIAL_ID_PREFIX);
    let item_id = regex(r"^[a-z0-9]+(-[a-z0-9]+)*$");
    let hash = regex(r"^[0-9a-f]{64}$");
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut covered: Vec<ToolKind> = Vec::new();
    let mut total = 0usize;
    let mut file_count = 0usize;
    let mut components = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let at = format!("components[{index}]");
        let object = entry
            .as_object()
            .ok_or(format!("{at}: must be an object"))?;
        if let Some(extra) = object
            .keys()
            .find(|key| !COMPONENT_KEYS.contains(&key.as_str()))
        {
            return fail(format!("{at}: unknown key {extra:?}"));
        }
        let id = object.get("id").and_then(Value::as_str).unwrap_or("");
        if !item_id.is_match(id) || id.len() > 64 || !ids.insert(id.to_string()) {
            return fail(format!("{at}: bad or duplicate id {id:?}"));
        }
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .and_then(ComponentType::parse)
            .ok_or(format!("{at}: bad type"))?;
        let name = object.get("name").and_then(Value::as_str).unwrap_or("");
        if !component_name_ok(name) {
            return fail(format!("{at}: bad name {name:?}"));
        }
        if !official
            && RESERVED_NAME_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            return fail(format!(
                "{at}: names starting with ks- or keysmith are kept for official packs"
            ));
        }
        if kind.namespace() == "invocable" && RESERVED_INVOCABLE_NAMES.contains(&name) {
            return fail(format!("{at}: /{name} is a built-in command"));
        }
        if kind == ComponentType::Agent && RESERVED_AGENT_NAMES.contains(&name) {
            return fail(format!("{at}: {name} is a built-in subagent"));
        }
        if !names.insert((kind.namespace(), name.to_string())) {
            return fail(format!(
                "{at}: another component already uses the name {name:?}"
            ));
        }
        let mut component_tools = Vec::new();
        for tool in object
            .get("tools")
            .and_then(Value::as_array)
            .ok_or(format!("{at}: tools missing"))?
        {
            let tool = tool.as_str().unwrap_or("");
            if !tools.contains(&tool) {
                return fail(format!(
                    "{at}: tool {tool:?} is not one of the pack's tools"
                ));
            }
            let tool = harness_tool(tool).ok_or(format!("{at}: bad tool"))?;
            if !kind.places_on(tool) {
                return fail(format!(
                    "{at}: a {} has nowhere to go on {}",
                    kind.as_str(),
                    tool.as_str()
                ));
            }
            if component_tools.contains(&tool) {
                return fail(format!("{at}: duplicate tool"));
            }
            component_tools.push(tool);
        }
        if component_tools.is_empty() {
            return fail(format!("{at}: tools must not be empty"));
        }
        for tool in &component_tools {
            if !covered.contains(tool) {
                covered.push(*tool);
            }
        }
        let title = loc(object.get("title")).ok_or(format!("{at}: bad title"))?;
        let listed = object
            .get("files")
            .and_then(Value::as_array)
            .ok_or(format!("{at}: files missing"))?;
        if listed.is_empty() {
            return fail(format!("{at}: files must not be empty"));
        }
        let mut paths = Vec::new();
        let mut declared = Vec::new();
        for file in listed {
            let object = file.as_object().ok_or(format!("{at}: bad file entry"))?;
            if object.len() != 2 || !object.contains_key("path") || !object.contains_key("sha256") {
                return fail(format!("{at}: a file entry has only path and sha256"));
            }
            let path = object.get("path").and_then(Value::as_str).unwrap_or("");
            if !safe_name(path) || !path.ends_with(".md") || !path.split('/').all(path_segment_ok) {
                return fail(format!("{at}: bad file path {path:?}"));
            }
            if !folded.insert(path.to_ascii_lowercase()) {
                return fail(format!("{at}: file {path:?} is used twice"));
            }
            let sha = object.get("sha256").and_then(Value::as_str).unwrap_or("");
            if !hash.is_match(sha) {
                return fail(format!("{at}: bad sha256"));
            }
            paths.push(path.to_string());
            declared.push(sha.to_string());
        }
        file_count += paths.len();
        if kind == ComponentType::Skill {
            let root = format!("components/{id}/");
            if paths.len() > MAX_SKILL_FILES {
                return fail(format!("{at}: a skill has at most {MAX_SKILL_FILES} files"));
            }
            if paths.iter().any(|path| !path.starts_with(&root)) {
                return fail(format!("{at}: a skill's files must be under {root}"));
            }
            let primary = format!("{root}SKILL.md");
            if !paths.contains(&primary) {
                return fail(format!("{at}: a skill needs {primary}"));
            }
            if paths.iter().any(|path| {
                *path != primary
                    && path
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .eq_ignore_ascii_case("skill.md")
            }) {
                return fail(format!(
                    "{at}: only the top-level SKILL.md may be named SKILL.md"
                ));
            }
        } else if paths != [format!("components/{id}.md")] {
            return fail(format!(
                "{at}: a {} is exactly one file, components/{id}.md",
                kind.as_str()
            ));
        }
        let mut component = Component {
            id: id.to_string(),
            kind,
            name: name.to_string(),
            tools: component_tools,
            title,
            files: Vec::new(),
        };
        for (path, sha) in paths.into_iter().zip(declared) {
            let data = files
                .remove(&path)
                .ok_or(format!("{path} is missing or used twice"))?;
            if hex::encode(Sha256::digest(&data)) != sha {
                return fail(format!("{path}: does not match its sha256"));
            }
            total += data.len();
            let content = check_component_text(&data, &component, &path)?;
            component.files.push(ComponentFile {
                path,
                content,
                sha256: sha,
            });
        }
        components.push(component);
    }
    let missing: Vec<&str> = pack_tools
        .iter()
        .filter(|tool| !covered.contains(tool))
        .map(|tool| tool.as_str())
        .collect();
    if !missing.is_empty() {
        return fail(format!("pack.json: no component is for {missing:?}"));
    }
    if file_count > MAX_FILES {
        return fail("pack.json: too many files");
    }
    if total > MAX_PACK_BYTES {
        return fail("files add up to more than the pack limit");
    }
    Ok(components)
}

fn hidden(ch: char) -> bool {
    matches!(ch as u32,
        0x061C | 0x200B | 0x200E | 0x200F | 0x202A..=0x202E | 0x2060..=0x2064
        | 0x2066..=0x2069 | 0xFEFF | 0xE0000..=0xE007F)
}

/// The text rules every harness file follows: what a reviewer sees is what the model reads,
/// and nothing in it runs a shell or pulls in a file from outside the project.
fn harness_text(data: &[u8], label: &str) -> std::result::Result<String, String> {
    if data.iter().all(u8::is_ascii_whitespace) {
        return fail(format!("{label}: file is empty"));
    }
    if data.len() > MAX_FILE_BYTES {
        return fail(format!("{label}: file is too large"));
    }
    let text = String::from_utf8(data.to_vec()).map_err(|_| format!("{label}: not UTF-8"))?;
    if text.contains('\r') {
        return fail(format!("{label}: use LF line endings"));
    }
    if let Some(bad) = text
        .chars()
        .find(|ch| *ch != '\t' && *ch != '\n' && ch.is_control())
    {
        return fail(format!("{label}: control character U+{:04X}", bad as u32));
    }
    if let Some(bad) = text.chars().find(|ch| hidden(*ch)) {
        return fail(format!("{label}: invisible character U+{:04X}", bad as u32));
    }
    if text.contains("!`") || regex(r"(?m)^[ \t]*(?:`{3,}|~{3,})[ \t]*!").is_match(&text) {
        return fail(format!(
            "{label}: shell commands (!`cmd`, ```!) are not allowed"
        ));
    }
    if regex(r"(?m)(?:^|[\s(\[])@(?:[~/$]|\.\.)").is_match(&text) {
        return fail(format!(
            "{label}: @ references outside the project are not allowed"
        ));
    }
    Ok(text)
}

/// A strict frontmatter subset: `---`, one `key: value` per line, `---`. Values are a single
/// line, plain or quoted; anything YAML would read differently from how it looks is refused.
/// Returns `None` for the fields when the text has no frontmatter.
pub fn split_frontmatter(
    text: &str,
    label: &str,
) -> std::result::Result<(Option<BTreeMap<String, String>>, String), String> {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines[0].trim_end_matches([' ', '\t']) != "---" {
        return Ok((None, text.to_string()));
    }
    if lines[0] != "---" {
        return fail(format!(
            "{label}: the opening --- must be alone on its line"
        ));
    }
    let end = (1..lines.len())
        .find(|i| lines[*i] == "---")
        .ok_or(format!("{label}: frontmatter is not closed with ---"))?;
    let line_re = regex(r"^([a-z][a-z0-9-]*):(?: +(.*))?$");
    let not_a_string = regex(
        r"(?i)^(?:true|false|yes|no|on|off|y|n|null|~|[-+]?0[xob][0-9a-f_]+|[-+]?\.?[0-9][0-9_.:e+-]*|[-+]?\.inf|\.nan)$",
    );
    let mut fields = BTreeMap::new();
    for (offset, line) in lines[1..end].iter().enumerate() {
        let at = format!("{label}: frontmatter line {}", offset + 2);
        if line.trim().is_empty() {
            continue;
        }
        let caps = line_re
            .captures(line)
            .ok_or(format!("{at}: must be `key: value` on one line"))?;
        let key = caps[1].to_string();
        let raw = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
        if fields.contains_key(&key) {
            return fail(format!("{at}: duplicate key {key:?}"));
        }
        if raw.is_empty() {
            return fail(format!("{at}: {key} needs a value on the same line"));
        }
        let first = raw.chars().next().unwrap_or(' ');
        let value = if first == '"' || first == '\'' {
            let inner = &raw[1..];
            if raw.chars().count() < 2
                || !raw.ends_with(first)
                || inner[..inner.len() - 1].contains(first)
                || (first == '"' && raw.contains('\\'))
            {
                return fail(format!(
                    "{at}: a quoted value must be one plain quoted string"
                ));
            }
            inner[..inner.len() - 1].to_string()
        } else {
            if "[]{}|>&*!%@`#,?:-\"'".contains(first)
                || raw.contains(": ")
                || raw.contains(" #")
                || raw.ends_with(':')
                || not_a_string.is_match(raw)
            {
                return fail(format!("{at}: put the value of {key} in double quotes"));
            }
            raw.to_string()
        };
        if value.trim().is_empty() {
            return fail(format!("{at}: {key} must not be empty"));
        }
        if value.chars().count() > MAX_FRONTMATTER_VALUE {
            return fail(format!("{at}: {key} is too long"));
        }
        fields.insert(key, value);
    }
    Ok((Some(fields), lines[end + 1..].join("\n")))
}

fn check_component_text(
    data: &[u8],
    component: &Component,
    path: &str,
) -> std::result::Result<String, String> {
    let text = harness_text(data, path)?;
    let kind = component.kind;
    if kind == ComponentType::Skill && path != format!("components/{}/SKILL.md", component.id) {
        // A skill's supporting file: read on demand, never parsed for metadata.
        return Ok(text);
    }
    let (fields, body) = split_frontmatter(&text, path)?;
    if body.trim().is_empty() {
        return fail(format!("{path}: has no instructions after the frontmatter"));
    }
    let Some(fields) = fields else {
        if !kind.required_keys().is_empty() {
            return fail(format!("{path}: a {} needs frontmatter", kind.as_str()));
        }
        return Ok(text);
    };
    if kind.allowed_keys().is_empty() {
        return fail(format!(
            "{path}: a {} file has no frontmatter",
            kind.as_str()
        ));
    }
    if let Some(extra) = fields
        .keys()
        .find(|key| !kind.allowed_keys().contains(&key.as_str()))
    {
        return fail(format!(
            "{path}: frontmatter key {extra:?} is not allowed for a {}",
            kind.as_str()
        ));
    }
    if let Some(missing) = kind
        .required_keys()
        .iter()
        .find(|key| !fields.contains_key(**key))
    {
        return fail(format!("{path}: frontmatter is missing {missing}"));
    }
    if fields
        .get("name")
        .is_some_and(|name| *name != component.name)
    {
        return fail(format!(
            "{path}: frontmatter name must equal the component name"
        ));
    }
    if kind == ComponentType::Agent
        && component.tools.contains(&ToolKind::Codex)
        && body.contains("'''")
    {
        return fail(format!("{path}: a Codex subagent cannot contain '''"));
    }
    Ok(text)
}

/// A Codex subagent file (`~/.codex/agents/<name>.toml`) from a markdown subagent.
pub fn codex_agent_toml(component: &Component) -> std::result::Result<String, String> {
    let file = component.files.first().ok_or("agent has no file")?;
    let (fields, body) = split_frontmatter(&file.content, &file.path)?;
    let fields = fields.ok_or("agent has no frontmatter")?;
    let description = fields
        .get("description")
        .ok_or("agent has no description")?;
    let instructions = format!("{}\n", body.trim_start_matches('\n').trim_end());
    if instructions.contains("'''") {
        return fail("agent instructions cannot contain '''");
    }
    let mut doc = toml_edit::DocumentMut::new();
    doc["name"] = toml_edit::value(component.name.as_str());
    doc["description"] = toml_edit::value(description.as_str());
    let mut text = doc.to_string();
    text.push_str("developer_instructions = '''\n");
    text.push_str(&instructions);
    text.push_str("'''\n");
    // What is written must read back as exactly what was meant.
    let check: toml_edit::DocumentMut = text.parse().map_err(|_| "agent TOML does not parse")?;
    if check.get("name").and_then(|v| v.as_str()) != Some(component.name.as_str())
        || check.get("description").and_then(|v| v.as_str()) != Some(description.as_str())
        || check.get("developer_instructions").and_then(|v| v.as_str())
            != Some(instructions.as_str())
    {
        return fail("agent TOML does not read back as written");
    }
    Ok(text)
}

// ----- where things go -----------------------------------------------------------------

/// The agents' folders, from an isolated home when there is one (tests, previews), else from
/// `CLAUDE_CONFIG_DIR` / `CODEX_HOME` / `GROK_HOME`, else the user's home.
#[derive(Debug, Clone)]
pub struct Roots {
    pub home: PathBuf,
    pub claude: PathBuf,
    pub codex: PathBuf,
    pub grok: PathBuf,
    /// `~/.agents`: Codex reads user skills from `~/.agents/skills`, a folder ZCode, pi and
    /// DSH read too.
    pub agents: PathBuf,
}

impl Roots {
    pub fn resolve(opts: &AdapterOptions) -> Result<Self> {
        let home = opts
            .home
            .clone()
            .or_else(dirs::home_dir)
            .ok_or_else(|| Error::invalid("the home folder is unknown"))?;
        let isolated = opts.home.as_deref();
        Ok(Self {
            claude: crate::rewrite::claude::claude_dir(isolated)
                .unwrap_or_else(|| home.join(".claude")),
            codex: crate::rewrite::link::codex_dir(isolated).unwrap_or_else(|| home.join(".codex")),
            grok: crate::rewrite::grok::grok_dir(isolated).unwrap_or_else(|| home.join(".grok")),
            agents: home.join(".agents"),
            home,
        })
    }

    fn agent(&self, tool: ToolKind) -> &Path {
        match tool {
            ToolKind::Codex => &self.codex,
            ToolKind::Grok => &self.grok,
            _ => &self.claude,
        }
    }

    /// An agent counts as present when its own folder exists. Switch does not create an
    /// agent's folder for an agent that is not there.
    pub fn present(&self, tool: ToolKind) -> bool {
        self.agent(tool).is_dir()
    }

    /// The folder a skill goes into, or the single file any other component becomes.
    fn unit(&self, component: &Component, tool: ToolKind) -> PathBuf {
        let name = &component.name;
        match (component.kind, tool) {
            (ComponentType::Skill, ToolKind::Codex) => self.agents.join("skills").join(name),
            (ComponentType::Skill, _) => self.agent(tool).join("skills").join(name),
            (ComponentType::Command, _) => {
                self.agent(tool).join("commands").join(format!("{name}.md"))
            }
            (ComponentType::Agent, ToolKind::Codex) => {
                self.codex.join("agents").join(format!("{name}.toml"))
            }
            (ComponentType::Agent, _) => self.agent(tool).join("agents").join(format!("{name}.md")),
            (ComponentType::Rule, ToolKind::Grok) => self
                .grok
                .join("rules")
                .join(format!("{GROK_RULE_PREFIX}{name}.md")),
            (ComponentType::Rule, _) => self.agent(tool).join("rules").join(format!("{name}.md")),
        }
    }

    /// Every file a component becomes on one agent: (target, text to write).
    pub fn targets(
        &self,
        component: &Component,
        tool: ToolKind,
    ) -> std::result::Result<Vec<(PathBuf, String)>, String> {
        let unit = self.unit(component, tool);
        match component.kind {
            ComponentType::Skill => {
                let root = format!("components/{}/", component.id);
                Ok(component
                    .files
                    .iter()
                    .map(|file| {
                        let relative = file.path.strip_prefix(&root).unwrap_or(&file.path);
                        let mut target = unit.clone();
                        for part in relative.split('/') {
                            target.push(part);
                        }
                        (target, file.content.clone())
                    })
                    .collect())
            }
            ComponentType::Agent if tool == ToolKind::Codex => {
                Ok(vec![(unit, codex_agent_toml(component)?)])
            }
            _ => Ok(vec![(unit, component.files[0].content.clone())]),
        }
    }

    /// A path as the person reads it: under the home folder it starts with `~/`.
    pub fn display(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(relative) => {
                let parts: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                format!("~/{}", parts.join("/"))
            }
            Err(_) => path.to_string_lossy().into_owned(),
        }
    }
}

// ----- the ledger --------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerFile {
    pub component: String,
    #[serde(rename = "type")]
    pub kind: ComponentType,
    pub name: String,
    pub tool: ToolKind,
    /// Absolute, as written.
    pub path: String,
    /// What this app wrote. A file that still hashes to this has not been changed.
    pub sha256: String,
    /// The agent folder it was written under. Nothing between that folder and the file may
    /// be a link when the file is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerPack {
    pub version: String,
    pub source: String,
    pub installed_at: String,
    pub files: Vec<LedgerFile>,
    /// Folders this app created for the pack, so they can go again once empty.
    #[serde(default)]
    pub dirs: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    packs: BTreeMap<String, LedgerPack>,
}

fn components_dir(store: &Store) -> PathBuf {
    store.paths().home.join("components")
}

fn ledger_path(store: &Store) -> PathBuf {
    components_dir(store).join("ledger.json")
}

fn journal_path(store: &Store) -> PathBuf {
    components_dir(store).join("journal.json")
}

/// A damaged ledger proves nothing, so it is read as empty: files it named become the person's
/// own and are never removed. The damaged copy is kept beside it.
fn load_ledger(store: &Store) -> Ledger {
    let path = ledger_path(store);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ledger::default();
    };
    match serde_json::from_str(&text) {
        Ok(ledger) => ledger,
        Err(_) => {
            let aside = path.with_file_name(format!(
                "ledger.json.damaged-{}",
                uuid::Uuid::new_v4().simple()
            ));
            let _ = std::fs::rename(&path, aside);
            Ledger::default()
        }
    }
}

fn save_ledger(store: &Store, ledger: &Ledger) -> Result<()> {
    atomic_write(&ledger_path(store), &serde_json::to_string_pretty(ledger)?)
}

/// What is on disk at a path, without following a link.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OnDisk {
    Absent,
    File(String),
    /// A folder, a link, or something unreadable: never ours to write over.
    Other,
}

fn on_disk(path: &Path) -> OnDisk {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => OnDisk::Absent,
        Ok(meta) if meta.is_file() => match std::fs::read(path) {
            Ok(bytes) => OnDisk::File(hex::encode(Sha256::digest(&bytes))),
            Err(_) => OnDisk::Other,
        },
        _ => OnDisk::Other,
    }
}

fn sha_of(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// What one installed pack looks like on disk now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledState {
    pub version: String,
    pub files: u32,
    /// Files the person changed. Updates and uninstall leave them alone.
    pub modified: u32,
    /// Files something other than Keysmith removed.
    pub missing: u32,
}

/// Every installed harness pack, as the extensions page shows it. Reads only.
pub fn installed(store: &Store) -> BTreeMap<String, InstalledState> {
    load_ledger_readonly(store)
        .packs
        .into_iter()
        .map(|(id, pack)| {
            let mut state = InstalledState {
                version: pack.version.clone(),
                files: pack.files.len() as u32,
                ..Default::default()
            };
            for file in &pack.files {
                match on_disk(Path::new(&file.path)) {
                    OnDisk::Absent => state.missing += 1,
                    OnDisk::File(sha) if sha == file.sha256 => {}
                    _ => state.modified += 1,
                }
            }
            (id, state)
        })
        .collect()
}

fn load_ledger_readonly(store: &Store) -> Ledger {
    std::fs::read_to_string(ledger_path(store))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

// ----- the journal -------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalOp {
    path: String,
    /// The folder the way down is checked from: nothing between it and `path` may be a link
    /// when the change runs, not only when it was planned.
    #[serde(default)]
    root: Option<String>,
    /// What was there when the change was planned (`None`: nothing).
    before: Option<String>,
    /// What should be there after (`None`: nothing).
    after: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Journal {
    pack_id: String,
    /// The pack's ledger entry once the change is done; `None` when the pack goes.
    entry: Option<LedgerPack>,
    ops: Vec<JournalOp>,
}

/// Make the folders a change needs. One that cannot be made (something else is there now) is
/// left: the writes below it then fail their own check and are left out, instead of the whole
/// journal failing on every start from now on.
fn create_dirs(dirs: &[String]) {
    for dir in dirs {
        let dir = Path::new(dir);
        if std::fs::symlink_metadata(dir).is_err() {
            let _ = std::fs::create_dir(dir);
        }
    }
}

/// Run a journal's changes. Each one happens only where the disk still holds what it held when
/// planned (or already holds the result), so running it twice is the same as once. A change
/// that cannot happen is left out of the ledger entry: its file is not ours.
fn run_journal(store: &Store, journal: &Journal) -> Result<BTreeSet<String>> {
    if let Some(entry) = &journal.entry {
        create_dirs(&entry.dirs);
    }
    let mut failed = BTreeSet::new();
    for op in &journal.ops {
        let path = Path::new(&op.path);
        let now = on_disk(path);
        let now_sha = match &now {
            OnDisk::File(sha) => Some(sha.clone()),
            _ => None,
        };
        if now_sha == op.after && now != OnDisk::Other {
            continue;
        }
        let matches_before = match &op.before {
            None => now == OnDisk::Absent,
            Some(sha) => now_sha.as_deref() == Some(sha.as_str()),
        };
        let way_blocked = op
            .root
            .as_deref()
            .is_some_and(|root| blocked_path(path, Path::new(root)));
        if !matches_before || (op.after.is_some() && way_blocked) {
            failed.insert(op.path.clone());
            continue;
        }
        // A write that fails is left out of the ledger like any other refused change, so one
        // unwritable file cannot leave a journal that fails again on every start.
        let done = match (&op.after, &op.content) {
            (Some(_), Some(content)) if op.before.is_none() => write_new(path, content).is_ok(),
            (Some(_), Some(content)) => atomic_write(path, content).is_ok(),
            (None, _) => std::fs::remove_file(path).is_ok(),
            _ => false,
        };
        if !done {
            failed.insert(op.path.clone());
        }
    }
    let mut ledger = load_ledger(store);
    match &journal.entry {
        Some(entry) => {
            let mut entry = entry.clone();
            entry.files.retain(|file| !failed.contains(&file.path));
            ledger.packs.insert(journal.pack_id.clone(), entry);
        }
        None => {
            ledger.packs.remove(&journal.pack_id);
        }
    }
    save_ledger(store, &ledger)?;
    if let Some(entry) = &journal.entry {
        remove_empty_dirs(&entry.dirs);
    }
    let _ = std::fs::remove_file(journal_path(store));
    Ok(failed)
}

/// Whether a change can still run exactly as planned: the disk holds what the plan saw, and
/// nothing on the way down has become a link.
fn still_as_planned(op: &JournalOp) -> bool {
    let path = Path::new(&op.path);
    if op
        .root
        .as_deref()
        .is_some_and(|root| blocked_path(path, Path::new(root)))
    {
        return false;
    }
    match (on_disk(path), &op.before) {
        (OnDisk::Absent, None) => true,
        (OnDisk::File(now), Some(before)) => now == *before,
        _ => false,
    }
}

/// Write a new file without ever replacing one: the text goes to a temporary file beside it,
/// which is then linked into place. Linking fails if the name exists (in any case or Unicode
/// form the disk treats as the same name), so a file that appeared since the last check is
/// never written over, and a crash leaves either nothing or the whole file.
fn write_new(path: &Path, content: &str) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no parent folder"))?;
    let tmp = dir.join(format!(
        ".keysmith-switch-new-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::hard_link(&tmp, path)
    })();
    let _ = std::fs::remove_file(&tmp);
    match result {
        // A volume without links (exFAT, some network drives): create the file in place,
        // still only if no file has that name. A crash here can leave part of the file; it
        // then does not match the journal and is never treated as this app's.
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists && !path.exists() => {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()
        }
        other => other,
    }
}

/// Finish whatever an interrupted change left. Called before any change and any plan.
fn recover(store: &Store) -> Result<()> {
    let path = journal_path(store);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    match serde_json::from_str::<Journal>(&text) {
        Ok(journal) => run_journal(store, &journal).map(|_| ()),
        Err(_) => {
            // A journal cut short before it was complete: nothing was written after it.
            let _ = std::fs::remove_file(&path);
            Ok(())
        }
    }
}

/// Remove folders this app created once nothing is left in them, deepest first.
fn remove_empty_dirs(dirs: &[String]) {
    let mut dirs: Vec<&String> = dirs.iter().collect();
    dirs.sort_by_key(|dir| std::cmp::Reverse(Path::new(dir).components().count()));
    for dir in dirs {
        let _ = std::fs::remove_dir(dir);
    }
}

/// Folders that do not exist yet between `path` and the first existing ancestor, outermost
/// first.
fn missing_dirs(path: &Path) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    let mut current = path.parent();
    while let Some(dir) = current {
        if std::fs::symlink_metadata(dir).is_ok() {
            break;
        }
        missing.push(dir.to_path_buf());
        current = dir.parent();
    }
    missing.reverse();
    missing
}

/// Whether the path has a link or a file where a folder should be between `root` and it.
fn blocked_path(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return true;
    };
    let mut current = root.to_path_buf();
    let parts: Vec<_> = relative.components().collect();
    for (index, part) in parts.iter().enumerate() {
        let PathPart::Normal(name) = part else {
            return true;
        };
        current.push(name);
        let last = index + 1 == parts.len();
        match std::fs::symlink_metadata(&current) {
            Err(_) => return false,
            Ok(meta) if meta.file_type().is_symlink() => return true,
            Ok(meta) if !last && !meta.is_dir() => return true,
            Ok(_) => {}
        }
    }
    false
}

// ----- planning ------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileAction {
    /// A new file.
    Add,
    /// Ours, unchanged by the person, and the pack changed it.
    Update,
    /// Ours and already what the pack says.
    Same,
    /// Ours, but the person changed it: left as it is.
    KeepModified,
    /// Ours, no longer in the pack, unchanged: removed.
    Remove,
    /// Ours, no longer in the pack, but changed by the person: left, and no longer tracked.
    Release,
    /// Something that is not ours is there. The whole install is refused.
    Conflict,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    pub component: String,
    #[serde(rename = "type")]
    pub kind: ComponentType,
    pub name: String,
    pub tool: ToolKind,
    pub path: String,
    /// The path as the person reads it (`~/.claude/skills/...`).
    pub display: String,
    pub action: FileAction,
    /// The full text that is written (add, update).
    pub content: Option<String>,
    /// What changes (update), as a unified diff.
    pub diff: Option<String>,
    /// Why a conflict is one: `exists`, `not-a-file`, `other-pack:<id>`.
    pub reason: Option<String>,
    #[serde(skip)]
    before: Option<String>,
    #[serde(skip)]
    after: Option<String>,
    /// The folder the way down to `path` is checked from.
    #[serde(skip)]
    root: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessPlan {
    /// Names exactly this change against exactly this disk. Confirming needs the same id.
    pub plan_id: String,
    pub pack_id: String,
    pub version: String,
    /// The version installed now, for an update.
    pub from_version: Option<String>,
    pub files: Vec<PlannedFile>,
    /// Agents the pack is for that are not on this computer. Their components are skipped.
    pub skipped_tools: Vec<ToolKind>,
    /// A Codex skill goes into `~/.agents/skills`, which ZCode, pi and DSH read too.
    pub shared_skills: bool,
    pub blocked: bool,
    pub nothing_to_do: bool,
    #[serde(skip)]
    new_dirs: Vec<String>,
}

/// What installing (or updating to) `components` would do on this disk.
pub fn plan(
    store: &Store,
    pack_id: &str,
    version: &str,
    components: &[Component],
    opts: &AdapterOptions,
) -> Result<HarnessPlan> {
    {
        let _lock = HomeLock::acquire(store.paths())?;
        recover(store)?;
    }
    let roots = Roots::resolve(opts)?;
    let ledger = load_ledger(store);
    let previous = ledger.packs.get(pack_id);
    let ours: BTreeMap<&str, &LedgerFile> = previous
        .map(|p| p.files.iter().map(|f| (f.path.as_str(), f)).collect())
        .unwrap_or_default();
    let others: BTreeMap<&str, &str> = ledger
        .packs
        .iter()
        .filter(|(id, _)| id.as_str() != pack_id)
        .flat_map(|(id, p)| p.files.iter().map(move |f| (f.path.as_str(), id.as_str())))
        .collect();
    // A skill folder already there is someone else's unless this pack wrote into it.
    let our_dirs: BTreeSet<&str> = previous
        .map(|p| p.dirs.iter().map(String::as_str).collect())
        .unwrap_or_default();

    let mut files = Vec::new();
    let mut skipped: Vec<ToolKind> = Vec::new();
    let mut new_dirs: Vec<String> = Vec::new();
    let mut shared_skills = false;
    let mut planned_paths = BTreeSet::new();
    for component in components {
        for tool in &component.tools {
            if !roots.present(*tool) {
                if !skipped.contains(tool) {
                    skipped.push(*tool);
                }
                continue;
            }
            if component.kind == ComponentType::Skill && *tool == ToolKind::Codex {
                shared_skills = true;
            }
            let unit = roots.unit(component, *tool);
            let unit_taken = component.kind == ComponentType::Skill
                && std::fs::symlink_metadata(&unit).is_ok()
                && !our_dirs.contains(unit.to_string_lossy().as_ref())
                && !ours.keys().any(|path| Path::new(path).starts_with(&unit));
            let targets = roots.targets(component, *tool).map_err(Error::invalid)?;
            for (target, content) in targets {
                let key = target.to_string_lossy().into_owned();
                planned_paths.insert(key.clone());
                let after = sha_of(&content);
                let now = on_disk(&target);
                let mut file = PlannedFile {
                    component: component.id.clone(),
                    kind: component.kind,
                    name: component.name.clone(),
                    tool: *tool,
                    path: key.clone(),
                    display: roots.display(&target),
                    action: FileAction::Add,
                    content: Some(content.clone()),
                    diff: None,
                    reason: None,
                    before: None,
                    after: Some(after.clone()),
                    root: None,
                };
                let check_root = if target.starts_with(&roots.agents) {
                    roots.agents.clone()
                } else {
                    roots.agent(*tool).to_path_buf()
                };
                file.root = Some(check_root.to_string_lossy().into_owned());
                if let Some(other) = others.get(key.as_str()) {
                    file.action = FileAction::Conflict;
                    file.reason = Some(format!("other-pack:{other}"));
                } else if blocked_path(&target, &check_root) || now == OnDisk::Other {
                    file.action = FileAction::Conflict;
                    file.reason = Some("not-a-file".into());
                } else if let Some(record) = ours.get(key.as_str()) {
                    match &now {
                        OnDisk::Absent => {}
                        OnDisk::File(sha) if *sha == after => {
                            file.action = FileAction::Same;
                            file.content = None;
                            file.before = Some(sha.clone());
                        }
                        OnDisk::File(sha) if *sha == record.sha256 => {
                            file.action = FileAction::Update;
                            file.before = Some(sha.clone());
                            let old = std::fs::read_to_string(&target).unwrap_or_default();
                            file.diff = Some(crate::diff::unified_diff(
                                &old,
                                &content,
                                "installed",
                                "new",
                            ));
                        }
                        OnDisk::File(sha) => {
                            file.action = FileAction::KeepModified;
                            file.content = None;
                            file.before = Some(sha.clone());
                        }
                        OnDisk::Other => unreachable!(),
                    }
                } else if now != OnDisk::Absent || unit_taken {
                    file.action = FileAction::Conflict;
                    file.reason = Some("exists".into());
                }
                if file.action == FileAction::Add {
                    for dir in missing_dirs(&target) {
                        let dir = dir.to_string_lossy().into_owned();
                        if !new_dirs.contains(&dir) {
                            new_dirs.push(dir);
                        }
                    }
                }
                files.push(file);
            }
        }
    }
    // Files the installed version had that this one no longer writes.
    if let Some(previous) = previous {
        for record in &previous.files {
            // Planned again, or on an agent that is not here now: left as recorded.
            if planned_paths.contains(&record.path) || skipped.contains(&record.tool) {
                continue;
            }
            let now = on_disk(Path::new(&record.path));
            let (action, before) = match &now {
                OnDisk::Absent => continue,
                OnDisk::File(sha) if *sha == record.sha256 => {
                    (FileAction::Remove, Some(sha.clone()))
                }
                OnDisk::File(sha) => (FileAction::Release, Some(sha.clone())),
                OnDisk::Other => continue,
            };
            files.push(PlannedFile {
                component: record.component.clone(),
                kind: record.kind,
                name: record.name.clone(),
                tool: record.tool,
                path: record.path.clone(),
                display: roots.display(Path::new(&record.path)),
                action,
                content: None,
                diff: None,
                reason: None,
                before,
                after: None,
                root: None,
            });
        }
    }
    let blocked = files.iter().any(|f| f.action == FileAction::Conflict);
    let changes = files.iter().any(|f| {
        matches!(
            f.action,
            FileAction::Add | FileAction::Update | FileAction::Remove | FileAction::Release
        )
    });
    let same_version = previous.is_some_and(|p| p.version == version);
    let nothing_to_do = !changes && same_version;
    let mut plan = HarnessPlan {
        plan_id: String::new(),
        pack_id: pack_id.to_string(),
        version: version.to_string(),
        from_version: previous.map(|p| p.version.clone()),
        files,
        skipped_tools: skipped,
        shared_skills,
        blocked,
        nothing_to_do,
        new_dirs,
    };
    plan.plan_id = plan_id(&plan);
    Ok(plan)
}

fn plan_id(plan: &HarnessPlan) -> String {
    let files: Vec<Value> = plan
        .files
        .iter()
        .map(|f| serde_json::json!([f.path, f.action, f.before, f.after, f.component, f.tool]))
        .collect();
    let canonical = serde_json::json!([
        plan.pack_id,
        plan.version,
        plan.from_version,
        files,
        plan.new_dirs
    ]);
    hex::encode(Sha256::digest(canonical.to_string().as_bytes()))
}

/// What a confirmed change did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub added: u32,
    pub updated: u32,
    /// Files the person changed, left alone.
    pub kept: u32,
    pub removed: u32,
}

/// Carry out a plan the person confirmed. `expected_id` is the id of the plan they saw; any
/// difference between that and what the disk and pack say now refuses the change.
pub fn apply(
    store: &Store,
    source: &str,
    plan: &HarnessPlan,
    expected_id: &str,
) -> Result<ApplyReport> {
    if plan.plan_id != expected_id {
        return Err(Error::user_cancel("components:changed:"));
    }
    if plan.blocked {
        return Err(Error::invalid("components:conflict:"));
    }
    let _lock = HomeLock::acquire(store.paths())?;
    recover(store)?;
    let ledger = load_ledger(store);
    let previous = ledger.packs.get(&plan.pack_id).cloned();
    let mut report = ApplyReport::default();
    let mut ops = Vec::new();
    let mut entry_files = Vec::new();
    for file in &plan.files {
        let record = LedgerFile {
            component: file.component.clone(),
            kind: file.kind,
            name: file.name.clone(),
            tool: file.tool,
            path: file.path.clone(),
            sha256: file.after.clone().unwrap_or_default(),
            root: file.root.clone(),
        };
        match file.action {
            FileAction::Add | FileAction::Update => {
                if file.action == FileAction::Add {
                    report.added += 1;
                } else {
                    report.updated += 1;
                }
                ops.push(JournalOp {
                    path: file.path.clone(),
                    root: file.root.clone(),
                    before: file.before.clone(),
                    after: file.after.clone(),
                    content: file.content.clone(),
                });
                entry_files.push(record);
            }
            FileAction::Same => entry_files.push(record),
            FileAction::KeepModified => {
                report.kept += 1;
                // Still ours, at what we wrote: it keeps reading as changed by the person.
                let old = previous
                    .as_ref()
                    .and_then(|p| p.files.iter().find(|f| f.path == file.path))
                    .map(|f| f.sha256.clone())
                    .unwrap_or_default();
                entry_files.push(LedgerFile {
                    sha256: old,
                    ..record
                });
            }
            FileAction::Remove => {
                report.removed += 1;
                ops.push(JournalOp {
                    path: file.path.clone(),
                    root: None,
                    before: file.before.clone(),
                    after: None,
                    content: None,
                });
            }
            FileAction::Release => report.kept += 1,
            FileAction::Conflict => unreachable!(),
        }
    }
    // Files on agents that are not here now stay as they were recorded.
    if let Some(previous) = &previous {
        for record in &previous.files {
            if plan.skipped_tools.contains(&record.tool)
                && !plan.files.iter().any(|f| f.path == record.path)
            {
                entry_files.push(record.clone());
            }
        }
    }
    let mut dirs = previous
        .as_ref()
        .map(|p| p.dirs.clone())
        .unwrap_or_default();
    for dir in &plan.new_dirs {
        if !dirs.contains(dir) {
            dirs.push(dir.clone());
        }
    }
    let entry = LedgerPack {
        version: plan.version.clone(),
        source: source.to_string(),
        installed_at: previous
            .as_ref()
            .map(|p| p.installed_at.clone())
            .unwrap_or_else(now_rfc3339),
        files: entry_files,
        dirs,
    };
    let journal = Journal {
        pack_id: plan.pack_id.clone(),
        entry: Some(entry),
        ops,
    };
    // The preview was read without the lock. Under it, everything must still be as it was;
    // otherwise nothing is written and the person looks again.
    if !journal.ops.iter().all(still_as_planned) {
        return Err(Error::user_cancel("components:changed:"));
    }
    atomic_write(&journal_path(store), &serde_json::to_string(&journal)?)?;
    let failed = run_journal(store, &journal)?;
    if !failed.is_empty() {
        // Something changed in the moment between the check and the write. Those files were
        // left as they were and are not this app's; say so instead of counting them as done.
        for file in plan.files.iter().filter(|f| failed.contains(&f.path)) {
            match file.action {
                FileAction::Add => report.added -= 1,
                FileAction::Update => report.updated -= 1,
                FileAction::Remove => report.removed -= 1,
                _ => {}
            }
            report.kept += 1;
        }
    }
    Ok(report)
}

/// Take a pack's components out. Only files that still hold what this app wrote are removed.
pub fn uninstall(store: &Store, pack_id: &str) -> Result<Option<ApplyReport>> {
    let _lock = HomeLock::acquire(store.paths())?;
    recover(store)?;
    let mut ledger = load_ledger(store);
    let Some(pack) = ledger.packs.remove(pack_id) else {
        return Ok(None);
    };
    let mut report = ApplyReport::default();
    for file in &pack.files {
        let path = Path::new(&file.path);
        if file
            .root
            .as_deref()
            .is_some_and(|root| blocked_path(path, Path::new(root)))
        {
            // A link now leads somewhere else: what is there is not what this app wrote.
            report.kept += 1;
            continue;
        }
        match on_disk(path) {
            OnDisk::File(sha) if sha == file.sha256 => {
                std::fs::remove_file(&file.path)?;
                report.removed += 1;
            }
            OnDisk::Absent => {}
            _ => report.kept += 1,
        }
    }
    remove_empty_dirs(&pack.dirs);
    save_ledger(store, &ledger)?;
    Ok(Some(report))
}

/// Whether a pack is in the ledger.
pub fn is_installed(store: &Store, pack_id: &str) -> bool {
    load_ledger_readonly(store).packs.contains_key(pack_id)
}

/// The Codex skill folders this app put in the shared `~/.agents/skills`, as places under the
/// home folder (`.agents/skills/<name>`). Cleaning Codex takes them out with the rest of its
/// setup; they are outside `~/.codex`, so cleanup would not otherwise reach them.
pub fn codex_shared_skills(store: &Store, home: &Path) -> Vec<String> {
    let mut found: Vec<String> = load_ledger_readonly(store)
        .packs
        .values()
        .flat_map(|pack| pack.files.iter())
        .filter(|file| file.tool == ToolKind::Codex && file.kind == ComponentType::Skill)
        .filter_map(|file| {
            let relative = Path::new(&file.path).strip_prefix(home).ok()?;
            let parts: Vec<String> = relative
                .components()
                .take(3)
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            (parts.len() == 3 && parts[0] == ".agents" && parts[1] == "skills")
                .then(|| parts.join("/"))
        })
        .collect();
    found.sort();
    found.dedup();
    found
}
