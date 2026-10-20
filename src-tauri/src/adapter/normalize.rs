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

    let mut envelope = match tool {
        ToolKind::Claude => normalize_claude(command, captured, envelope),
        ToolKind::Codex => normalize_codex(command, captured, envelope),
        ToolKind::Grok => normalize_grok(command, captured, envelope),
        ToolKind::Zcode => normalize_zcode(command, captured, envelope),
        ToolKind::Pi => normalize_pi(command, captured, envelope),
        ToolKind::Dsh => normalize_dsh(command, captured, envelope),
    };
    // A CLI that stopped without a reason in its output still gave one on stderr.
    if !envelope.ok
        && envelope.exit_code != 0
        && envelope.blockers.is_empty()
        && envelope.error.is_none()
    {
        if let Some(reason) = stderr_reason(&envelope.redacted_stderr) {
            envelope.blockers.push(reason);
        }
    }
    envelope
}

/// The `error: …` message a CLI printed (with the lines that follow it), or else its last
/// line, which is where Python puts the exception of a traceback.
fn stderr_reason(stderr: &str) -> Option<String> {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let reason = match lines.iter().position(|line| line.starts_with("error: ")) {
        Some(at) => lines[at..].join(" ")["error: ".len()..].to_string(),
        None => lines.last()?.to_string(),
    };
    Some(reason.chars().take(400).collect())
}

