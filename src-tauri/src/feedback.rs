use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use url::Url;

use crate::error::{Error, Result};
use crate::models::APP_VERSION;

const REPO: &str = "Jia-Ethan/keysmith-switch";
const MAX_SCREENSHOTS: usize = 3;
const MAX_SCREENSHOT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackInput {
    kind: String,
    description: String,
    solution: Option<String>,
    contact: Option<String>,
    screenshots: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackResult {
    issue_url: Option<String>,
    fallback_url: Option<String>,
    reason: Option<&'static str>,
}

fn prepare(input: &FeedbackInput) -> Result<(String, String)> {
    let description = input.description.trim();
    if description.is_empty() || description.len() > 10_000 {
        return Err(Error::invalid("Description must be 1-10000 characters"));
    }
    let contact = input.contact.as_deref().unwrap_or("").trim();
    if contact.len() > 500 {
        return Err(Error::invalid("Contact must be at most 500 characters"));
    }
    let solution = input.solution.as_deref().unwrap_or("").trim();
    let (prefix, body) = match input.kind.as_str() {
        "bug" => (
            "Bug",
            format!(
                "### Keysmith Switch 版本 / Version\n\n{APP_VERSION}\n\n### 操作系统与架构 / Operating system and architecture\n\n{}\n\n### 操作位置 / Area\n\nother\n\n### 实际发生的问题（含复现步骤和预期结果）/ What happened (include steps and what you expected)\n\n{description}\n\n### 其他信息 / Additional context\n\n{}\n\n### 安全提醒 / Privacy reminder\n\nThis issue is public. Review and remove sensitive information before publishing.",
                format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH),
                if contact.is_empty() { "Not provided" } else { contact }
            ),
        ),
        "feature" if !solution.is_empty() && solution.len() <= 10_000 => (
            "Feature",
            format!(
                "### 需求描述 / Request\n\n{description}\n\n### 期望的解决方案 / Proposed solution\n\n{solution}\n\n### 联系方式 / Contact (optional)\n\n{}\n\n### 安全提醒 / Privacy reminder\n\nThis issue is public. Review and remove sensitive information before publishing.",
                if contact.is_empty() { "Not provided" } else { contact }
            ),
        ),
        "feature" => return Err(Error::invalid("Solution must be 1-10000 characters")),
        _ => return Err(Error::invalid("Unknown feedback kind")),
    };
    if input.screenshots.len() > MAX_SCREENSHOTS {
        return Err(Error::invalid("At most 3 screenshots are allowed"));
    }
    for screenshot in &input.screenshots {
        let path = Path::new(screenshot);
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if !path.is_absolute()
            || !["png", "jpg", "jpeg", "webp"].contains(&extension.to_ascii_lowercase().as_str())
        {
            return Err(Error::invalid("Screenshots must be PNG, JPG or WebP files"));
        }
        let metadata =
            std::fs::metadata(path).map_err(|_| Error::invalid("Screenshot is not accessible"))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_SCREENSHOT_BYTES {
            return Err(Error::invalid(
                "Each screenshot must be a nonempty file under 5 MB",
            ));
        }
    }
    let summary = description
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(description);
    let summary: String = summary.trim().chars().take(80).collect();
    let title = format!("[{prefix}] {summary}");
    Ok((title, body))
}

fn fallback(title: &str, body: &str, reason: &'static str) -> FeedbackResult {
    let mut url =
        Url::parse("https://github.com/Jia-Ethan/keysmith-switch/issues/new").expect("static URL");
    url.query_pairs_mut()
        .append_pair("title", title)
        .append_pair("body", body);
    FeedbackResult {
        issue_url: None,
        fallback_url: Some(url.into()),
        reason: Some(reason),
    }
}

async fn run_gh(args: &[&str], body: Option<&str>) -> std::io::Result<std::process::Output> {
    let mut child = Command::new("gh")
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(if body.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    if let Some(body) = body {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(body.as_bytes()).await?;
        }
    }
    match tokio::time::timeout(Duration::from_secs(20), child.wait_with_output()).await {
        Ok(output) => output,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "GitHub CLI timed out",
        )),
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn submit_feedback(input: FeedbackInput) -> Result<FeedbackResult> {
    let (title, body) = prepare(&input)?;
    if !input.screenshots.is_empty() {
        return Ok(fallback(&title, &body, "screenshotsManual"));
    }
    if which::which("gh").is_err() {
        return Ok(fallback(&title, &body, "ghMissing"));
    }
    let auth = run_gh(&["auth", "status", "--hostname", "github.com"], None).await;
    if !auth.is_ok_and(|output| output.status.success()) {
        return Ok(fallback(&title, &body, "notAuthenticated"));
    }
    let permission = run_gh(
        &[
            "api",
            "repos/Jia-Ethan/keysmith-switch",
            "--jq",
            ".permissions.push",
        ],
        None,
    )
    .await;
    if !permission.is_ok_and(|output| {
        output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
    }) {
        return Ok(fallback(&title, &body, "noPermission"));
    }
    let created = run_gh(
        &[
            "issue",
            "create",
            "--repo",
            REPO,
            "--title",
            &title,
            "--body-file",
            "-",
        ],
        Some(&body),
    )
    .await;
    if let Ok(output) = created {
        if output.status.success() {
            let link = String::from_utf8_lossy(&output.stdout);
            let link = link.trim();
            if link.starts_with("https://github.com/Jia-Ethan/keysmith-switch/issues/")
                && link
                    .rsplit('/')
                    .next()
                    .is_some_and(|number| number.parse::<u64>().is_ok())
            {
                return Ok(FeedbackResult {
                    issue_url: Some(link.to_string()),
                    fallback_url: None,
                    reason: None,
                });
            }
        }
    }
    Ok(fallback(&title, &body, "createFailed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(kind: &str) -> FeedbackInput {
        FeedbackInput {
            kind: kind.into(),
            description: "A clear request".into(),
            solution: Some("A proposed solution".into()),
            contact: None,
            screenshots: vec![],
        }
    }

    #[test]
    fn validates_required_fields_before_issue_creation() {
        let mut bug = example("bug");
        bug.description = "  ".into();
        assert!(prepare(&bug).is_err());
        let mut feature = example("feature");
        feature.solution = None;
        assert!(prepare(&feature).is_err());
    }

    #[test]
    fn prefilled_link_retains_draft_and_does_not_include_diagnostics() {
        let input = example("feature");
        let (title, body) = prepare(&input).unwrap();
        let result = fallback(&title, &body, "ghMissing");
        let url = Url::parse(result.fallback_url.as_deref().unwrap()).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("title").unwrap(), "[Feature] A clear request");
        assert!(query.get("body").unwrap().contains("A proposed solution"));
        assert!(!query.get("body").unwrap().contains("diagnostic-output"));
    }

    #[tokio::test]
    async fn screenshot_handoff_does_not_create_an_issue() {
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        std::fs::write(file.path(), b"image").unwrap();
        let mut input = example("bug");
        input.screenshots = vec![file.path().display().to_string()];
        let result = submit_feedback(input).await.unwrap();
        assert_eq!(result.reason, Some("screenshotsManual"));
        assert!(result.issue_url.is_none());
        assert!(result.fallback_url.is_some());
        assert!(!result.fallback_url.as_deref().unwrap().contains("image"));
    }

    #[test]
    fn screenshot_bounds_are_enforced() {
        let mut input = example("bug");
        input.screenshots = vec!["/tmp/a.png".into(); 4];
        assert!(prepare(&input).is_err());
        input.screenshots = vec!["relative.jpg".into()];
        assert!(prepare(&input).is_err());
    }
}
