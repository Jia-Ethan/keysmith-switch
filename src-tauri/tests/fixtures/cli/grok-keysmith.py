#!/usr/bin/env python3
"""Deterministic Grok Keysmith fixture emitting grok-keysmith.envelope.v1."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

VERSION = os.environ.get("FIXTURE_VERSION", "0.4.1")
FAIL = os.environ.get("FIXTURE_FAIL")


def grok_dir(argv: list[str]) -> Path:
    if "--grok-dir" in argv:
        idx = argv.index("--grok-dir")
        return Path(argv[idx + 1])
    return Path(os.environ.get("HOME") or ".").expanduser() / ".grok"


def emit(operation: str, preview: bool, ok: bool, target: dict, plan, result, diagnostics, exit_code: int) -> int:
    envelope = {
        "schema": "grok-keysmith.envelope.v1",
        "tool": "grok",
        "version": VERSION,
        "operation": operation,
        "preview": preview,
        "apply": not preview,
        "ok": ok,
        "target": target,
        "plan": plan,
        "result": result,
        "diagnostics": diagnostics,
        "exit_code": exit_code,
    }
    print(json.dumps(envelope, indent=2, ensure_ascii=False))
    return exit_code


def status_result(root: Path) -> dict:
    rule = root / "rules" / "99-keysmith.md"
    marker = root / ".fixture-state"
    recovery = (root / ".recovery").exists()
    drift = rule.exists() and marker.exists() and rule.read_text(encoding="utf-8") != marker.read_text(encoding="utf-8")
    if recovery:
        state = "recovery-required"
    elif drift:
        state = "drift"
    elif rule.exists():
        state = "active-aligned"
    else:
        state = "not-installed"
    return {
        "state": state,
        "nodes": {
            "grok_dir": {"kind": "directory", "path": str(root)},
            "rule": {
                "kind": "regular" if rule.exists() else "missing",
                "path": str(rule),
                "fingerprint": {"sha256": "g" * 64, "size": 1} if rule.exists() else None,
            },
        },
        "conflicts": ["unmanaged edit"] if drift else [],
        "drift": ["rule fingerprint"] if drift else [],
        "recovery_required": recovery,
        "diagnostics": [],
        "exit_code": 1 if state in {"drift", "conflict", "recovery-required"} else 0,
    }


def get_flag(argv: list[str], name: str) -> str | None:
    if name in argv:
        idx = argv.index(name)
        if idx + 1 < len(argv):
            return argv[idx + 1]
    return None


def main(argv: list[str]) -> int:
    root = grok_dir(argv)
    target = {"grok_dir": str(root)}
    if FAIL == "1":
        return emit("status", True, False, target, None, None, ["fixture forced failure"], 1)
    if "--version" in argv:
        return emit(
            "version",
            True,
            True,
            {},
            None,
            {"tool": "grok", "version": VERSION},
            [],
            0,
        )
    if "--status" in argv:
        result = status_result(root)
        return emit("status", True, True, target, None, result, result.get("diagnostics") or [], result.get("exit_code") or 0)
    if "--recover" in argv:
        preview = "--yes" not in argv
        if not preview:
            marker = root / ".recovery"
            if marker.exists():
                marker.unlink()
        return emit("recover", preview, True, target, {"journals": []}, {"repaired": not preview}, [], 0)
    if "--uninstall" in argv:
        preview = "--yes" not in argv
        rule = root / "rules" / "99-keysmith.md"
        if not preview and rule.exists():
            rule.unlink()
        return emit(
            "uninstall",
            preview,
            True,
            target,
            {"blockers": [], "rule": {"path": str(rule)}},
            {"removed": not preview},
            [],
            0,
        )
    # deploy
    preview = "--yes" not in argv
    source = get_flag(argv, "--file")
    rule = root / "rules" / "99-keysmith.md"
    if not preview:
        rule.parent.mkdir(parents=True, exist_ok=True)
        body = Path(source).read_text(encoding="utf-8") if source and Path(source).exists() else "fixture-grok\n"
        rule.write_text(body, encoding="utf-8")
        (root / ".fixture-state").write_text(body, encoding="utf-8")
    return emit(
        "deploy",
        preview,
        True,
        target,
        {"blockers": [], "rule": {"path": str(rule)}, "prompt_source": source},
        {"written": not preview},
        [],
        0,
    )


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
