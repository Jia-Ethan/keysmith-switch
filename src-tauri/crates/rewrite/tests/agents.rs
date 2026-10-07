//! Requests captured from real agents against a local mock upstream (see each fixture's
//! `_capture` note, where present). Each test applies one rule and checks the exact set of
//! strings that changed anywhere in the body.

use keysmith_rewrite::{rewrite_request, rewrite_request_bytes, Matcher, Protocol, Rule, Tool};
use pretty_assertions::assert_eq;
use serde_json::Value;

/// Every string in a JSON value, by path.
fn strings(value: &Value, path: String, out: &mut Vec<(String, String)>) {
    match value {
        Value::String(text) => out.push((path, text.clone())),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                strings(item, format!("{path}/{i}"), out);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                strings(item, format!("{path}/{key}"), out);
            }
        }
        _ => {}
    }
}

/// The (before, after) pairs of strings a rewrite changed.
fn changes(fixture: &str, protocol: Protocol, tool: Tool, rule: Rule) -> Vec<(String, String)> {
    let original: Value = serde_json::from_str(fixture).unwrap();
    let mut body = original.clone();
    let count = rewrite_request(protocol, tool, &Matcher::new(&[rule]), &mut body);
    let (mut before, mut after) = (Vec::new(), Vec::new());
    strings(&original, String::new(), &mut before);
    strings(&body, String::new(), &mut after);
    assert_eq!(before.len(), after.len());
    let diffs: Vec<_> = before
        .into_iter()
        .zip(after)
        .filter(|(a, b)| a.1 != b.1)
        .map(|(a, b)| {
            assert_eq!(a.0, b.0);
            (a.1, b.1)
        })
        .collect();
    assert_eq!(count, diffs.len());
    diffs
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

const CLAUDE: &str = include_str!("fixtures/claude-2.1.291-messages.json");

#[test]
fn claude_code_changes_only_typed_turns() {
    // Untouched: CLAUDE.md and other system reminders, the system prompt, assistant turns,
    // tool input and results, mid-conversation system messages, /greet and what it expanded
    // to, /usage and /context output, /init, bang-mode input and output, the Read result for
    // an @-mentioned file.
    assert_eq!(
        changes(
            CLAUDE,
            Protocol::Messages,
            Tool::Claude,
            Rule::new("提示词", "指令")
        ),
        vec![
            pair("RUNTOOL turn one 提示词", "RUNTOOL turn one 指令"),
            pair("describe @img.png 提示词", "describe @img.png 指令"),
            pair("turn three 提示词 @notes.txt", "turn three 指令 @notes.txt"),
        ]
    );
}

#[test]
fn claude_code_keeps_key_order_and_cache_control() {
    let matcher = Matcher::new(&[Rule::new("提示词", "指令")]);
    let out = rewrite_request_bytes(
        Protocol::Messages,
        Tool::Claude,
        &matcher,
        CLAUDE.as_bytes(),
    )
    .unwrap();
    let original: Value = serde_json::from_str(CLAUDE).unwrap();
    let rewritten: Value = serde_json::from_slice(&out).unwrap();
    let keys = |value: &Value| {
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&original), keys(&rewritten));
    // Same bytes apart from the rewritten words, so prompt caching sees the same prefix.
    let expected = serde_json::to_string(&original)
        .unwrap()
        .replace("RUNTOOL turn one 提示词", "RUNTOOL turn one 指令")
        .replace("describe @img.png 提示词", "describe @img.png 指令")
        .replace("turn three 提示词 @notes.txt", "turn three 指令 @notes.txt");
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

#[test]
fn claude_code_skips_interrupt_markers_and_compaction() {
    let m = Matcher::new(&[Rule::new("x", "y")]);
    let mut body = serde_json::json!({"messages": [
        {"role": "user", "content": [{"type": "text", "text": "[Request interrupted by user] x"}]},
        {"role": "user", "content": "This session is being continued from a previous conversation x"},
        {"role": "user", "content": [
            {"type": "text", "text": "<command-message>c</command-message>\n<command-name>/c</command-name>\n<command-args>x</command-args>"},
            {"type": "text", "text": "template with x"},
            {"type": "text", "text": "typed x"}
        ]},
        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "x"}]},
        {"role": "system", "content": "x"},
        {"role": "assistant", "content": "x"}
    ]});
    assert_eq!(
        rewrite_request(Protocol::Messages, Tool::Claude, &m, &mut body),
        1
    );
    assert_eq!(body["messages"][2]["content"][1]["text"], "template with x");
    assert_eq!(body["messages"][2]["content"][2]["text"], "typed y");
}

#[test]
fn zcode_messages_skip_reminders_and_slash_commands() {
    assert_eq!(
        changes(
            include_str!("fixtures/zcode-0.16.9-anthropic-messages.json"),
            Protocol::Messages,
            Tool::Zcode,
            Rule::new("KSMARK", "MARKED")
        ),
        vec![
            pair("anth turn1 KSMARK5", "anth turn1 MARKED5"),
            pair("anth turn3 KSMARK7", "anth turn3 MARKED7"),
        ]
    );
}

