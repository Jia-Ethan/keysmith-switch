//! Rewriting an OpenAI Chat Completions request body.
//!
//! Only string content or `text` parts of `role: "user"` messages change, and only where the
//! agent's dialect says the person typed them. System, developer, assistant and tool messages
//! never change.

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
