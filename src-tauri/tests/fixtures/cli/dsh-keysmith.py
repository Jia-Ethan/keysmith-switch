#!/usr/bin/env python3
"""Deterministic dsh Keysmith fixture emitting dsh-keysmith/v1.

Mirrors dsh-keysmith 0.2.1's payload shape so the adapter contract can be tested without
the real CLI. State comes from files under --dsh-dir, in the same order the real adapter
decides it:

- any `.dsh-keysmith-transaction-*` directory: recovery-required
- no `.dsh-keysmith-manifest.json`: not-installed
- manifest present, every managed file matching its recorded `after`: active
- manifest present, any managed file differing or missing: drift

Environment switches, mirroring the behaviours the real adapter has:
- FIXTURE_VERSION=0.2.0  answers the old contract (no `version`, `state`, `dsh_dir_exists`)
- FIXTURE_DIR_MISSING=1  makes `--dsh-dir` point at a path that does not exist
- FIXTURE_ADAPTER_STATE    the adapter_patch state to report (default "absent")
- FIXTURE_LANG_ECHO=1    repeats the `--lang` it was given, so a test can prove Switch
                         always asks for English
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path

SCHEMA = "dsh-keysmith/v1"
VERSION = os.environ.get("FIXTURE_VERSION", "0.2.1")
MANIFEST = ".dsh-keysmith-manifest.json"
AGENTS = "AGENTS.md"
PATCH = "cordis.patch.yml"
ADAPTER = "profiles/node_modules/@deepseek-ai/dsh-llm-deepseek/lib/index.js"
RESOURCES = {AGENTS: "agents_md", PATCH: "cordis_patch", ADAPTER: "adapter_patch"}


def flag(argv: list[str], name: str) -> str | None:
    if name in argv:
        idx = argv.index(name)
        if idx + 1 < len(argv):
            return argv[idx + 1]
    return None


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fingerprint(path: Path) -> dict | None:
    if not path.is_file():
        return None
    data = path.read_bytes()
    return {"sha256": sha(data), "size": len(data), "mtime_ns": 0}


def load_manifest(root: Path) -> dict | None:
    try:
        raw = json.loads((root / MANIFEST).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return raw if isinstance(raw, dict) else None


def residue(root: Path) -> list[dict]:
    items = []
    for entry in sorted(root.glob(".dsh-keysmith-transaction-*")):
        items.append({"kind": "transaction", "id": entry.name, "path": str(entry),
                      "phase": "prepared", "operation": "deploy"})
    return items


def drift_notes(root: Path, manifest: dict) -> list[str]:
    notes = []
    for name, key in RESOURCES.items():
        entry = (manifest.get("resources") or {}).get(key)
        if entry is None:
            continue
        live = fingerprint(root / name)
        if live is None:
            notes.append(f"{key}: missing since deploy: {root / name}")
        elif live["sha256"] != entry.get("after_sha256"):
            notes.append(f"{key}: changed since deploy: {root / name}")
    return notes


def inspect(root: Path) -> tuple[str, list[str], dict | None, list[dict]]:
    left = residue(root)
    manifest = load_manifest(root)
    if left:
        return "recovery-required", [], manifest, left
    if manifest is None:
        return "not-installed", [], None, []
    notes = drift_notes(root, manifest)
    return ("drift" if notes else "active"), notes, manifest, []


def manifest_public(manifest: dict | None) -> dict | None:
    if manifest is None:
        return None
    resources = {}
    for key in set(RESOURCES.values()):
        entry = (manifest.get("resources") or {}).get(key)
        if entry is None:
            continue
        resources[key] = {"path": entry.get("path"), "backup": entry.get("backup"),
                          "after_sha256": entry.get("after_sha256")}
    return {"deployment_id": manifest.get("deployment_id"),
            "tool_version": manifest.get("tool_version"),
            "created_at": manifest.get("created_at"),
            "resources": resources}


def report(operation: str, mode: str, ok: bool, exit_status: int, *, actions=None,
           warnings=None, blockers=None, error=None, extra=None) -> dict:
    payload = {
        "schema": SCHEMA,
        "operation": operation,
        "mode": mode,
        "ok": ok,
        "actions": actions or [],
        "warnings": warnings or [],
        "blockers": blockers or [],
        "exit_status": exit_status,
        "error": error,
    }
    if VERSION != "0.2.0":
        payload["version"] = VERSION
    if extra:
        payload.update(extra)
    return payload


def action(kind: str, path: Path | str, detail: str = "") -> dict:
    return {"action": kind, "path": str(path), "detail": detail}


def dump(payload: dict) -> None:
    print(json.dumps(payload, indent=2))


def extra_for(root: Path, with_state: bool = True) -> dict:
    """The payload fields dsh-keysmith 0.2.1 sends.

    Only `--status` and `--recover` report `state`/`drift`/`dsh_dir_exists`; deploy and
    uninstall describe the operation they performed and leave the state out. Keeping that
    asymmetry here is the point — the adapter has to cope with it.
    """
    state, notes, manifest, left = inspect(root)
    extra = {
        "dsh_dir": str(root),
        "ready": not left,
        "residue": left,
        "manifest": manifest_public(manifest),
        "adapter_patch": {
            "state": os.environ.get("FIXTURE_ADAPTER_STATE", "absent"),
            "path": str(root / ADAPTER),
            "managed": bool((manifest or {}).get("resources", {}).get("adapter_patch")),
            "already_patched": None,
        },
    }
    if VERSION != "0.2.0" and with_state:
        extra["dsh_dir_exists"] = root.is_dir()
        extra["state"] = state
        extra["drift"] = notes
    return extra


def status(root: Path, lang: str | None) -> int:
    state, _notes, _manifest, left = inspect(root)
    ready = not left
    blockers = [f"unfinished transaction residue: {item['path']}" for item in left] if left else []
    payload = report("status", "preview", ready, 0 if ready else 1, blockers=blockers,
                     extra=extra_for(root))
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        payload["lang"] = lang
    dump(payload)
    return 0 if ready else 1


def deploy(root: Path, lang: str | None, name: str, yes: bool) -> int:
    mode = "execute" if yes else "preview"
    if not root.is_dir():
        message = f"DSH directory does not exist: {root}"
        dump(report("deploy", mode, False, 2, error=message, blockers=[message],
                    extra={"dsh_dir": str(root), "name": name}))
        return 2
    _state, _notes, _manifest, left = inspect(root)
    if left:
        blockers = [f"unfinished transaction residue: {item['path']}" for item in left]
        dump(report("deploy", mode, False, 1, blockers=blockers,
                    extra={**extra_for(root, with_state=False), "name": name}))
        return 1
    source = flag(__import__("sys").argv, "--file")
    data = Path(source).read_bytes() if source else b"# fixture dsh\n"
    actions = [
        action("replace" if (root / AGENTS).is_file() else "create", root / AGENTS, "agents_md"),
        action("replace" if (root / PATCH).is_file() else "create", root / PATCH, "cordis_patch"),
        action("write", root / MANIFEST, "manifest"),
        action("skip", root / ADAPTER, "absent"),
    ]
    warnings = ["adapter file absent; skipping adapter patch"]
    extra = {**extra_for(root, with_state=False), "name": name,
             "source": {"kind": "file" if source else "builtin", "path": source},
             "target": {"agents_md": str(root / AGENTS), "cordis_patch": str(root / PATCH)}}
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        extra["lang"] = lang
    if not yes:
        dump(report("deploy", "preview", True, 0, actions=actions, warnings=warnings, extra=extra))
        return 0
    (root / AGENTS).write_bytes(data)
    (root / PATCH).write_bytes(b"id: system-prompt\n")
    manifest = {
        "schema_version": 1,
        "tool_version": VERSION,
        "deployment_id": "0" * 32,
        "created_at": "2026-01-01T00:00:00+00:00",
        "resources": {
            "agents_md": {"path": AGENTS, "after_sha256": sha(data), "before": None, "backup": None},
            "cordis_patch": {"path": PATCH, "after_sha256": sha(b"id: system-prompt\n"),
                             "before": None, "backup": None},
        },
        "previous_manifest": {"fingerprint": None},
    }
    (root / MANIFEST).write_text(json.dumps(manifest), encoding="utf-8")
    dump(report("deploy", "execute", True, 0, actions=actions, warnings=warnings,
                extra={**extra, "deployment_id": "0" * 32,
                       "manifest": manifest_public(manifest)}))
    return 0


def uninstall(root: Path, lang: str | None, yes: bool) -> int:
    mode = "execute" if yes else "preview"
    manifest = load_manifest(root)
    if manifest is None:
        message = "no dsh-keysmith manifest found; nothing to uninstall"
        dump(report("uninstall", mode, False, 1, error=message, blockers=[message],
                    extra={"dsh_dir": str(root)}))
        return 1
    _state, notes, _manifest, left = inspect(root)
    if left:
        blockers = [f"unfinished transaction residue: {item['path']}" for item in left]
        dump(report("uninstall", mode, False, 1, blockers=blockers,
                    extra={**extra_for(root)}))
        return 1
    if notes:
        # The real adapter names the first drifted file and refuses.
        target = notes[0].split(": ", 1)[-1]
        message = f"target drifted from the manifest, refusing to overwrite: {target}"
        dump(report("uninstall", mode, False, 1, error=message, blockers=[message],
                    extra={"dsh_dir": str(root), "manifest": manifest_public(manifest), "adapter_patch": extra_for(root, with_state=False)["adapter_patch"]}))
        return 1
    actions = [action("remove", root / AGENTS, "agents_md"),
               action("remove", root / PATCH, "cordis_patch"),
               action("backup", root / f"{MANIFEST}.uninstalled", "manifest")]
    extra = {**extra_for(root, with_state=False), "manifest": manifest_public(manifest)}
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        extra["lang"] = lang
    if not yes:
        dump(report("uninstall", "preview", True, 0, actions=actions, extra=extra))
        return 0
    (root / AGENTS).unlink(missing_ok=True)
    (root / PATCH).unlink(missing_ok=True)
    (root / MANIFEST).unlink(missing_ok=True)
    dump(report("uninstall", "execute", True, 0, actions=actions,
                extra={**extra, "manifest": None}))
    return 0


def recover(root: Path, lang: str | None, yes: bool) -> int:
    mode = "execute" if yes else "preview"
    left = residue(root)
    extra = {"dsh_dir": str(root), "residue": left}
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        extra["lang"] = lang
    if not left:
        dump(report("recover", mode, True, 0, extra=extra))
        return 0
    actions = [action("remove", item["path"], "transaction") for item in left]
    if not yes:
        dump(report("recover", "preview", True, 0, actions=actions, extra=extra))
        return 0
    for item in left:
        (root / item["id"]).rmdir()
    dump(report("recover", "execute", True, 0, actions=actions,
                extra={"dsh_dir": str(root), "residue": []}))
    return 0


def main(argv: list[str]) -> int:
    if "--version" in argv:
        print(f"dsh-keysmith {VERSION}")
        return 0
    lang = flag(argv, "--lang")
    root = Path(flag(argv, "--dsh-dir") or (Path.home() / ".dsh"))
    if os.environ.get("FIXTURE_DIR_MISSING") == "1":
        root = Path("/nonexistent/fixture-dsh")
    name = flag(argv, "--name") or "dsh-unrestricted"
    yes = "--yes" in argv
    if "--status" in argv:
        return status(root, lang)
    if "--uninstall" in argv:
        return uninstall(root, lang, yes)
    if "--recover" in argv:
        return recover(root, lang, yes)
    return deploy(root, lang, name, yes)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
