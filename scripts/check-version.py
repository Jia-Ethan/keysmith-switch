#!/usr/bin/env python3
"""Fail a release build when source version declarations drift."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def json_version(path: str) -> str:
    return str(json.loads((ROOT / path).read_text(encoding="utf-8"))["version"])


def match_version(path: str, pattern: str) -> str:
    text = (ROOT / path).read_text(encoding="utf-8")
    match = re.search(pattern, text, re.MULTILINE)
    if not match:
        raise SystemExit(f"version declaration not found: {path}")
    return match.group(1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected", required=True)
    args = parser.parse_args()

    versions = {
        "package.json": json_version("package.json"),
        "package-lock.json": json_version("package-lock.json"),
        "src-tauri/tauri.conf.json": json_version("src-tauri/tauri.conf.json"),
        "src-tauri/Cargo.toml": match_version(
            "src-tauri/Cargo.toml", r'^version\s*=\s*"([^"]+)"'
        ),
        "src-tauri/Cargo.lock": match_version(
            "src-tauri/Cargo.lock",
            r'^\[\[package\]\]\nname = "keysmith-switch"\nversion = "([^"]+)"',
        ),
        "src-tauri/src/models.rs": match_version(
            "src-tauri/src/models.rs", r'^pub const APP_VERSION: &str = "([^"]+)";'
        ),
        "src-tauri/src/updater.rs": match_version(
            "src-tauri/src/updater.rs", r'^pub const APP_VERSION: &str = "([^"]+)";'
        ),
    }
    mismatches = {path: version for path, version in versions.items() if version != args.expected}
    if mismatches:
        details = ", ".join(f"{path}={version}" for path, version in mismatches.items())
        raise SystemExit(f"expected version {args.expected}; mismatches: {details}")
    print(f"version {args.expected} is consistent across {len(versions)} declarations")


if __name__ == "__main__":
    main()
