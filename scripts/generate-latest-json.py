#!/usr/bin/env python3
"""Generate a local latest.json + .sig workflow notes. Does not publish."""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--notes", default="")
    parser.add_argument("--darwin-aarch64", type=Path)
    parser.add_argument("--windows-x86_64", type=Path)
    parser.add_argument("--out", type=Path, default=Path("latest.json"))
    args = parser.parse_args()
    platforms = {}
    for key, path in (
        ("darwin-aarch64", args.darwin_aarch64),
        ("windows-x86_64", args.windows_x86_64),
    ):
        if path and path.is_file():
            sig = path.with_suffix(path.suffix + ".sig")
            platforms[key] = {
                "url": f"file://{path.resolve()}",
                "signature": sig.read_text().strip() if sig.is_file() else "MISSING_SIG",
                "sha256": sha256(path),
            }
    payload = {
        "version": args.version,
        "notes": args.notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    args.out.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
