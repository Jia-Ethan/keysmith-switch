//! Rewriting an OpenAI Responses request body as Codex sends it.
//!
//! Only what the person typed changes: `input_text` parts of `role: "user"` messages. Codex
//! also sends its own context as user messages (environment, AGENTS.md, skills, IDE context);
//! those are recognised by their opening line and left alone, except that IDE and mention
//! context carry the person's words after a "## My request" heading, and only that part is
//! rewritten. `instructions`, developer, assistant, tool and reasoning items never change.

use serde_json::Value;

use crate::Matcher;

/// Opening lines of user messages Codex writes itself. Checked against the start of the text
/// after leading whitespace.
const INJECTED_PREFIXES: &[&str] = &[
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
/// starts with [`REQUEST_HEADING`].
const WRAPPED_PREFIXES: &[&str] = &[
    "# Context from my IDE setup:",
    "# Files mentioned by the user:",
    "# Applications mentioned by the user:",
];
const REQUEST_HEADING: &str = "## My request";

/// Rewrite one user text part, or `None` when nothing changes.
pub fn rewrite_user_text(matcher: &Matcher, text: &str) -> Option<String> {
    let head = text.trim_start();
    if INJECTED_PREFIXES
        .iter()
        .any(|prefix| head.starts_with(prefix))
    {
        return None;
    }
    if WRAPPED_PREFIXES
        .iter()
        .any(|prefix| head.starts_with(prefix))
    {
        let heading = request_heading_end(text)?;
        let (context, request) = text.split_at(heading);
        let rewritten = matcher.rewrite(request);
        return match rewritten {
            std::borrow::Cow::Borrowed(_) => None,
            std::borrow::Cow::Owned(request) => Some(format!("{context}{request}")),
        };
    }
    match matcher.rewrite(text) {
        std::borrow::Cow::Borrowed(_) => None,
        std::borrow::Cow::Owned(out) => Some(out),
    }
}

/// Byte offset just past the line break that ends the last "## My request…" heading line.
fn request_heading_end(text: &str) -> Option<usize> {
    let mut found = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with(REQUEST_HEADING) {
            found = Some(offset + line.len());
        }
        offset += line.len();
    }
    found
}

/// Rewrite every user text part of a Responses request in place. Returns how many parts
/// changed.
pub fn rewrite_request(matcher: &Matcher, body: &mut Value) -> usize {
    let Some(items) = body.get_mut("input").and_then(Value::as_array_mut) else {
        return 0;
    };
    let mut changed = 0;
    for item in items {
        let is_user_message = item.get("role").and_then(Value::as_str) == Some("user")
            && item
                .get("type")
                .and_then(Value::as_str)
                .is_none_or(|kind| kind == "message");
        if !is_user_message {
            continue;
        }
        match item.get_mut("content") {
            Some(Value::String(text)) => {
                if let Some(out) = rewrite_user_text(matcher, text) {
                    *text = out;
                    changed += 1;
                }
            }
            Some(Value::Array(parts)) => {
                for part in parts {
                    if part.get("type").and_then(Value::as_str) != Some("input_text") {
                        continue;
                    }
                    if let Some(Value::String(text)) = part.get_mut("text") {
                        if let Some(out) = rewrite_user_text(matcher, text) {
                            *text = out;
                            changed += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    changed
}

/// Rewrite a request body given as bytes. `None` means forward the original bytes: nothing
/// changed, or the body is not JSON this understands.
pub fn rewrite_request_bytes(matcher: &Matcher, bytes: &[u8]) -> Option<Vec<u8>> {
    let mut body: Value = serde_json::from_slice(bytes).ok()?;
    if rewrite_request(matcher, &mut body) == 0 {
        return None;
    }
    serde_json::to_vec(&body).ok()
}