fn normalize_claude(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        let version = extract_version(&captured.stdout, "7.2");
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
    // Refused before it could plan (no deployment manifest, a lock): the reasons are its
    // diagnostics.
    if !envelope.ok && envelope.blockers.is_empty() {
        envelope.blockers = envelope.warnings.clone();
    }
    envelope.salvage_available = plan
        .get("salvage_available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    envelope.config_salvage = plan
        .get("config_salvage")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    envelope.confirmation_token = plan
        .get("confirmation_token")
        .or_else(|| result.get("confirmation_token"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|token| token.len() == 64 && token.chars().all(|ch| ch.is_ascii_hexdigit()));
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

const PI_SCHEMA: &str = "pi-keysmith.envelope.v1";
const PI_MIN_VERSION: (u64, u64, u64) = (0, 2, 0);
const DSH_SCHEMA: &str = "dsh-keysmith/v1";
/// 0.2.1 is the first dsh-keysmith whose `--json` reports `state`, `dsh_dir_exists` and a
/// `version` field. 0.2.0 shipped the contract without them and must not be accepted.
const DSH_MIN_VERSION: (u64, u64, u64) = (0, 2, 1);

/// `pi-keysmith.envelope.v1`. Same shape as Grok's envelope, different wire: the state is in
/// `status.*` (not `result.*`), `ok` is false for a valid `conflict` / `recovery-required`
/// status, and every diagnostic is `"<code>: <sentence>"`.
fn normalize_pi(command: &AdapterCommand, captured: &Captured, mut envelope: Envelope) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        // argparse text (`pi-keysmith.py 0.2.0`), not an envelope.
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
    let json = match parse_json(&captured.stdout) {
        Ok(value) => value,
        Err(error) => return envelope.fail(error),
    };
    if json.get("schema").and_then(Value::as_str) != Some(PI_SCHEMA) {
        return envelope.fail(format!("pi adapter did not answer with {PI_SCHEMA}"));
    }
    let version = json.get("version").and_then(Value::as_str).unwrap_or("");
    if !semver_at_least(version, PI_MIN_VERSION) {
        return envelope.fail(format!(
            "pi adapter {version} is too old; Keysmith Switch needs 0.2.0 or newer"
        ));
    }
    envelope.adapter_version = Some(normalize_version(version));
    envelope.ok = json_bool(&json, "ok").unwrap_or(false);
    envelope.exit_code = json
        .get("exit_code")
        .and_then(Value::as_i64)
        .map(|code| code as i32)
        .unwrap_or(captured.exit_code);
    envelope.preview = json_bool(&json, "preview").unwrap_or(envelope.preview);

    let target = json.get("target").cloned().unwrap_or(Value::Null);
    if let Some(dir) = target.get("pi_dir").and_then(Value::as_str) {
        if target.get("scope").and_then(Value::as_str) != Some("user") {
            return envelope.fail("pi adapter reported a scope other than user");
        }
        envelope.target_paths.push(TargetPath {
            path: dir.to_string(),
            role: "config-root".into(),
            exists: json
                .pointer("/status/nodes/pi_dir/kind")
                .and_then(Value::as_str)
                == Some("directory"),
        });
    }

    // Every diagnostic, keyed by its code (the part before the first colon).
    let diagnostics: Vec<(String, String)> = json
        .get("diagnostics")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| {
                    let code = item.split(':').next().unwrap_or("").trim().to_string();
                    (code, redact_text(item))
                })
                .collect()
        })
        .unwrap_or_default();
    // A bare code reads as its diagnostic sentence when the adapter gave one.
    let explain = |code: &str| -> String {
        let base = code.split(':').next().unwrap_or(code);
        diagnostics
            .iter()
            .find(|(known, _)| known == base)
            .map(|(_, text)| text.clone())
            .unwrap_or_else(|| redact_text(code))
    };
    envelope.warnings = diagnostics.iter().map(|(_, text)| text.clone()).collect();

    let status = json.get("status").cloned().unwrap_or(Value::Null);
    if let Some(state) = status.get("state").and_then(Value::as_str) {
        envelope.status = match state {
            "not-installed" => ToolStatus::NotInstalled,
            "conflict" => ToolStatus::Conflict,
            "recovery-required" => ToolStatus::RecoveryRequired,
            // A hand-edited SYSTEM.md does not stop a deploy: pi backs it up and replaces it.
            // Reported on a deploy plan, it would read as a repair to make first.
            "drift"
                if matches!(
                    command,
                    AdapterCommand::PlanActivate { .. } | AdapterCommand::Activate { .. }
                ) =>
            {
                ToolStatus::Inactive
            }
            "drift" => ToolStatus::Drift,
            "active-aligned" => ToolStatus::Active,
            other => return envelope.fail(format!("pi adapter reported unknown state {other}")),
        };
    }
    envelope.recovery_required = status
        .get("recovery_required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || envelope.status == ToolStatus::RecoveryRequired;
    envelope.conflicts = status
        .get("conflicts")
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default();
    for (key, role) in [("system", "system_file"), ("manifest", "manifest")] {
        if let Some(node) = status.pointer(&format!("/nodes/{key}")) {
            if let Some(path) = node.get("path").and_then(Value::as_str) {
                envelope.target_paths.push(TargetPath {
                    path: path.to_string(),
                    role: role.into(),
                    exists: node.get("kind").and_then(Value::as_str) == Some("regular"),
                });
            }
        }
    }
    envelope.current_fingerprint = status
        .pointer("/nodes/system/fingerprint/sha256")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(residue) = status.get("residue") {
        let left: Vec<String> = ["journals", "cleanup_markers", "temp_files"]
            .iter()
            .flat_map(|key| {
                residue
                    .get(*key)
                    .and_then(Value::as_array)
                    .map(string_values)
                    .unwrap_or_default()
            })
            .collect();
        if !left.is_empty() {
            envelope.recovery_required = true;
            envelope
                .warnings
                .push(format!("unfinished transaction left: {}", left.join(", ")));
        }
    }

    let plan = json.get("plan").cloned().unwrap_or(Value::Null);
    let blockers: Vec<String> = match plan.get("blockers").and_then(Value::as_array) {
        Some(items) => string_values(items),
        // A status has no plan: what stops a write is in the status itself.
        None => match command {
            AdapterCommand::PlanDeactivate { .. } | AdapterCommand::Deactivate { .. } => {
                status_blockers(&status, "uninstall")
            }
            AdapterCommand::PlanActivate { .. } | AdapterCommand::Activate { .. } => {
                status_blockers(&status, "deploy")
            }
            _ => envelope.conflicts.clone(),
        },
    };
    envelope.blockers = blockers.iter().map(|code| explain(code)).collect();
    envelope.confirmation_token = plan
        .get("confirmation_token")
        .and_then(Value::as_str)
        .filter(|token| {
            token.len() == 64
                && token
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
        })
        .map(str::to_string);
    let root = target.get("pi_dir").and_then(Value::as_str);
    if let Some(writes) = plan.get("writes").and_then(Value::as_array) {
        for write in writes {
            let Some(name) = write.get("path").and_then(Value::as_str) else {
                continue;
            };
            let detail = ["backup", "source", "archive"]
                .iter()
                .find_map(|key| {
                    write
                        .get(*key)
                        .and_then(Value::as_str)
                        .map(|value| format!("{key}: {value}"))
                })
                .unwrap_or_default();
            envelope.planned_files.push(PlannedFile {
                path: root
                    .map(|dir| std::path::Path::new(dir).join(name).display().to_string())
                    .unwrap_or_else(|| name.to_string()),
                action: write
                    .get("action")
                    .and_then(Value::as_str)
                    .unwrap_or("write")
                    .to_string(),
                detail: redact_text(&detail),
            });
        }
    }

    // Refused before or while writing: say why. Exits 2, 3 and 5 changed nothing; only 4 may
    // have left a half-done write for recover.
    if !envelope.ok {
        let has = |code: &str| diagnostics.iter().any(|(known, _)| known == code);
        if has("preview-token-mismatch") {
            envelope.error = Some("pi changed since the preview; preview again".into());
        } else if has("lock-busy") {
            envelope.error = Some("another pi-keysmith run is busy; try again".into());
        } else if has("write-failed") || envelope.exit_code == 4 {
            envelope.recovery_required = true;
            envelope.error = Some("pi write failed partway; recover first".into());
        }
        if envelope.blockers.is_empty() {
            envelope.blockers = diagnostics
                .iter()
                .filter(|(code, _)| code != "windows-beta")
                .map(|(_, text)| text.clone())
                .collect();
        }
    }

    if matches!(
        command,
        AdapterCommand::Doctor | AdapterCommand::Status { .. }
    ) {
        envelope.doctor.ok = envelope.ok && !envelope.recovery_required;
        envelope.doctor.checks.push(DoctorCheck {
            name: "state".into(),
            ok: envelope.ok,
            detail: Some(envelope.status.as_str().to_string()),
        });
        for (name, key, kind) in [
            ("deployable", "deployable", "deploy"),
            ("uninstall_ready", "uninstall_ready", "uninstall"),
        ] {
            let blocked = status_blockers(&status, kind);
            envelope.doctor.checks.push(DoctorCheck {
                name: name.into(),
                ok: json_bool(&status, key).unwrap_or(false),
                detail: (!blocked.is_empty()).then(|| blocked.join(", ")),
            });
        }
    }
    envelope
}

