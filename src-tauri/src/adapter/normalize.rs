use serde_json::Value;

use crate::models::{normalize_version, ToolKind, ToolStatus};
use crate::redact::redact_text;

use super::envelope::{BackupEntry, DoctorCheck, Envelope, PlannedFile, TargetPath};
use super::process::Captured;
use super::AdapterCommand;

pub fn normalize(
    tool: ToolKind,
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    envelope.argv = captured.argv.clone();
    envelope.exit_code = captured.exit_code;
    envelope.redacted_stderr = redact_text(&captured.stderr);
    if captured.timed_out {
        return envelope.fail("CLI timed out");
    }

    match tool {
        ToolKind::Claude => normalize_claude(command, captured, envelope),
        ToolKind::Codex => normalize_codex(command, captured, envelope),
        ToolKind::Grok => normalize_grok(command, captured, envelope),
        ToolKind::Zcode => normalize_zcode(command, captured, envelope),
    }
}

fn normalize_claude(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        let version = extract_version(&captured.stdout, "7.1");
        envelope.adapter_version = Some(version);
        envelope.ok = captured.exit_code == 0;
        envelope.preview = true;
        return envelope;
    }
    if matches!(command, AdapterCommand::Doctor) {
        return normalize_claude_doctor(captured, envelope);
    }
    let json = match parse_json(&captured.stdout) {
        Ok(value) => value,
        Err(error) => return envelope.fail(error),
    };
    envelope.ok = json_bool(&json, "ok").unwrap_or(captured.exit_code == 0);
    envelope.preview = json
        .get("mode")
        .and_then(Value::as_str)
        .map(|mode| mode != "execute")
        .unwrap_or(envelope.preview);
    envelope.reload_required = json_bool(&json, "reload_required").unwrap_or(false);
    envelope.reload_hint = json_string(&json, "reload_hint");
    envelope.error = json_string(&json, "error").map(|value| redact_text(&value));
    envelope.warnings = json_string_list(&json, "warnings");
    envelope.blockers = json_string_list(&json, "blockers");
    envelope.planned_files = json_actions(&json);
    envelope.backups = json_backups(&json);
    push_target(&mut envelope, &json, "memory_file", "memory");
    push_target(&mut envelope, &json, "instruction_file", "instruction");
    if let Some(target) = json.get("target") {
        if let Some(path) = target.get("memory_file").and_then(Value::as_str) {
            envelope.target_paths.push(TargetPath {
                path: path.to_string(),
                role: "memory".into(),
                exists: true,
            });
        }
    }
    if let Some(source) = json.get("source_identity") {
        envelope.current_fingerprint = source
            .get("instruction_sha256")
            .and_then(Value::as_str)
            .map(str::to_string);
        if source.get("drift").and_then(Value::as_bool) == Some(true) {
            envelope.status = ToolStatus::Drift;
        }
    }
    if let Some(recovery) = json.get("recovery_state") {
        envelope.recovery_required = recovery
            .get("recovery_required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || recovery
                .get("must_recover_before_writes")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        if envelope.recovery_required {
            envelope.status = ToolStatus::RecoveryRequired;
        }
    }
    if matches!(command, AdapterCommand::Status { .. }) {
        if envelope.status != ToolStatus::RecoveryRequired && envelope.status != ToolStatus::Drift {
            if json_bool(&json, "installed") == Some(true) {
                envelope.status = ToolStatus::Active;
            } else if json
                .pointer("/presence/memory_file")
                .and_then(Value::as_bool)
                == Some(true)
                || json_bool(&json, "memory_file_exists") == Some(true)
            {
                envelope.status = ToolStatus::Inactive;
            } else {
                envelope.status = ToolStatus::NotInstalled;
            }
        }
    }
    if !envelope.blockers.is_empty() {
        envelope.ok = false;
        if envelope.status == ToolStatus::Inactive || envelope.status == ToolStatus::Active {
            envelope.status = ToolStatus::Conflict;
        }
    }
    if captured.exit_code != 0 && envelope.ok {
        envelope.ok = false;
    }
    envelope
}

fn normalize_claude_doctor(captured: &Captured, mut envelope: Envelope) -> Envelope {
    let json = match parse_json(&captured.stdout) {
        Ok(value) => value,
        Err(error) => return envelope.fail(error),
    };
    let keys = [
        "installation_type",
        "upstream_candidates",
        "upstream_path",
        "system_prompt_file",
        "append_prompt_file",
        "settings_file",
        "shell_kind",
        "shell_rc",
        "repair_actions",
    ];
    let mut checks = Vec::new();
    for key in keys {
        let detail = json.get(key).map(|value| value_to_string(value));
        checks.push(DoctorCheck {
            name: key.to_string(),
            ok: true,
            detail,
        });
    }
    let repair = json
        .get("repair_actions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .any(|item| !item.to_ascii_lowercase().contains("no repair"))
        })
        .unwrap_or(false);
    envelope.doctor = super::envelope::DoctorReport {
        ok: !repair && captured.exit_code == 0,
        checks,
    };
    envelope.ok = envelope.doctor.ok;
    envelope.preview = true;
    if json.get("upstream_path").and_then(Value::as_str).is_none() {
        envelope.status = ToolStatus::NotInstalled;
    }
    envelope
}

