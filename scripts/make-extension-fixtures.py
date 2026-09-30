#!/usr/bin/env python3
"""Regenerate src-tauri/fixtures/extensions from the extensions repo's own tooling.

    python3 scripts/make-extension-fixtures.py ../extensions

Signs with the TEST ONLY key next to the output. That key is public on purpose,
exactly like the updater's; it must never be the key an app trusts in production.
Needs Node (for the Tauri signer) and the extensions repo checked out.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "fixtures" / "extensions"
KEY = OUT / "TEST_ONLY.key"
PASSWORD = "test-only"
BASE = "https://github.com/Jia-Ethan/keysmith-switch-extensions/releases/download"


def write_pack(packs: Path, pack_id: str, version: str, min_app: str, items: dict) -> None:
    directory = packs / pack_id
    (directory / "prompts").mkdir(parents=True)
    manifest_items = []
    for item_id, (tool, title, text) in items.items():
        name = f"prompts/{item_id}.md"
        (directory / name).write_text(text, encoding="utf-8")
        manifest_items.append({"id": item_id, "tool": tool, "title": {"zh-CN": title, "en": title}, "tags": ["fixture"], "file": name, "sha256": ""})
    manifest = {
        "schema": 1,
        "id": pack_id,
        "version": version,
        "min_app_version": min_app,
        "kind": "prompts",
        "name": {"zh-CN": f"测试包 {pack_id}", "en": f"Fixture {pack_id}"},
        "description": {"en": "Fixture pack for tests"},
        "tools": sorted({tool for tool, _, _ in items.values()}),
        "items": manifest_items,
    }
    (directory / "pack.json").write_text(json.dumps(manifest), encoding="utf-8")


def sign(path: Path) -> None:
    subprocess.run(
        ["npx", "tauri", "signer", "sign", "-f", str(KEY), "-p", PASSWORD, str(path)],
        check=True, cwd=ROOT, stdout=subprocess.DEVNULL,
    )


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    sys.path.insert(0, str(Path(sys.argv[1]).resolve() / "scripts"))
    import packlib  # type: ignore

    OUT.mkdir(parents=True, exist_ok=True)
    if not KEY.exists():
        subprocess.run(["npx", "tauri", "signer", "generate", "--ci", "-p", PASSWORD, "-w", str(KEY)], check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    releases = {
        "v1": {"fixture.pack": ("0.1.0", "0.2.5", {
            "alpha": ("claude", "Alpha", "Alpha, first text.\n"),
            "beta": ("codex", "Beta", "Beta text.\n"),
        })},
        "v2": {
            "fixture.pack": ("0.2.0", "0.2.5", {
                "alpha": ("claude", "Alpha", "Alpha, second text.\n"),
                "beta": ("codex", "Beta", "Beta text.\n"),
                "gamma": ("claude", "Gamma", "Gamma, new in 0.2.0.\n"),
            }),
            "fixture.future": ("1.0.0", "99.0.0", {"omega": ("claude", "Omega", "Needs a much newer app.\n")}),
        },
        "v3": {"fixture.pack": ("0.1.0", "0.2.5", {"alpha": ("claude", "Alpha", "Alpha, first text.\n")})},
    }
    for name, packs_spec in releases.items():
        target = OUT / name
        shutil.rmtree(target, ignore_errors=True)
        with tempfile.TemporaryDirectory() as tmp:
            packs = Path(tmp) / "packs"
            packs.mkdir()
            for pack_id, (version, min_app, items) in packs_spec.items():
                write_pack(packs, pack_id, version, min_app, items)
                # seal: fill in the hashes
                manifest_path = packs / pack_id / "pack.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                for item in manifest["items"]:
                    item["sha256"] = packlib.sha256_hex((packs / pack_id / item["file"]).read_bytes())
                manifest_path.write_bytes(packlib.canonical_json(manifest))
            packlib.build_index(packs, target, f"{BASE}/test-{name}", "2026-09-30T00:00:00Z")
            packlib.verify_release(target)
        if name == "v3":
            # An index from a format this app does not know yet: signed, but too new.
            index = json.loads((target / "index.json").read_text(encoding="utf-8"))
            index["schema"] = 2
            (target / "index.json").write_bytes(packlib.canonical_json(index))
        sign(target / "index.json")
        print(f"built {name}: {sorted(p.name for p in target.iterdir())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