/// dsh-keysmith 0.2.1 answers with `dsh-keysmith/v1`. It reports `state` outright, so unlike
/// pi there is no status tree to walk — but the state it reports has to line up with what a
/// deploy may safely do, and dsh backs up before it replaces, so drift does not block a deploy.
fn normalize_dsh(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
) -> Envelope {
    if matches!(command, AdapterCommand::Version) {
        // argparse text (`dsh-keysmith 0.2.1`), not an envelope.
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
    let json = match parse_json(&captured.stdout) {
        Ok(value) => value,
        Err(error) => return envelope.fail(error),
    };
    if json.get("schema").and_then(Value::as_str) != Some(DSH_SCHEMA) {
        return envelope.fail(format!("dsh adapter did not answer with {DSH_SCHEMA}"));
    }
    let version = json.get("version").and_then(Value::as_str).unwrap_or("");
    if !semver_at_least(version, DSH_MIN_VERSION) {
        return envelope.fail(format!(
            "dsh adapter {version} is too old; Keysmith Switch needs 0.2.1 or newer"
        ));
    }
    envelope.adapter_version = Some(normalize_version(version));
    envelope.ok = json_bool(&json, "ok").unwrap_or(false);
    envelope.exit_code = json
        .get("exit_status")
        .and_then(Value::as_i64)
        .map(|code| code as i32)
        .unwrap_or(captured.exit_code);
    // dsh-keysmith says `preview`/`execute`; the envelope carries a boolean.
    envelope.preview = json
        .get("mode")
        .and_then(Value::as_str)
        .map(|mode| mode != "execute")
        .unwrap_or(envelope.preview);

    let dsh_dir = json.get("dsh_dir").and_then(Value::as_str);
    let dir_exists = json_bool(&json, "dsh_dir_exists").unwrap_or(false);
    if let Some(dir) = dsh_dir {
        envelope.target_paths.push(TargetPath {
            path: dir.to_string(),
            role: "config-root".into(),
            exists: dir_exists,
        });
    }

    // dsh-keysmith's own messages are English regardless of `--lang`, so they are passed
    // through as-is; the interface translates only what it has wording for.
    envelope.warnings = json
        .get("warnings")
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default()
        .iter()
        .map(|item| redact_text(item))
        .collect();

    let drift: Vec<String> = json
        .get("drift")
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default();

    if let Some(state) = json.get("state").and_then(Value::as_str) {
        envelope.status = match state {
            "not-installed" => ToolStatus::NotInstalled,
            "recovery-required" => ToolStatus::RecoveryRequired,
            // A hand-edited AGENTS.md does not stop a deploy: dsh-keysmith backs it up and
            // replaces it. Reported on a deploy plan, it would read as a repair to make first.
            "drift"
                if matches!(
                    command,
                    AdapterCommand::PlanActivate { .. } | AdapterCommand::Activate { .. }
                ) =>
            {
                ToolStatus::Inactive
            }
            "drift" => ToolStatus::Drift,
            "active" => ToolStatus::Active,
            other => return envelope.fail(format!("dsh adapter reported unknown state {other}")),
        };
    } else if matches!(
        command,
        AdapterCommand::Doctor | AdapterCommand::Status { .. }
    ) {
        return envelope.fail("dsh adapter did not report a state");
    } else if matches!(
        command,
        AdapterCommand::Activate { .. } | AdapterCommand::Deactivate { .. }
    ) {
        // dsh-keysmith describes the operation it performed, not the state it left behind.
        // A confirmed write that succeeded did what it said, so the state is the one it aimed
        // at; deriving it here is why Switch can show the result without a second round trip.
        envelope.status = if matches!(command, AdapterCommand::Activate { .. }) {
            ToolStatus::Active
        } else {
            ToolStatus::NotInstalled
        };
    }
    envelope.recovery_required = envelope.status == ToolStatus::RecoveryRequired;
    for note in &drift {
        envelope.warnings.push(redact_text(note));
    }

    // The adapter hot-fix is the one resource that can silently stop applying: it is skipped
    // with a warning when the upstream source matches no known snippet, which is the expected
    // outcome on newer dsh releases. Surface it rather than let a skip read as a success.
    if let Some(adapter_state) = json.pointer("/adapter_patch/state").and_then(Value::as_str) {
        let adapter_path = json
            .pointer("/adapter_patch/path")
            .and_then(Value::as_str)
            .unwrap_or("");
        if adapter_path.is_empty() {
            envelope.target_paths.push(TargetPath {
                path: adapter_path.to_string(),
                role: "adapter".into(),
                exists: false,
            });
        }
        if matches!(adapter_state, "unknown" | "absent") {
            let note = if adapter_state == "unknown" {
                "dsh adapter matches no known version; the streaming tool-call fix was skipped"
            } else {
                "dsh adapter file not found; the streaming tool-call fix was skipped"
            };
            if !envelope.warnings.iter().any(|known| known == note) {
                envelope.warnings.push(note.to_string());
            }
        }
    }

    // The deployed prompt's sha, so the harness can name which library prompt is live.
    envelope.current_fingerprint = json
        .pointer("/manifest/resources/agents_md/after_sha256")
        .and_then(Value::as_str)
        .map(str::to_string);

    let blockers = json
        .get("blockers")
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default();
    envelope.blockers = blockers.iter().map(|item| redact_text(item)).collect();
    if let Some(error) = json.get("error").and_then(Value::as_str) {
        if !error.is_empty() {
            envelope.error = Some(redact_text(error));
        }
    }

    // dsh-keysmith reports every file it plans to touch, including the manifest and backups.
    // Paths it prints are already absolute.
    for action in json
        .get("actions")
        .and_then(Value::as_array)
        .map(|items| items.iter())
        .into_iter()
        .flatten()
    {
        let Some(path) = action.get("path").and_then(Value::as_str) else {
            continue;
        };
        let kind = action
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("write");
        // `noop`/`skip` describe a resource that is already in place; listing them as planned
        // writes would overstate what a confirmation changes.
        if matches!(kind, "noop" | "skip") {
            continue;
        }
        envelope.planned_files.push(PlannedFile {
            path: path.to_string(),
            action: kind.to_string(),
            detail: redact_text(action.get("detail").and_then(Value::as_str).unwrap_or("")),
        });
    }

    // Exit 1 with blockers is the drift refusal (or residue); 2 changed nothing. Neither may
    // leave a half-written deployment, so neither marks recovery as required on its own.
    if !envelope.ok && envelope.error.is_none() && !envelope.blockers.is_empty() {
        envelope.error = envelope.blockers.first().cloned();
    }

    if matches!(
        command,
        AdapterCommand::Doctor | AdapterCommand::Status { .. }
    ) {
        envelope.doctor.ok = envelope.ok && !envelope.recovery_required;
        envelope.doctor.checks.push(DoctorCheck {
            name: "state".into(),
            ok: envelope.ok,
            detail: Some(envelope.status.as_str().to_string()),
        });
        envelope.doctor.checks.push(DoctorCheck {
            name: "dsh_dir".into(),
            ok: dir_exists,
            detail: dsh_dir.map(str::to_string),
        });
        envelope.doctor.checks.push(DoctorCheck {
            name: "drift".into(),
            ok: drift.is_empty(),
            detail: (!drift.is_empty()).then(|| drift.join("; ")),
        });
    }
    envelope
}

fn status_blockers(status: &Value, kind: &str) -> Vec<String> {
    status
        .pointer(&format!("/blockers/{kind}"))
        .and_then(Value::as_array)
        .map(string_values)
        .unwrap_or_default()
}

fn semver_at_least(version: &str, min: (u64, u64, u64)) -> bool {
    let mut parts = normalize_version(version)
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>())
        .collect::<Vec<_>>()
        .into_iter();
    let mut next = || parts.next().and_then(|part| part.ok());
    match (next(), next(), next()) {
        (Some(major), Some(minor), Some(patch)) => (major, minor, patch) >= min,
        _ => false,
    }
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
    let mut status_dir: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[Warning]") {
            envelope.warnings.push(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("[Error]") {
            envelope.blockers.push(rest.trim().to_string());
        }
        // A dry run reports a refused step as `→ [Blocked] …`, with a leading arrow.
        if let Some(rest) = line
            .trim_start_matches('→')
            .trim_start()
            .strip_prefix("[Blocked]")
        {
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
            status_dir = Some(path.clone());
            envelope.target_paths.push(TargetPath {
                path,
                role: "codex-dir".into(),
                exists: true,
            });
        }
        // The adapter names the prompt file Codex loads. Report it, so it can be
        // read back; a value in angle brackets means "not set".
        if let (Some(rest), Some(dir)) =
            (line.strip_prefix("model_instructions_file:"), &status_dir)
        {
            let value = rest.trim();
            if !value.is_empty() && !value.starts_with('<') {
                let path = std::path::Path::new(dir).join(value);
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                let listed = text.lines().any(|item| {
                    item.trim()
                        .strip_prefix(file_name)
                        .is_some_and(|tail| tail.starts_with(": regular file"))
                });
                envelope.target_paths.push(TargetPath {
                    path: path.display().to_string(),
                    role: "instruction".into(),
                    exists: listed,
                });
            }
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
        if let Some(version) = zcode_version_token(&captured.stdout) {
            envelope.adapter_version = Some(normalize_version(&version));
        }
        envelope.ok = captured.exit_code == 0;
        return envelope;
    }
    if let Some(json) = zcode_json(&captured.stdout) {
        return normalize_zcode_json(command, captured, envelope, &json);
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

fn zcode_version_token(stdout: &str) -> Option<String> {
    if let Some(json) = zcode_json(stdout) {
        if let Some(version) = json.get("version").and_then(Value::as_str) {
            return Some(version.to_string());
        }
    }
    stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().last())
        .map(str::to_string)
}

fn zcode_json(stdout: &str) -> Option<Value> {
    let value = parse_json(stdout).ok()?;
    let schema = value.get("schema").and_then(Value::as_str).unwrap_or("");
    if schema == "zcode-keysmith/v1" {
        Some(value)
    } else {
        None
    }
}

fn normalize_zcode_json(
    command: &AdapterCommand,
    captured: &Captured,
    mut envelope: Envelope,
    json: &Value,
) -> Envelope {
    let operation = json.get("operation").and_then(Value::as_str).unwrap_or("");
    let mode = json.get("mode").and_then(Value::as_str).unwrap_or("");
    envelope.preview = mode != "execute";
    envelope.ok = json_bool(json, "ok").unwrap_or(false) && captured.exit_code == 0;
    if let Some(error) = json.get("error").and_then(Value::as_str) {
        if !error.is_empty() {
            envelope.blockers.push(redact_text(error));
            envelope.ok = false;
        }
    }
    envelope.blockers.extend(json_string_list(json, "blockers"));
    envelope.warnings.extend(json_string_list(json, "warnings"));
    if !envelope.blockers.is_empty() {
        envelope.ok = false;
    }
    envelope.planned_files.extend(json_actions(json));
    for path in json_string_list(json, "removed") {
        envelope.planned_files.push(PlannedFile {
            path,
            action: "removed".into(),
            detail: "removed".into(),
        });
    }
    let managed = json.get("managed").cloned().unwrap_or_else(|| json.clone());
    let system_exists = json_bool(&managed, "system_file_exists").unwrap_or(false);
    if let Some(hash) = json_string(&managed, "system_file_sha256") {
        if hash != "missing" && !hash.is_empty() {
            envelope.current_fingerprint = Some(hash);
        }
    }
    for (key, role) in [
        ("system_file", "system_file"),
        ("config_file", "config_file"),
        ("wrapper", "wrapper"),
        ("managed_dir", "managed_dir"),
        ("dir", "managed_dir"),
    ] {
        push_target(&mut envelope, &managed, key, role);
    }
    envelope.status = if operation == "doctor" {
        if system_exists && envelope.blockers.is_empty() {
            ToolStatus::Active
        } else {
            ToolStatus::NotInstalled
        }
    } else if operation == "uninstall" && mode == "execute" {
        ToolStatus::NotInstalled
    } else if operation == "install" && mode == "execute" {
        ToolStatus::Active
    } else if system_exists {
        ToolStatus::Active
    } else {
        ToolStatus::Inactive
    };
    if matches!(
        command,
        AdapterCommand::Doctor | AdapterCommand::Status { .. }
    ) || operation == "doctor"
    {
        envelope.doctor.ok = envelope.ok;
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
