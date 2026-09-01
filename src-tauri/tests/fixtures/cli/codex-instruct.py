#!/usr/bin/env python3
"""Deterministic Codex Keysmith fixture emitting structured English text."""
from __future__ import annotations

import os
import sys
from pathlib import Path

VERSION = os.environ.get("FIXTURE_VERSION", "0.3.8")
FAIL = os.environ.get("FIXTURE_FAIL")


def codex_dir(argv: list[str]) -> Path:
    if "--codex-dir" in argv:
        return Path(argv[argv.index("--codex-dir") + 1])
    return Path(os.environ.get("HOME") or ".").expanduser() / ".codex"


def get_flag(argv: list[str], name: str) -> str | None:
    if name in argv:
        idx = argv.index(name)
        if idx + 1 < len(argv):
            return argv[idx + 1]
    return None


def activation(root: Path) -> str:
    prompt = root / "gpt-unrestricted.md"
    marker = root / ".fixture-state"
    if (root / ".recovery").exists():
        return "conflict"
    if prompt.exists() and marker.exists() and prompt.read_text(encoding="utf-8") != marker.read_text(encoding="utf-8"):
        return "conflict"
    if prompt.exists():
        return "active"
    return "not-installed"


def main(argv: list[str]) -> int:
    if FAIL == "1":
        print("[Error] fixture forced failure")
        return 1
    if "--version" in argv:
        print(f"codex-instruct.py {VERSION}")
        return 0
    root = codex_dir(argv)
    if "--status" in argv:
        state = activation(root)
        residue = "none" if not (root / ".recovery").exists() else "journal-1"
        print("[Status] Found 1 Codex configuration location(s) (read-only inspection):")
        print(f"── Status directory: {root} ──")
        print(f"    config.toml: regular file ({root / 'config.toml'})")
        print(f"    gpt-unrestricted.md: {'regular file' if (root / 'gpt-unrestricted.md').exists() else 'missing'} ({root / 'gpt-unrestricted.md'})")
        print(f"    hooks.json: missing ({root / 'hooks.json'})")
        print(f"    hooks.json.disabled: missing ({root / 'hooks.json.disabled'})")
        print(f"    deployment manifest: missing ({root / 'manifest.json'})")
        print("    model_instructions_file: gpt-unrestricted.md")
        print(f"    Config activation: {state}")
        print(f"    Transaction residue: {residue}")
        print("    Legacy migration: none")
        print("    Hooks status: absent")
        print("    Structural health: healthy")
        print("    Uninstall readiness: ready")
        print("    Deployability: ready")
        print("[Done] Status found no blockers; live active/disabled hooks were not read or parsed, manifest-referenced backup recovery evidence was read and hashed, and no files were changed.")
        return 1 if state == "conflict" else 0
    if "--recover" in argv:
        if "--yes" in argv:
            marker = root / ".recovery"
            if marker.exists():
                marker.unlink()
            print("[Done] Recovered fixture residue")
        else:
            print("[Preview] No files were changed; add --yes to confirm recovery.")
        return 0
    if "--uninstall" in argv:
        prompt = root / "gpt-unrestricted.md"
        print(f"  Target: {root}")
        print("    → remove gpt-unrestricted.md")
        if "--yes" in argv and prompt.exists():
            prompt.unlink()
            print("[Done] Uninstalled fixture prompt")
        else:
            print("[Preview] No files were changed; add --yes to confirm uninstall.")
        return 0
    # deploy dry-run / yes
    source = get_flag(argv, "--file") or "bundled"
    prompt = root / "gpt-unrestricted.md"
    print(f"[Prompt] Source: {source}; SHA-256: {'a' * 64}")
    print(f"  Target: {root}")
    print("    → write gpt-unrestricted.md")
    print("    → update config.toml")
    if "--yes" in argv:
        root.mkdir(parents=True, exist_ok=True)
        body = Path(source).read_text(encoding="utf-8") if source != "bundled" and Path(source).exists() else "fixture-codex\n"
        prompt.write_text(body, encoding="utf-8")
        (root / ".fixture-state").write_text(body, encoding="utf-8")
        print("[Done] deployed fixture prompt")
    else:
        print("[Preview] no files were changed; add --yes to deploy")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