fn normalize_grok(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        if let Ok(json) = parse_json(&captured.stdout) {
            if let Some(version) = json
                .pointer("/result/version")
                .and_then(Value::as_str)
                .or_else(|| json.get("version").and_then(Value::as_str))
            {
                envelope.adapter_version = Some(normalize_version(version));
            }
        } else if let Some(version) = captured
            .stdout
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().last())
        {
            envelope.adapter_version = Some(normalize_version(version));
        }
        envelope.ok = captured.exit_code == 0;
        return envelope;
    }
    let json = match parse_json(&captured.stdout) {
        Ok(value) => value,
        Err(error) => return envelope.fail(error),
    };
    envelope.ok = json_bool(&json, "ok").unwrap_or(captured.exit_code == 0);
    envelope.preview = json_bool(&json, "preview").unwrap_or(envelope.preview);
    envelope.warnings = json
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| redact_text(item))
                .collect()
        })
        .unwrap_or_default();
    if let Some(target) = json.get("target") {
        if let Some(path) = target.get("grok_dir").and_then(Value::as_str) {
            envelope.target_paths.push(TargetPath {
                path: path.to_string(),
                role: "config-root".into(),
                exists: true,
            });
        }
    }
    let result = json.get("result").cloned().unwrap_or(Value::Null);
    let plan = json.get("plan").cloned().unwrap_or(Value::Null);
    if let Some(state) = result.get("state").and_then(Value::as_str) {
        if let Ok(status) = state.parse() {
            envelope.status = status;
        }
    }
    envelope.recovery_required = result
        .get("recovery_required")
        .and_then(Value::as_bool)
        .unwrap_or(envelope.status == ToolStatus::RecoveryRequired)
        || envelope.status == ToolStatus::RecoveryRequired;
    envelope.conflicts = result
        .get("conflicts")
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default();
    envelope.blockers = plan
        .get("blockers")
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_else(|| envelope.conflicts.clone());
    if let Some(rule) = plan.get("rule").and_then(|value| value.get("path")) {
        if let Some(path) = rule.as_str() {
            envelope.planned_files.push(PlannedFile {
                path: path.to_string(),
                action: "write".into(),
                detail: "managed rule".into(),
            });
        }
    }
    if let Some(rule) = result.pointer("/nodes/rule") {
        if let Some(path) = rule.get("path").and_then(Value::as_str) {
            envelope.target_paths.push(TargetPath {
                path: path.to_string(),
                role: "rule".into(),
                exists: rule.get("kind").and_then(Value::as_str) == Some("regular"),
            });
        }
        envelope.current_fingerprint = rule
            .pointer("/fingerprint/sha256")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    if matches!(command, AdapterCommand::Doctor) {
        envelope.doctor.ok = envelope.ok && !envelope.recovery_required;
        envelope.doctor.checks.push(DoctorCheck {
            name: "state".into(),
            ok: envelope.ok,
            detail: Some(envelope.status.as_str().to_string()),
        });
    }
    if !envelope.blockers.is_empty()
        && matches!(
            command,
            AdapterCommand::PlanActivate { .. } | AdapterCommand::Activate { .. }
        )
    {
        envelope.ok = false;
    }
    envelope
}

