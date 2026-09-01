use keysmith_switch_lib::error::Error;
use keysmith_switch_lib::redact::redact_text;

#[test]
fn logs_and_errors_never_contain_sk_or_token_cookie_values() {
    let raw = concat!(
        "failed Authorization: Bearer sk-ant-secretABCDEFG ",
        "cookie=session-xyz123 token=rotating-token-999 ",
        "api_key=sk-proj-anotherSECRET"
    );
    let redacted = redact_text(raw);
    assert!(!redacted.contains("sk-"), "{redacted}");
    assert!(!redacted.contains("session-xyz123"), "{redacted}");
    assert!(!redacted.contains("rotating-token-999"), "{redacted}");
    assert!(!redacted.contains("anotherSECRET"), "{redacted}");
    assert!(redacted.contains("[REDACTED]"), "{redacted}");

    let err = Error::command_failed(raw);
    let rendered = err.to_string();
    assert!(!rendered.contains("sk-"), "{rendered}");
    assert!(!rendered.contains("session-xyz123"), "{rendered}");
    assert!(!rendered.contains("rotating-token-999"), "{rendered}");
}
