use regex::Regex;
use std::sync::OnceLock;

const REDACTED: &str = "[REDACTED]";

fn patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)\bsk-[A-Za-z0-9_\-]{6,}",
            r"(?i)\b(xai|ghp|gho|github_pat|xox[baprs]|AIza)[-_][A-Za-z0-9_\-]{6,}",
            r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]+=*",
            r"(?i)\b(authorization|api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|session[_-]?token|secret|password|passwd|credential|cookie|set-cookie)\s*[:=]\s*(?:bearer\s+)?[^\s,;]+",
            r"(?i)\b(token|cookie)\s*[:=]\s*[^\s,;]+",
            r"(?i)\b[A-Za-z0-9_-]*(?:token|cookie)[A-Za-z0-9_-]*\s*[:=]\s*[^\s,;]+",
        ]
        .into_iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect()
    })
}

/// Strip API keys, tokens, cookies, and credentials from logs and errors.
pub fn redact_text(input: &str) -> String {
    let mut out = input.to_string();
    for regex in patterns() {
        out = regex.replace_all(&out, REDACTED).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::redact_text;

    #[test]
    fn strips_sk_and_token_cookie_values() {
        let raw = "key=sk-ant-secret123456 cookie=abc123 token=xyz789";
        let redacted = redact_text(raw);
        assert!(!redacted.contains("sk-"), "{redacted}");
        assert!(!redacted.contains("abc123"), "{redacted}");
        assert!(!redacted.contains("xyz789"), "{redacted}");
    }
}