fn normalize_codex(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        if let Some(version) = captured
            .stdout
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().last())
        {
            envelope.adapter_version = Some(normalize_version(version));
        }
        envelope.ok = captured.exit_code == 0;
        return envelope;
    }
    let text = captured.stdout.as_str();
    envelope.ok =
        captured.exit_code == 0 && !text.contains("[Error]") && !text.contains("[Blocked]");
    envelope.preview = !captured.argv.iter().any(|item| item == "--yes");
    if let Some(status) = parse_codex_activation(text) {
        envelope.status = status;
    }
    if text.to_ascii_lowercase().contains("transaction residue")
        && text
            .lines()
            .any(|line| line.contains("Transaction residue") && !line.contains("none"))
    {
        envelope.recovery_required = true;
        envelope.status = ToolStatus::RecoveryRequired;
    }
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[Warning]") {
            envelope.warnings.push(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("[Error]") {
            envelope.blockers.push(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("[Blocked]") {
            envelope.blockers.push(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("Target:") {
            envelope.target_paths.push(TargetPath {
                path: rest.trim().to_string(),
                role: "codex-dir".into(),
                exists: true,
            });
        }
        if let Some(rest) = line.strip_prefix("── Status directory:") {
            let path = rest.trim().trim_end_matches('─').trim().to_string();
            envelope.target_paths.push(TargetPath {
                path,
                role: "codex-dir".into(),
                exists: true,
            });
        }
        if let Some(rest) = line.strip_prefix("→ ") {
            envelope.planned_files.push(PlannedFile {
                path: rest.to_string(),
                action: "plan".into(),
                detail: rest.to_string(),
            });
        }
    }
    if envelope.status == ToolStatus::Conflict {
        envelope.ok = false;
    }
    if matches!(command, AdapterCommand::Doctor) {
        envelope.doctor.ok = envelope.ok && !envelope.recovery_required;
        envelope.doctor.checks.push(DoctorCheck {
            name: "activation".into(),
            ok: envelope.ok,
            detail: Some(envelope.status.as_str().to_string()),
        });
    }
    envelope
}

fn normalize_zcode(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        if let Some(version) = captured
            .stdout
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().last())
        {
            envelope.adapter_version = Some(normalize_version(version));
        }
        envelope.ok = captured.exit_code == 0;
        return envelope;
    }
    let text = captured.stdout.as_str();
    envelope.ok = captured.exit_code == 0;
    envelope.preview =
        text.contains("preview") || !captured.argv.iter().any(|item| item == "--yes");
    let mut system_exists = false;
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "system_file" | "config_file" | "wrapper" | "launch_agent" | "managed_dir" => {
                    envelope.target_paths.push(TargetPath {
                        path: value.to_string(),
                        role: key.to_string(),
                        exists: true,
                    });
                    envelope.planned_files.push(PlannedFile {
                        path: value.to_string(),
                        action: "write".into(),
                        detail: key.to_string(),
                    });
                }
                "system_file_exists" => system_exists = value == "true",
                "system_file_sha256" if value != "missing" => {
                    envelope.current_fingerprint = Some(value.to_string());
                }
                "backup" => envelope.backups.push(BackupEntry {
                    target: value.to_string(),
                    backup_path: Some(value.to_string()),
                    planned: envelope.preview,
                }),
                _ => {}
            }
        }
    }
    envelope.status = if system_exists || text.contains("install complete") {
        ToolStatus::Active
    } else if text.contains("doctor") {
        ToolStatus::NotInstalled
    } else {
        ToolStatus::Inactive
    };
    if matches!(
        command,
        AdapterCommand::Doctor | AdapterCommand::Status { .. }
    ) {
        envelope.doctor.ok = captured.exit_code == 0;
        envelope.doctor.checks.push(DoctorCheck {
            name: "system_file_exists".into(),
            ok: system_exists,
            detail: Some(system_exists.to_string()),
        });
        if !system_exists {
            envelope.status = ToolStatus::NotInstalled;
        }
    }
    envelope
}

fn parse_codex_activation(text: &str) -> Option<ToolStatus> {
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("Config activation:") {
            let token = rest.split_whitespace().next().unwrap_or("");
            return token.parse().ok();
        }
    }
    if text.contains("not-installed") {
        Some(ToolStatus::NotInstalled)
    } else if text.contains("inactive") {
        Some(ToolStatus::Inactive)
    } else if text.contains("conflict") {
        Some(ToolStatus::Conflict)
    } else if text.contains("active") {
        Some(ToolStatus::Active)
    } else {
        None
    }
}

fn parse_json(stdout: &str) -> std::result::Result<Value, String> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err("CLI produced no JSON".into());
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                if let Ok(value) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
                    return Ok(value);
                }
            }
        }
    }
    Err("CLI output is not valid JSON".into())
}

fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|item| {
        if item.is_null() {
            None
        } else {
            item.as_str().map(str::to_string)
        }
    })
}

fn json_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default()
}

fn string_values(items: &Vec<Value>) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| item.as_str().map(|value| redact_text(value)))
        .collect()
}

fn json_actions(value: &Value) -> Vec<PlannedFile> {
    value
        .get("actions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(PlannedFile {
                        path: item.get("path")?.as_str()?.to_string(),
                        action: item
                            .get("action")
                            .and_then(Value::as_str)
                            .unwrap_or("write")
                            .to_string(),
                        detail: redact_text(
                            item.get("detail").and_then(Value::as_str).unwrap_or(""),
                        ),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn json_backups(value: &Value) -> Vec<BackupEntry> {
    value
        .get("backups")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(BackupEntry {
                        target: item.get("target")?.as_str()?.to_string(),
                        backup_path: item
                            .get("backup_path")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        planned: item
                            .get("planned")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn push_target(envelope: &mut Envelope, json: &Value, key: &str, role: &str) {
    if let Some(path) = json.get(key).and_then(Value::as_str) {
        let exists_key = format!("{key}_exists");
        envelope.target_paths.push(TargetPath {
            path: path.to_string(),
            role: role.to_string(),
            exists: json_bool(json, &exists_key).unwrap_or(false),
        });
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => redact_text(text),
        other => redact_text(&other.to_string()),
    }
}

fn extract_version(stdout: &str, fallback: &str) -> String {
    stdout
        .lines()
        .next()
        .and_then(|line| {
            line.split_whitespace()
                .last()
                .map(|value| normalize_version(value))
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}
