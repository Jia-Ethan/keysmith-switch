use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    pub added_lines: usize,
    pub removed_lines: usize,
    pub hunks: usize,
}

pub fn unified_diff(old: &str, new: &str, old_label: &str, new_label: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut unified = diff
        .unified_diff()
        .context_radius(3)
        .header(old_label, new_label)
        .to_string();
    if !unified.is_empty() && !unified.ends_with('\n') {
        unified.push('\n');
    }
    unified
}

pub fn change_summary(old: &str, new: &str) -> ChangeSummary {
    let diff = TextDiff::from_lines(old, new);
    let mut summary = ChangeSummary::default();
    for op in diff.ops() {
        match op.tag() {
            similar::DiffTag::Equal => {}
            similar::DiffTag::Delete | similar::DiffTag::Insert | similar::DiffTag::Replace => {
                summary.hunks += 1;
            }
        }
    }
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => summary.removed_lines += 1,
            ChangeTag::Insert => summary.added_lines += 1,
            ChangeTag::Equal => {}
        }
    }
    summary
}