#[test]
fn zcode_chat_completions_skip_reminders_and_skill_turns() {
    assert_eq!(
        changes(
            include_str!("fixtures/zcode-0.16.9-openai-chat-completions.json"),
            Protocol::Chat,
            Tool::Zcode,
            Rule::new("KSMARK", "MARKED")
        ),
        vec![
            pair("first turn KSMARK1", "first turn MARKED1"),
            pair("second turn KSMARK2", "second turn MARKED2"),
        ]
    );
}

#[test]
fn zcode_responses_skip_reminders() {
    assert_eq!(
        changes(
            include_str!("fixtures/zcode-0.16.9-openai-responses.json"),
            Protocol::Responses,
            Tool::Zcode,
            Rule::new("KSMARK", "MARKED")
        ),
        vec![pair("resp turn KSMARK4", "resp turn MARKED4")]
    );
}

#[test]
fn zcode_custom_command_expansion_is_skipped() {
    let m = Matcher::new(&[Rule::new("x", "y")]);
    let mut body = serde_json::json!({"messages": [
        {"role": "user", "content": "Run custom command /c.\nCommand source: user/zcode.\n\nbody x"},
        {"role": "user", "content": "plain x"}
    ]});
    assert_eq!(
        rewrite_request(Protocol::Chat, Tool::Zcode, &m, &mut body),
        1
    );
    assert_eq!(body["messages"][1]["content"], "plain y");
}

const GROK: &str = include_str!("fixtures/grok-1.0.46-responses.json");

#[test]
fn grok_changes_only_the_typed_query() {
    // Untouched: <user_info>/<rules>, the skills reminder, tool calls and output, assistant
    // turns, the @-file contents and skill expansion Grok appends after a query, and /kscmd
    // (a slash command, skipped whole).
    let body: Value = serde_json::from_str(GROK).unwrap();
    let first = body["input"][3]["content"].as_str().unwrap().to_string();
    assert_eq!(
        changes(
            GROK,
            Protocol::Responses,
            Tool::Grok,
            Rule::new("TYPED_TURN", "WORDS_TURN")
        ),
        vec![
            (
                first.clone(),
                first.replacen("TYPED_TURN1", "WORDS_TURN1", 1)
            ),
            pair(
                "<user_query>\nTYPED_TURN2 second message\n</user_query>",
                "<user_query>\nWORDS_TURN2 second message\n</user_query>"
            ),
        ]
    );
}

#[test]
fn grok_finds_the_real_end_of_a_query_with_a_literal_closing_tag() {
    let m = Matcher::new(&[Rule::new("line", "row")]);
    let text = "<user_query>\nline1 </user_query> <x> & <b>\nline2 @note.txt\n</user_query>\n\n<system-reminder>\nline in a file\n</system-reminder>";
    let mut body = serde_json::json!({"input": [{"role": "user", "content": text}]});
    assert_eq!(
        rewrite_request(Protocol::Responses, Tool::Grok, &m, &mut body),
        1
    );
    assert_eq!(
        body["input"][0]["content"],
        "<user_query>\nrow1 </user_query> <x> & <b>\nrow2 @note.txt\n</user_query>\n\n<system-reminder>\nline in a file\n</system-reminder>"
    );
}

#[test]
fn grok_leaves_its_own_context_and_unwrapped_text_alone() {
    let m = Matcher::new(&[Rule::new("x", "y")]);
    let mut body = serde_json::json!({"input": [
        {"role": "user", "content": "<user_info>\nx\n</user_info>"},
        {"role": "user", "content": "<system-reminder>\nx\n</system-reminder>"},
        {"role": "user", "content": "x without a wrapper (--verbatim)"},
        {"role": "user", "content": "<user_query>\n/cmd x\n</user_query>"},
        {"role": "user", "content": "<user_query>\nx but never closed"}
    ]});
    assert_eq!(
        rewrite_request(Protocol::Responses, Tool::Grok, &m, &mut body),
        0
    );
}

#[test]
fn grok_chat_completions_use_the_same_wrapper() {
    let m = Matcher::new(&[Rule::new("提示词", "指令")]);
    let mut body = serde_json::json!({"messages": [
        {"role": "system", "content": "提示词"},
        {"role": "user", "content": "<user_query>\nhello 提示词\n</user_query>"},
        {"role": "assistant", "content": "提示词"}
    ]});
    assert_eq!(
        rewrite_request(Protocol::Chat, Tool::Grok, &m, &mut body),
        1
    );
    assert_eq!(
        body["messages"][1]["content"],
        "<user_query>\nhello 指令\n</user_query>"
    );
}

#[test]
fn protocol_by_path() {
    assert_eq!(Protocol::from_path("v1/messages"), Some(Protocol::Messages));
    assert_eq!(
        Protocol::from_path("v1/messages/count_tokens"),
        Some(Protocol::Messages)
    );
    assert_eq!(
        Protocol::from_path("v1/chat/completions"),
        Some(Protocol::Chat)
    );
    assert_eq!(Protocol::from_path("responses/"), Some(Protocol::Responses));
    assert_eq!(Protocol::from_path("responses/compact"), None);
    assert_eq!(Protocol::from_path("v1/models"), None);
}
