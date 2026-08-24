#!/usr/bin/env python3
"""Generate Tauri updater metadata from signed release artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import quote, urlparse


SEMVER_RE = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def artifact_entry(path: Path, base_url: str) -> dict[str, object]:
    if not path.is_file():
        raise SystemExit(f"artifact does not exist: {path}")
    signature_path = Path(f"{path}.sig")
    if not signature_path.is_file():
        raise SystemExit(f"signature does not exist: {signature_path}")
    signature = signature_path.read_text(encoding="utf-8").strip()
    if not signature:
        raise SystemExit(f"signature is empty: {signature_path}")
    size = path.stat().st_size
    if size <= 0:
        raise SystemExit(f"artifact is empty: {path}")
    return {
        "url": f"{base_url.rstrip('/')}/{quote(path.name, safe='')}",
        "signature": signature,
        "sha256": sha256(path),
        "size": size,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--minimum-updater-version", required=True)
    parser.add_argument("--notes", default="")
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--darwin-aarch64", type=Path)
    parser.add_argument("--windows-x86_64", type=Path)
    parser.add_argument("--linux-x86_64", type=Path)
    parser.add_argument("--out", type=Path, default=Path("latest.json"))
    args = parser.parse_args()

    version = args.version.strip()
    if not version:
        raise SystemExit("version must not be empty")
    minimum_updater_version = args.minimum_updater_version.strip()
    if not SEMVER_RE.fullmatch(minimum_updater_version):
        raise SystemExit("minimum updater version must be a complete semantic version")
    parsed_url = urlparse(args.base_url)
    if parsed_url.scheme != "https" or not parsed_url.netloc:
        raise SystemExit("base URL must be an absolute HTTPS URL")
    if parsed_url.query or parsed_url.fragment:
        raise SystemExit("base URL must not contain a query or fragment")

    platforms = {}
    for key, path in (
        ("darwin-aarch64", args.darwin_aarch64),
        ("windows-x86_64", args.windows_x86_64),
        ("linux-x86_64", args.linux_x86_64),
    ):
        if path is not None:
            platforms[key] = artifact_entry(path, args.base_url)
    if not platforms:
        raise SystemExit("at least one updater artifact is required")

    payload = {
        "version": version,
        "minimum_updater_version": minimum_updater_version,
        "notes": args.notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    if args.out.exists() and args.out.is_dir():
        raise SystemExit(f"output path is a directory: {args.out}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
