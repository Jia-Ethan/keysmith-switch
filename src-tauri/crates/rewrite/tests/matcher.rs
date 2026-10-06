use keysmith_rewrite::{
    is_slash_command, validate_table, Matcher, Rule, RuleError, Snapshot, ToolSwitches,
    SNAPSHOT_SCHEMA,
};
use pretty_assertions::assert_eq;

fn rewrite(rules: &[(&str, &str)], text: &str) -> String {
    let rules: Vec<Rule> = rules.iter().map(|(f, t)| Rule::new(*f, *t)).collect();
    Matcher::new(&rules).rewrite(text).into_owned()
}

#[test]
fn replaces_literally_and_case_sensitively() {
    assert_eq!(
        rewrite(&[("cat", "dog")], "cat Cat CAT cats"),
        "dog Cat CAT dogs"
    );
}

#[test]
fn longest_match_wins_at_the_same_position() {
    let rules = [("ab", "X"), ("abc", "Y"), ("a", "Z")];
    assert_eq!(rewrite(&rules, "abcd ab a"), "Yd X Z");
}

#[test]
fn leftmost_match_wins_over_a_longer_later_one() {
    assert_eq!(rewrite(&[("bc", "1"), ("abcd", "2")], "xbcd"), "x1d");
    assert_eq!(rewrite(&[("bc", "1"), ("abcd", "2")], "abcd"), "2");
}

#[test]
fn replacement_output_is_never_rescanned() {
    assert_eq!(rewrite(&[("a", "b"), ("b", "c")], "ab"), "bc");
    assert_eq!(rewrite(&[("x", "xx")], "x"), "xx");
}

#[test]
fn overlapping_occurrences_are_consumed_once() {
    assert_eq!(rewrite(&[("aa", "b")], "aaa"), "ba");
}

#[test]
fn slash_commands_are_left_whole() {
    let rules = [("model", "M")];
    assert_eq!(rewrite(&rules, "/model gpt"), "/model gpt");
    assert_eq!(rewrite(&rules, "  \n/model gpt"), "  \n/model gpt");
    assert_eq!(rewrite(&rules, "use model / x"), "use M / x");
    assert!(is_slash_command("\t/review"));
    assert!(!is_slash_command("a/b"));
}

#[test]
fn handles_cjk_emoji_and_combining_marks_without_normalising() {
    assert_eq!(rewrite(&[("提示词", "指令")], "改提示词库"), "改指令库");
    assert_eq!(rewrite(&[("👍🏽", "+1")], "ok 👍🏽 👍"), "ok +1 👍");
    // "é" precomposed does not match "e" + combining acute.
    assert_eq!(rewrite(&[("é", "E")], "e\u{301} é"), "e\u{301} E");
}

#[test]
fn empty_matcher_and_no_match_borrow() {
    let empty = Matcher::new(&[]);
    assert!(empty.is_empty());
    assert!(matches!(empty.rewrite("hi"), std::borrow::Cow::Borrowed(_)));
    let rules = [Rule::new("zz", "y")];
    let matcher = Matcher::new(&rules);
    assert!(matches!(
        matcher.rewrite("hi"),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn replacement_may_be_empty() {
    assert_eq!(rewrite(&[("um ", "")], "um um hello"), "hello");
}

#[test]
fn first_rule_wins_for_duplicate_matched_text() {
    assert_eq!(rewrite(&[("a", "mine"), ("a", "pack")], "a"), "mine");
}

#[test]
fn table_validation() {
    assert_eq!(
        validate_table(&[Rule::new("", "x")]),
        Err(RuleError::EmptyFrom)
    );
    assert_eq!(
        validate_table(&[Rule::new("a\nb", "x")]),
        Err(RuleError::ControlCharacter)
    );
    assert_eq!(
        validate_table(&[Rule::new("a", "1"), Rule::new("a", "2")]),
        Err(RuleError::Duplicate("a".into()))
    );
    assert_eq!(
        validate_table(&[Rule::new("x".repeat(201), "")]),
        Err(RuleError::FromTooLong)
    );
    assert!(validate_table(&[Rule::new("a", "line\nbreak")]).is_ok());
}

#[test]
fn snapshot_switches_gate_the_codex_matcher() {
    let mut snapshot = Snapshot {
        schema: SNAPSHOT_SCHEMA,
        enabled: true,
        tools: ToolSwitches {
            codex: true,
            ..Default::default()
        },
        rules: vec![Rule::new("a", "b")],
        by_tool: None,
    };
    assert!(snapshot.codex_matcher().is_some());
    snapshot.tools.codex = false;
    assert!(snapshot.codex_matcher().is_none());
    snapshot.tools.codex = true;
    snapshot.enabled = false;
    assert!(snapshot.codex_matcher().is_none());
    snapshot.enabled = true;
    snapshot.rules.clear();
    assert!(snapshot.codex_matcher().is_none());
}

#[test]
fn snapshot_parse_rejects_damage_and_unknown_schema() {
    assert!(Snapshot::parse(b"{not json").is_none());
    assert!(Snapshot::parse(br#"{"schema":2,"enabled":true,"tools":{},"rules":[]}"#).is_none());
    let ok = Snapshot::parse(
        br#"{"schema":1,"enabled":true,"tools":{"codex":true},"rules":[{"from":"a","to":"b"}]}"#,
    )
    .unwrap();
    assert_eq!(ok.rules, vec![Rule::new("a", "b")]);
}
