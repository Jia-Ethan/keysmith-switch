//! Input rewrite: literal word replacement applied to what a person types, after they
//! press enter and before the model sees it.
//!
//! Matching is literal and case-sensitive, with no Unicode normalisation. The scan runs left
//! to right; at any position the longest rule wins, and text a rule produced is never
//! scanned again. A message that starts with `/` (after leading whitespace) is a slash
//! command and is left whole.
//!
//! The app owns the rules and writes them out as a [`Snapshot`]; the relay only reads it.

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

/// What the app hands the relay: the switches and the rules already merged in priority order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema: u32,
    /// The master switch.
    pub enabled: bool,
    pub tools: ToolSwitches,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSwitches {
    #[serde(default)]
    pub codex: bool,
}

impl Snapshot {
    /// The matcher for Codex, or `None` when any switch on the way is off.
    pub fn codex_matcher(&self) -> Option<Matcher> {
        if self.schema != SNAPSHOT_SCHEMA || !self.enabled || !self.tools.codex {
            return None;
        }
        let matcher = Matcher::new(&self.rules);
        (!matcher.is_empty()).then_some(matcher)
    }

    /// A snapshot that cannot be read rewrites nothing.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .filter(|snapshot| snapshot.schema == SNAPSHOT_SCHEMA)
    }
}
