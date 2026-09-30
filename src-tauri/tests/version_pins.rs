//! The version the app insists on and the adapter it ships must be the same
//! adapter. `confirm_activate` refuses any other, so a pin bumped in one place
//! and forgotten in the other would stop every deployment for that tool.

use keysmith_switch_lib::models::{versions_compatible, ToolKind};

fn vendored(rel: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../third_party/keysmith")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn assert_pinned(tool: ToolKind, shipped: &str) {
    assert!(
        versions_compatible(tool.expected_version(), shipped.trim()),
        "{tool:?}: the app expects {} but third_party/keysmith ships {}; update ToolKind::expected_version (and the test fixtures) together with the vendored adapter",
        tool.expected_version(),
        shipped.trim()
    );
}

#[test]
fn expected_versions_match_the_vendored_adapters() {
    assert_pinned(ToolKind::Codex, &vendored("codex/VERSION"));
    assert_pinned(ToolKind::Grok, &vendored("grok/VERSION"));
    assert_pinned(ToolKind::Zcode, &vendored("zcode/VERSION"));
    let claude = vendored("claude/claude-instruct.py");
    let line = claude
        .lines()
        .find(|line| line.starts_with("VERSION = "))
        .expect("claude-instruct.py declares VERSION");
    assert_pinned(
        ToolKind::Claude,
        line.trim_start_matches("VERSION = ").trim_matches('"'),
    );
}
