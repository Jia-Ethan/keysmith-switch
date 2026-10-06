//! Input rewrite: literal word replacement applied to what a person types, after they
//! press enter and before the model sees it.
//!
//! Matching is literal and case-sensitive, with no Unicode normalisation. The scan runs left
//! to right; at any position the longest rule wins, and text a rule produced is never
//! scanned again. A message that starts with `/` (after leading whitespace) is a slash
//! command and is left whole.
//!
//! The app owns the rules and writes them out as a [`Snapshot`]; the relay only reads it.
//!
//! Which text counts as typed depends on the agent ([`dialect`]); where that text sits in a
//! request depends on the wire format ([`responses`], [`messages`], [`chat`]).

pub mod chat;
pub mod dialect;
pub mod messages;
pub mod responses;

use std::collections::BTreeMap;

use std::borrow::Cow;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use serde::{Deserialize, Serialize};

pub const SNAPSHOT_SCHEMA: u32 = 1;
pub const MAX_RULES_PER_TABLE: usize = 1000;
pub const MAX_FROM_CHARS: usize = 200;
pub const MAX_TO_CHARS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub from: String,
    pub to: String,
}

impl Rule {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    EmptyFrom,
    FromTooLong,
    ToTooLong,
    ControlCharacter,
    Duplicate(String),
    TooMany,
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFrom => write!(f, "a rule must match some text"),
            Self::FromTooLong => {
                write!(f, "matched text is longer than {MAX_FROM_CHARS} characters")
            }
            Self::ToTooLong => write!(f, "replacement is longer than {MAX_TO_CHARS} characters"),
            Self::ControlCharacter => write!(f, "matched text contains a control character"),
            Self::Duplicate(from) => write!(f, "two rules match the same text: {from}"),
            Self::TooMany => write!(f, "a table holds at most {MAX_RULES_PER_TABLE} rules"),
        }
    }
}

impl std::error::Error for RuleError {}

/// One rule on its own. Newlines and tabs may appear in a replacement but not in the matched
/// text, where they would be invisible in the editor.
pub fn validate_rule(rule: &Rule) -> Result<(), RuleError> {
    if rule.from.is_empty() {
        return Err(RuleError::EmptyFrom);
    }
    if rule.from.chars().count() > MAX_FROM_CHARS {
        return Err(RuleError::FromTooLong);
    }
    if rule.to.chars().count() > MAX_TO_CHARS {
        return Err(RuleError::ToTooLong);
    }
    if rule.from.chars().any(char::is_control) {
        return Err(RuleError::ControlCharacter);
    }
    Ok(())
}

/// A whole table: every rule valid, no two with the same matched text.
pub fn validate_table(rules: &[Rule]) -> Result<(), RuleError> {
    if rules.len() > MAX_RULES_PER_TABLE {
        return Err(RuleError::TooMany);
    }
    let mut seen = std::collections::HashSet::new();
    for rule in rules {
        validate_rule(rule)?;
        if !seen.insert(rule.from.as_str()) {
            return Err(RuleError::Duplicate(rule.from.clone()));
        }
    }
    Ok(())
}

/// True for a message the rewrite must leave whole.
pub fn is_slash_command(text: &str) -> bool {
    text.trim_start().starts_with('/')
}

#[derive(Debug, Clone)]
pub struct Matcher {
    automaton: Option<AhoCorasick>,
    replacements: Vec<String>,
}

impl Matcher {
    /// Rules in priority order. When two share the same matched text the first one wins;
    /// rules that match nothing are dropped.
    pub fn new<'a>(rules: impl IntoIterator<Item = &'a Rule>) -> Self {
        let mut seen = std::collections::HashSet::new();
        let mut patterns = Vec::new();
        let mut replacements = Vec::new();
        for rule in rules {
            if rule.from.is_empty() || !seen.insert(rule.from.clone()) {
                continue;
            }
            patterns.push(rule.from.clone());
            replacements.push(rule.to.clone());
        }
        let automaton = if patterns.is_empty() {
            None
        } else {
            AhoCorasickBuilder::new()
                .match_kind(MatchKind::LeftmostLongest)
                .build(&patterns)
                .ok()
        };
        Self {
            automaton,
            replacements,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.automaton.is_none()
    }

    /// The text with every rule applied, or the text unchanged when nothing matched or it is
    /// a slash command.
    pub fn rewrite<'t>(&self, text: &'t str) -> Cow<'t, str> {
        let Some(automaton) = &self.automaton else {
            return Cow::Borrowed(text);
        };
        if is_slash_command(text) {
            return Cow::Borrowed(text);
        }
        let mut matches = automaton.find_iter(text).peekable();
        if matches.peek().is_none() {
            return Cow::Borrowed(text);
        }
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for found in matches {
            out.push_str(&text[last..found.start()]);
            out.push_str(&self.replacements[found.pattern().as_usize()]);
            last = found.end();
        }
        out.push_str(&text[last..]);
        Cow::Owned(out)
    }
}

