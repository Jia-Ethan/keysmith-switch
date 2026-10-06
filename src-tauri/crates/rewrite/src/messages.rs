//! Rewriting an Anthropic Messages request body (`/v1/messages` and its `count_tokens`).
//!
//! Only `text` blocks (or string content) of `role: "user"` messages change, and only where
//! the agent's dialect says the person typed them. `system`, mid-conversation `role:
//! "system"` messages, assistant turns, `tool_result`, image and document blocks never
//! change. Other block fields, such as `cache_control`, stay as they are.

use serde_json::Value;

use crate::{Matcher, Tool};

pub fn rewrite_request(tool: Tool, matcher: &Matcher, body: &mut Value) -> usize {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return 0;
    };
    let mut changed = 0;
    for message in messages {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        if let Some(content) = message.get_mut("content") {
            changed += crate::rewrite_content(tool, matcher, content, "text");
        }
    }
    changed
}
