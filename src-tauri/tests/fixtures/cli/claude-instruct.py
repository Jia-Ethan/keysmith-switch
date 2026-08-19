#!/usr/bin/env python3
"""Deterministic Claude Keysmith fixture for adapter contract tests."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

VERSION = os.environ.get("FIXTURE_VERSION", "v7.1")
FAIL = os.environ.get("FIXTURE_FAIL")


def home() -> Path:
    return Path(os.environ.get("CLAUDE_KEYSMITH_HOME") or os.environ.get("HOME") or ".").expanduser()


def memory_file() -> Path:
    return home() / ".claude" / "CLAUDE.md"


def instruction_file(name: str) -> Path:
    return home() / ".claude" / "keysmith" / f"{name}.md"


def dump(payload: dict, exit_status: int = 0) -> int:
    payload.setdefault("schema", "claude-keysmith/v1")
    payload.setdefault("exit_status", exit_status)
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return exit_status


def get_flag(argv: list[str], name: str, default: str | None = None) -> str | None:
    if name in argv:
        idx = argv.index(name)
        if idx + 1 < len(argv):
            return argv[idx + 1]
    return default


def installed_name(argv: list[str]) -> str:
    return get_flag(argv, "--name", "claude-project-rules") or "claude-project-rules"


def status_payload(argv: list[str]) -> dict:
    name = installed_name(argv)
    memory = memory_file()
    instruction = instruction_file(name)
    marker = home() / ".claude" / "keysmith" / ".fixture-state"
    drift = False
    recovery = (home() / ".claude" / "keysmith" / ".recovery").exists()
    if instruction.exists() and marker.exists():
        drift = instruction.read_text(encoding="utf-8") != marker.read_text(encoding="utf-8")
    installed = memory.exists() and instruction.exists() and not drift
    return {
        "scope": get_flag(argv, "--scope", "user"),
        "root": str(home() / ".claude"),
        "memory_file": str(memory),
        "instruction_file": str(instruction),
        "import_target": f"@keysmith/{name}.md",
        "memory_file_exists": memory.exists(),
        "instruction_file_exists": instruction.exists(),
        "import_block_exists": installed,
        "installed": installed,
        "presence": {
            "memory_file": memory.exists(),
            "instruction_file": instruction.exists(),
            "import_block": installed,
        },
        "source_identity": {
            "kind": "deployed" if instruction.exists() else "missing",
            "instruction_sha256": "abc123" if instruction.exists() else None,
            "instruction_size_bytes": instruction.stat().st_size if instruction.exists() else None,
            "drift": drift if instruction.exists() else None,
        },
        "recovery_state": {
            "journals": [],
            "journal_count": 0,
            "atomic_temp_files": [],
            "atomic_temp_count": 0,
            "conflicts": [],
            "lock_present": False,
            "lock_live": False,
            "recovery_required": recovery,
            "must_recover_before_writes": recovery,
        },
        "ok": True,
        "operation": "status",
    }


def main(argv: list[str]) -> int:
    if FAIL == "1":
        return dump(
            {
                "operation": argv[0] if argv else "unknown",
                "mode": "preview",
                "ok": False,
                "error": "fixture forced failure",
                "blockers": ["fixture forced failure"],
                "warnings": [],
                "actions": [],
                "backups": [],
            },
            1,
        )
    if "--version" in argv:
        print(f"claude-keysmith {VERSION}")
        return 0
    if not argv:
        return dump({"ok": False, "error": "missing command", "blockers": ["missing command"]}, 2)

    command = argv[0]
    yes = "--yes" in argv
    name = installed_name(argv)
    source = get_flag(argv, "--file")

    if command == "status":
        return dump(status_payload(argv), 0)
    if command == "doctor":
        print(
            json.dumps(
                {
                    "installation_type": "path",
                    "upstream_candidates": [],
                    "upstream_path": str(home() / "fake-claude"),
                    "system_prompt_file": str(home() / ".claude" / "keysmith" / "system-prompt.md"),
                    "append_prompt_file": str(home() / ".claude" / "keysmith" / "append-prompt.md"),
                    "settings_file": str(home() / ".claude" / "settings.json"),
                    "shell_kind": "zsh",
                    "shell_rc": str(home() / ".zshrc"),
                    "repair_actions": ["No repair action required."],
                },
                indent=2,
            )
        )
        return 0
    if command == "install":
        memory = memory_file()
        instruction = instruction_file(name)
        actions = [
            {"action": "write", "path": str(memory), "detail": "install/update managed import block"},
            {"action": "write", "path": str(instruction), "detail": "write keysmith instruction file"},
        ]
        if yes:
            memory.parent.mkdir(parents=True, exist_ok=True)
            instruction.parent.mkdir(parents=True, exist_ok=True)
            body = Path(source).read_text(encoding="utf-8") if source and Path(source).exists() else "fixture-body\n"
            instruction.write_text(body, encoding="utf-8")
            memory.write_text(f"<!-- claude-keysmith:start name={name} -->\n@fixture\n", encoding="utf-8")
            (instruction.parent / ".fixture-state").write_text(body, encoding="utf-8")
        return dump(
            {
                "operation": "install",
                "mode": "execute" if yes else "preview",
                "ok": True,
                "scope": get_flag(argv, "--scope", "user"),
                "name": name,
                "target": {"memory_file": str(memory), "instruction_file": str(instruction)},
                "actions": actions,
                "warnings": [],
                "blockers": [],
                "backups": [],
                "reload_required": False,
                "reload_hint": None,
                "error": None,
            }
        )
    if command == "uninstall":
        memory = memory_file()
        instruction = instruction_file(name)
        if yes:
            if memory.exists():
                memory.unlink()
            if instruction.exists():
                instruction.unlink()
        return dump(
            {
                "operation": "uninstall",
                "mode": "execute" if yes else "preview",
                "ok": True,
                "scope": get_flag(argv, "--scope", "user"),
                "actions": [
                    {"action": "write", "path": str(memory), "detail": "remove managed import block"},
                    {"action": "remove", "path": str(instruction), "detail": "remove keysmith instruction file"},
                ],
                "warnings": [],
                "blockers": [],
                "backups": [
                    {
                        "target": str(memory),
                        "backup_path": None if not yes else str(memory) + ".bak",
                        "planned": not yes,
                    }
                ],
                "error": None,
            }
        )
    if command == "recover":
        marker = home() / ".claude" / "keysmith" / ".recovery"
        if yes and marker.exists():
            marker.unlink()
        return dump(
            {
                "operation": "recover",
                "mode": "execute" if yes else "preview",
                "ok": True,
                "scope": get_flag(argv, "--scope", "user"),
                "actions": [{"action": "noop" if not marker.exists() else "rollback-pending", "path": str(marker), "detail": "fixture recover"}],
                "residue": [],
                "warnings": [],
                "blockers": [],
                "backups": [],
                "error": None,
            }
        )
    return dump({"ok": False, "error": f"unknown command {command}", "blockers": [command]}, 2)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
