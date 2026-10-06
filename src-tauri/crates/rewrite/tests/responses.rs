use keysmith_rewrite::responses::{rewrite_request, rewrite_request_bytes, rewrite_user_text};
use keysmith_rewrite::{Matcher, Rule};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/codex-0.144-responses.json");

fn matcher() -> Matcher {
    Matcher::new(&[Rule::new("提示词", "指令")])
}

fn texts(body: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for item in body["input"].as_array().unwrap() {
        if let Some(parts) = item["content"].as_array() {
            for part in parts {
                out.push(part["text"].as_str().unwrap_or_default().to_string());
            }
        }
        if let Some(output) = item["output"].as_str() {
            out.push(output.to_string());
        }
        if let Some(arguments) = item["arguments"].as_str() {
            out.push(arguments.to_string());
        }
    }
    out
}

#[test]
fn codex_request_changes_only_what_the_person_typed() {
    let original: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut body = original.clone();
    let changed = rewrite_request(&matcher(), &mut body);
    // The earlier user turn, the IDE-wrapped request and the current turn.
    assert_eq!(changed, 3);

    let before = texts(&original);
    let after = texts(&body);
    let diffs: Vec<(String, String)> = before
        .iter()
        .zip(&after)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a.clone(), b.clone()))
        .collect();
    assert_eq!(
        diffs,
        vec![
            ("上一轮说的提示词".into(), "上一轮说的指令".into()),
            (
                "# Context from my IDE setup:\n\n## My request for Codex:\n提示词".into(),
                "# Context from my IDE setup:\n\n## My request for Codex:\n指令".into()
            ),
            (
                "把提示词改一下 /not-a-command".into(),
                "把指令改一下 /not-a-command".into()
            ),
        ]
    );

    // Everything outside input[] user text is untouched.
    let mut stripped_before = original.clone();
    let mut stripped_after = body.clone();
    stripped_before["input"] = Value::Null;
    stripped_after["input"] = Value::Null;
    assert_eq!(stripped_before, stripped_after);
}

#[test]
fn injected_context_and_slash_commands_are_skipped() {
    let m = matcher();
    assert_eq!(
        rewrite_user_text(&m, "<environment_context>提示词</environment_context>"),
        None
    );
    assert_eq!(
        rewrite_user_text(&m, "  # AGENTS.md instructions 提示词"),
        None
    );
    assert_eq!(rewrite_user_text(&m, "/review 提示词"), None);
    assert_eq!(
        rewrite_user_text(&m, "# Files mentioned by the user:\n提示词\n"),
        None,
        "wrapped context without a request heading is left whole"
    );
}

#[test]
fn wrapped_context_rewrites_only_after_the_last_request_heading() {
    let m = matcher();
    let text = "# Files mentioned by the user:\n\n## 提示词.md: /x\n\n## My request:\n改提示词";
    assert_eq!(
        rewrite_user_text(&m, text).unwrap(),
        "# Files mentioned by the user:\n\n## 提示词.md: /x\n\n## My request:\n改指令"
    );
}

#[test]
fn string_content_and_non_message_items() {
    let mut body = json!({
        "input": [
            {"role": "user", "content": "提示词"},
            {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "提示词"}]},
            {"type": "message", "role": "user", "content": [{"type": "input_image", "image_url": "data:提示词"}]},
            {"type": "reasoning", "summary": [{"type": "summary_text", "text": "提示词"}]}
        ]
    });
    assert_eq!(rewrite_request(&matcher(), &mut body), 1);
    assert_eq!(body["input"][0]["content"], "指令");
    assert_eq!(body["input"][1]["content"][0]["text"], "提示词");
    assert_eq!(body["input"][2]["content"][0]["image_url"], "data:提示词");
}

#[test]
fn bytes_api_passes_through_when_nothing_changes() {
    let m = matcher();
    assert_eq!(rewrite_request_bytes(&m, b"not json"), None);
    assert_eq!(
        rewrite_request_bytes(&m, br#"{"input":[{"role":"user","content":"hi"}]}"#),
        None
    );
    let out = rewrite_request_bytes(
        &m,
        "{\"input\":[{\"role\":\"user\",\"content\":\"提示词\"}]}".as_bytes(),
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["input"][0]["content"], "指令");
}

#[test]
fn rewriting_is_deterministic_across_turns() {
    // History is resent each turn; the same text must come out the same every time.
    let m = matcher();
    let mut first: Value = serde_json::from_str(FIXTURE).unwrap();
    let mut second: Value = serde_json::from_str(FIXTURE).unwrap();
    rewrite_request(&m, &mut first);
    rewrite_request(&m, &mut second);
    assert_eq!(first, second);
}