/// The agents input rewrite knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Claude,
    Codex,
    Grok,
    Zcode,
}

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::Claude, Tool::Codex, Tool::Grok, Tool::Zcode];

    pub fn as_str(self) -> &'static str {
        match self {
            Tool::Claude => "claude",
            Tool::Codex => "codex",
            Tool::Grok => "grok",
            Tool::Zcode => "zcode",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.as_str() == text)
    }
}

/// What the app hands the relay: the switches and the rules already merged in priority order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema: u32,
    /// The master switch.
    pub enabled: bool,
    pub tools: ToolSwitches,
    /// The rules for Codex, and for every agent missing from `by_tool`. A v0.4.0 relay reads
    /// only this list.
    pub rules: Vec<Rule>,
    /// Rules per agent, when rule tables apply to some agents only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_tool: Option<BTreeMap<Tool, Vec<Rule>>>,
}

/// One switch per agent. A v0.4.0 snapshot has only `codex`; the others read as off.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSwitches {
    #[serde(default)]
    pub codex: bool,
    #[serde(default)]
    pub claude: bool,
    #[serde(default)]
    pub grok: bool,
    #[serde(default)]
    pub zcode: bool,
}

impl ToolSwitches {
    pub fn get(&self, tool: Tool) -> bool {
        match tool {
            Tool::Claude => self.claude,
            Tool::Codex => self.codex,
            Tool::Grok => self.grok,
            Tool::Zcode => self.zcode,
        }
    }
}

impl Snapshot {
    /// The matcher for one agent, or `None` when any switch on the way is off or it has no
    /// rules.
    pub fn matcher(&self, tool: Tool) -> Option<Matcher> {
        if self.schema != SNAPSHOT_SCHEMA || !self.enabled || !self.tools.get(tool) {
            return None;
        }
        let rules = self
            .by_tool
            .as_ref()
            .and_then(|by_tool| by_tool.get(&tool))
            .unwrap_or(&self.rules);
        let matcher = Matcher::new(rules);
        (!matcher.is_empty()).then_some(matcher)
    }

    pub fn codex_matcher(&self) -> Option<Matcher> {
        self.matcher(Tool::Codex)
    }

    /// A snapshot that cannot be read rewrites nothing.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .filter(|snapshot| snapshot.schema == SNAPSHOT_SCHEMA)
    }
}

/// Rewrite a message's `content`: a plain string, or an array of parts where only parts of
/// type `text_type` are text the person may have typed. Returns how many parts changed.
pub(crate) fn rewrite_content(
    tool: Tool,
    matcher: &Matcher,
    content: &mut serde_json::Value,
    text_type: &str,
) -> usize {
    use serde_json::Value;
    match content {
        Value::String(text) => match dialect::rewrite_text(tool, matcher, text, None) {
            Some(out) => {
                *text = out;
                1
            }
            None => 0,
        },
        Value::Array(parts) => {
            let mut changed = 0;
            let mut previous: Option<String> = None;
            for part in parts {
                let is_text = part.get("type").and_then(Value::as_str) == Some(text_type);
                let Some(Value::String(text)) = part.get_mut("text").filter(|_| is_text) else {
                    previous = None;
                    continue;
                };
                let original = text.clone();
                if let Some(out) = dialect::rewrite_text(tool, matcher, text, previous.as_deref()) {
                    *text = out;
                    changed += 1;
                }
                previous = Some(original);
            }
            changed
        }
        _ => 0,
    }
}

/// The request formats the relay can rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// OpenAI Responses, `POST …/responses`.
    Responses,
    /// Anthropic Messages, `POST …/messages` and `…/messages/count_tokens`.
    Messages,
    /// OpenAI Chat Completions, `POST …/chat/completions`.
    Chat,
}

impl Protocol {
    /// The format of a request by its path (without the query), or `None` for a request that
    /// carries no conversation to rewrite (`/responses/compact`, `/models`, …).
    pub fn from_path(path: &str) -> Option<Self> {
        let path = path.trim_end_matches('/');
        if path.ends_with("chat/completions") {
            Some(Self::Chat)
        } else if path.ends_with("messages") || path.ends_with("messages/count_tokens") {
            Some(Self::Messages)
        } else if path.ends_with("responses") {
            Some(Self::Responses)
        } else {
            None
        }
    }
}

/// Rewrite a request body in place. Returns how many text parts changed.
pub fn rewrite_request(
    protocol: Protocol,
    tool: Tool,
    matcher: &Matcher,
    body: &mut serde_json::Value,
) -> usize {
    match protocol {
        Protocol::Responses => responses::rewrite_request(tool, matcher, body),
        Protocol::Messages => messages::rewrite_request(tool, matcher, body),
        Protocol::Chat => chat::rewrite_request(tool, matcher, body),
    }
}

/// Rewrite a request body given as bytes. `None` means forward the original bytes: nothing
/// changed, or the body is not JSON this understands. Keys keep their order.
pub fn rewrite_request_bytes(
    protocol: Protocol,
    tool: Tool,
    matcher: &Matcher,
    bytes: &[u8],
) -> Option<Vec<u8>> {
    let mut body: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if rewrite_request(protocol, tool, matcher, &mut body) == 0 {
        return None;
    }
    serde_json::to_vec(&body).ok()
}
