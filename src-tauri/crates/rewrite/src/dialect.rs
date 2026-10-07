//! What counts as typed, per agent.
//!
//! Every agent puts its own context into user turns next to what the person typed. A dialect
//! looks at one piece of user text and returns the rewritten text, or `None` to leave it
//! alone: injected context, slash commands and what they expand to, and anything this module
//! cannot place.

use std::borrow::Cow;

use crate::{Matcher, Tool};

/// Rewrite one text part of a user turn for `tool`. `previous` is the text part just before
/// it in the same message, when there is one.
pub fn rewrite_text(
    tool: Tool,
    matcher: &Matcher,
    text: &str,
    previous: Option<&str>,
) -> Option<String> {
    match tool {
        Tool::Codex => codex(matcher, text),
        Tool::Claude => claude(matcher, text, previous),
        Tool::Zcode => zcode(matcher, text),
        Tool::Grok => grok(matcher, text),
    }
}

fn owned(text: Cow<'_, str>) -> Option<String> {
    match text {
        Cow::Borrowed(_) => None,
        Cow::Owned(out) => Some(out),
    }
}

fn starts_with_any(text: &str, prefixes: &[&str]) -> bool {
    let head = text.trim_start();
    prefixes.iter().any(|prefix| head.starts_with(prefix))
}

/// True for text that opens with a lowercase tag such as `<system-reminder>` or
/// `<command-name>`. Claude Code tells its own context apart from typed text the same way.
fn starts_with_tag(text: &str) -> bool {
    let Some(rest) = text.trim_start().strip_prefix('<') else {
        return false;
    };
    let mut chars = rest.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_lowercase()) {
        return false;
    }
    for c in chars {
        if c.is_ascii_whitespace() || c == '>' {
            return true;
        }
        if !(c.is_alphanumeric() || c == '_' || c == '-') {
            return false;
        }
    }
    false
}

// ----- Codex ------------------------------------------------------------------------

/// Opening lines of user messages Codex writes itself.
const CODEX_INJECTED: &[&str] = &[
    "<environment_context>",
    "<user_instructions>",
    "<user_shell_command>",
    "<turn_aborted>",
    "<subagent_notification>",
    "<startup_context>",
    "<skill>",
    "<skills_instructions>",
    "<apps_instructions>",
    "<plugins_instructions>",
    "<permissions instructions>",
    "<collaboration_mode>",
    "<model_switch>",
    "<personality_spec>",
    "<realtime_conversation>",
    "<realtime_delegation>",
    "# AGENTS.md instructions",
];

/// Context Codex wraps around the person's words. The words follow the last heading that
/// starts with [`CODEX_REQUEST_HEADING`].
const CODEX_WRAPPED: &[&str] = &[
    "# Context from my IDE setup:",
    "# Files mentioned by the user:",
    "# Applications mentioned by the user:",
];
const CODEX_REQUEST_HEADING: &str = "## My request";

fn codex(matcher: &Matcher, text: &str) -> Option<String> {
    if starts_with_any(text, CODEX_INJECTED) {
        return None;
    }
    if starts_with_any(text, CODEX_WRAPPED) {
        let heading = request_heading_end(text)?;
        let (context, request) = text.split_at(heading);
        return owned(matcher.rewrite(request)).map(|request| format!("{context}{request}"));
    }
    owned(matcher.rewrite(text))
}

/// Byte offset just past the line break that ends the last "## My request…" heading line.
fn request_heading_end(text: &str) -> Option<usize> {
    let mut found = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with(CODEX_REQUEST_HEADING) {
            found = Some(offset + line.len());
        }
        offset += line.len();
    }
    found
}

// ----- Claude Code ------------------------------------------------------------------

/// Text Claude Code writes into user turns that does not open with a tag.
const CLAUDE_INJECTED: &[&str] = &[
    "[Request interrupted by user",
    // The summary that replaces history after /compact, and the compaction prompt itself.
    "This session is being continued from a previous conversation",
    "CRITICAL: Respond with TEXT ONLY.",
];

fn claude(matcher: &Matcher, text: &str, previous: Option<&str>) -> Option<String> {
    if starts_with_tag(text) || starts_with_any(text, CLAUDE_INJECTED) {
        return None;
    }
    // A slash command arrives as a `<command-name>` part followed by what it expanded to.
    if previous.is_some_and(|previous| previous.contains("<command-name>")) {
        return None;
    }
    owned(matcher.rewrite(text))
}

// ----- ZCode ------------------------------------------------------------------------

/// What ZCode sends in place of a slash command on the turn it is typed.
const ZCODE_EXPANDED: &[&str] = &["Run custom command /", "Use the skill named `"];

fn zcode(matcher: &Matcher, text: &str) -> Option<String> {
    if starts_with_tag(text) || starts_with_any(text, ZCODE_EXPANDED) {
        return None;
    }
    owned(matcher.rewrite(text))
}

// ----- Grok Build -------------------------------------------------------------------

const GROK_OPEN: &str = "<user_query>\n";
const GROK_CLOSE: &str = "\n</user_query>";
/// Blocks Grok appends after the query in the same message: @-file contents and the skill a
/// custom command expanded to.
const GROK_APPENDED: &[&str] = &["<system-reminder>", "<skill_information>"];

/// Grok wraps what the person typed as `<user_query>\n…\n</user_query>` and may append
/// blocks after it. Typed text is not escaped, so a literal `</user_query>` can appear inside:
/// the query ends at the last closing tag that only Grok's own blocks (or nothing) follow.
/// Everything else in a user turn (`<user_info>`, `<system-reminder>`, summaries) is Grok's.
fn grok(matcher: &Matcher, text: &str) -> Option<String> {
    let body = text.strip_prefix(GROK_OPEN)?;
    let mut end = None;
    for (at, _) in body.match_indices(GROK_CLOSE) {
        let rest = body[at + GROK_CLOSE.len()..].trim_start();
        if rest.is_empty() || GROK_APPENDED.iter().any(|block| rest.starts_with(block)) {
            end = Some(at);
        }
    }
    let end = end?;
    let typed = &body[..end];
    let rewritten = owned(matcher.rewrite(typed))?;
    Some(format!("{GROK_OPEN}{rewritten}{}", &body[end..]))
}

#[cfg(test)]
mod tests {
    use super::starts_with_tag;

    #[test]
    fn tags() {
        assert!(starts_with_tag("<system-reminder>\nx"));
        assert!(starts_with_tag("  <command-name>/x</command-name>"));
        assert!(starts_with_tag("<session id=\"1\">"));
        assert!(!starts_with_tag("<3 you"));
        assert!(!starts_with_tag("<Div>"));
        assert!(!starts_with_tag("a <b>"));
        assert!(!starts_with_tag("<abc"));
    }
}
