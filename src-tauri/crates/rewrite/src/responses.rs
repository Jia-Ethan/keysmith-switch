//! Rewriting an OpenAI Responses request body.
//!
//! Only `input_text` parts (or string content) of `role: "user"` messages change, and only
//! where the agent's dialect says the person typed them. `instructions`, system, developer,
//! assistant, tool and reasoning items never change.

use serde_json::Value;

use crate::dialect::rewrite_text;
use crate::{Matcher, Tool};

/// Rewrite every user text part of a Responses request in place. Returns how many parts
/// changed.
pub fn rewrite_request(tool: Tool, matcher: &Matcher, body: &mut Value) -> usize {
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
        if let Some(content) = item.get_mut("content") {
            changed += crate::rewrite_content(tool, matcher, content, "input_text");
        }
    }
    changed
}

/// Codex's own words for one text part, kept for callers that only know Codex.
pub fn rewrite_user_text(matcher: &Matcher, text: &str) -> Option<String> {
    rewrite_text(Tool::Codex, matcher, text, None)
}

/// A Codex Responses body given as bytes; see [`crate::rewrite_request_bytes`].
pub fn rewrite_request_bytes(matcher: &Matcher, bytes: &[u8]) -> Option<Vec<u8>> {
    crate::rewrite_request_bytes(crate::Protocol::Responses, Tool::Codex, matcher, bytes)
}
