#!/usr/bin/env python3
"""Deterministic ZCode Keysmith fixture.

doctor and install stay on text lines. uninstall emits zcode-keysmith/v1
when --json is present, matching sidecar 0.3.2.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

VERSION = os.environ.get("FIXTURE_VERSION", "0.1.0")
FAIL = os.environ.get("FIXTURE_FAIL")


def managed_dir(argv: list[str]) -> Path:
    if "--managed-dir" in argv:
        return Path(argv[argv.index("--managed-dir") + 1])
    return Path(os.environ.get("HOME") or ".").expanduser() / ".zcode-keysmith"


def get_flag(argv: list[str], name: str) -> str | None:
    if name in argv:
        idx = argv.index(name)
        if idx + 1 < len(argv):
            return argv[idx + 1]
    return None


def main(argv: list[str]) -> int:
    if FAIL == "1":
        print("zcode-keysmith error: fixture forced failure")
        return 1
    if "--version" in argv:
        print(f"zcode-keysmith.py {VERSION}")
        return 0
    command = argv[0] if argv else "doctor"
    root = managed_dir(argv)
    system = root / "system-role.md"
    if command == "doctor":
        print("zcode-keysmith doctor")
        print(f"managed_dir: {root}")
        print(f"system_file: {system}")
        print(f"system_file_exists: {str(system.exists()).lower()}")
        print(f"system_file_sha256: {'b' * 64 if system.exists() else 'missing'}")
        print(f"config_file: {root / 'config.json'}")
        print(f"wrapper: {root / 'bin' / 'zcode-agent-wrapper.py'}")
        print(f"launch_agent: {Path.home() / 'Library' / 'LaunchAgents' / 'com.jia.zcode-keysmith.env.plist'}")
        return 0
    if command == "install":
        preview = "--yes" not in argv or "--dry-run" in argv
        source = get_flag(argv, "--system-file") or "examples/system-role.md"
        print("zcode-keysmith install preview" if preview else "zcode-keysmith install complete")
        print(f"source_system_file: {source}")
        print(f"system_file: {system}")
        print(f"config_file: {root / 'config.json'}")
        print(f"wrapper: {root / 'bin' / 'zcode-agent-wrapper.py'}")
        print(f"launch_agent: {Path.home() / 'Library' / 'LaunchAgents' / 'com.jia.zcode-keysmith.env.plist'}")
        print(f"write: {str(not preview).lower()}")
        if not preview:
            root.mkdir(parents=True, exist_ok=True)
            body = Path(source).read_text(encoding="utf-8") if Path(source).exists() else "fixture-zcode\n"
            system.write_text(body, encoding="utf-8")
        return 0
    if command == "uninstall":
        preview = "--yes" not in argv or "--dry-run" in argv
        removed: list[str] = []
        if not preview and system.exists():
            removed.append(str(system))
            system.unlink()
        if "--json" in argv:
            payload = {
                "schema": "zcode-keysmith/v1",
                "operation": "uninstall",
                "mode": "preview" if preview else "execute",
                "ok": True,
                "actions": [
                    {"action": "plan" if preview else "remove", "path": str(system), "detail": "target"}
                ],
                "warnings": [],
                "blockers": [],
                "exit_status": 0,
                "error": None,
                "managed_dir": str(root),
                "write": not preview,
                "removed": removed,
                "activation": [],
                "backups": [],
            }
            print(json.dumps(payload, ensure_ascii=False, indent=2))
            return 0
        print("zcode-keysmith uninstall preview" if preview else "zcode-keysmith uninstall complete")
        print(f"target: {system}")
        print(f"write: {str(not preview).lower()}")
        for path in removed:
            print(f"removed: {path}")
        return 0
    print(f"unknown command {command}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
