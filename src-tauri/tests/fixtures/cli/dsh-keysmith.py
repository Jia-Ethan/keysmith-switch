#!/usr/bin/env python3
"""Deterministic dsh Keysmith fixture emitting dsh-keysmith/v1.

Mirrors dsh-keysmith 0.4.0's payload shape so the adapter contract can be tested
without the real CLI. State comes from files under --dsh-dir:

- no `.dsh-keysmith-manifest.json` and no live plugin/bundles/insert: not-installed
- manifest present (or leftover plugin pieces) matching the live profile: active
- plugin / bundles / insert mismatch: drift

`--file` overlays `profiles/<profile>/node_modules/dsh-keysmith/lib/contract.md`.
Home `AGENTS.md` is never written. `--recover` is a no-op.

Environment switches:
- FIXTURE_VERSION=0.2.0  answers the old contract (no `version`, `state`, `dsh_dir_exists`)
- FIXTURE_DIR_MISSING=1  makes `--dsh-dir` point at a path that does not exist
- FIXTURE_LANG_ECHO=1    repeats the `--lang` it was given
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

SCHEMA = "dsh-keysmith/v1"
VERSION = os.environ.get("FIXTURE_VERSION", "0.4.0")
MANIFEST = ".dsh-keysmith-manifest.json"
PLUGIN = "dsh-keysmith"
DEFAULT_PROFILE = "web"
INSERT_BLOCK = "- insert:\n    - id: dsh-keysmith\n      name: dsh-keysmith\n"


def flag(argv: list[str], name: str) -> str | None:
    if name in argv:
        idx = argv.index(name)
        if idx + 1 < len(argv):
            return argv[idx + 1]
    return None


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def profile_dir(root: Path, profile: str) -> Path:
    return root / "profiles" / profile


def plugin_dest(root: Path, profile: str) -> Path:
    return profile_dir(root, profile) / "node_modules" / PLUGIN


def load_manifest(root: Path) -> dict | None:
    try:
        raw = json.loads((root / MANIFEST).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return raw if isinstance(raw, dict) else None


def has_insert(text: str) -> bool:
    return "- id: dsh-keysmith" in text


def live_state(root: Path, profile: str) -> tuple[bool, bool, bool]:
    dest = plugin_dest(root, profile)
    pkg_path = profile_dir(root, profile) / "package.json"
    try:
        pkg = json.loads(pkg_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        pkg = {}
    bundles = ((pkg.get("dsh") or {}).get("profile") or {}).get("bundles") or []
    in_bundles = PLUGIN in bundles
    patch = profile_dir(root, profile) / "cordis.patch.yml"
    inserted = has_insert(patch.read_text(encoding="utf-8")) if patch.is_file() else False
    copied = dest.is_dir() and (dest / "lib" / "index.js").is_file()
    return copied, in_bundles, inserted


def inspect(root: Path, profile: str) -> tuple[str, list[str], dict | None]:
    manifest = load_manifest(root)
    used = str((manifest or {}).get("profile") or profile)
    copied, in_bundles, inserted = live_state(root, used)
    notes: list[str] = []
    if manifest is None:
        if copied or in_bundles or inserted:
            if not copied:
                notes.append("profile_plugin: missing plugin copy")
            if not in_bundles:
                notes.append("profile_bundles: dsh-keysmith not in bundles")
            if not inserted:
                notes.append("profile_insert: insert missing")
            return ("drift" if notes else "active"), notes, None
        return "not-installed", [], None
    if not copied:
        notes.append("profile_plugin: missing since deploy")
    if not in_bundles:
        notes.append("profile_bundles: changed since deploy")
    if not inserted:
        notes.append("profile_insert: changed since deploy")
    return ("drift" if notes else "active"), notes, manifest


def manifest_public(manifest: dict | None) -> dict | None:
    if manifest is None:
        return None
    resources = {}
    for key in ("profile_plugin", "profile_bundles", "profile_insert"):
        entry = manifest.get(key) or (manifest.get("resources") or {}).get(key)
        if entry is None:
            continue
        after = entry.get("after") or {}
        resources[key] = {
            "path": entry.get("path"),
            "backup": entry.get("backup"),
            "after_sha256": after.get("sha256") or entry.get("after_sha256"),
        }
    return {
        "deployment_id": manifest.get("deployment_id"),
        "tool_version": manifest.get("tool_version"),
        "created_at": manifest.get("created_at"),
        "profile": manifest.get("profile"),
        "resources": resources,
    }


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


def extra_for(root: Path, profile: str, with_state: bool = True) -> dict:
    state, notes, manifest = inspect(root, profile)
    extra = {
        "dsh_dir": str(root),
        "profile": (manifest or {}).get("profile") or profile,
        "ready": True,
        "residue": [],
        "manifest": manifest_public(manifest),
        "adapter_patch": {"state": "absent", "path": None, "managed": False, "already_patched": None},
    }
    if VERSION != "0.2.0" and with_state:
        extra["dsh_dir_exists"] = root.is_dir()
        extra["state"] = state
        extra["drift"] = notes
    return extra


def status(root: Path, lang: str | None, profile: str) -> int:
    payload = report("status", "preview", True, 0, extra=extra_for(root, profile))
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        payload["lang"] = lang
    dump(payload)
    return 0


def write_plugin(dest: Path, overlay: bytes | None) -> None:
    lib = dest / "lib"
    lib.mkdir(parents=True, exist_ok=True)
    (lib / "index.js").write_text("export default {}\n", encoding="utf-8")
    body = overlay if overlay is not None else b"# fixture dsh contract\n"
    (lib / "contract.md").write_bytes(body)


def deploy(root: Path, lang: str | None, name: str, yes: bool, profile: str, source: str | None) -> int:
    mode = "execute" if yes else "preview"
    if not root.is_dir():
        message = f"DSH directory does not exist: {root}"
        dump(report("deploy", mode, False, 2, error=message, blockers=[message],
                    extra={"dsh_dir": str(root), "dsh_dir_exists": False, "name": name,
                           "state": "not-installed"}))
        return 2
    dest = plugin_dest(root, profile)
    pkg_path = profile_dir(root, profile) / "package.json"
    patch_path = profile_dir(root, profile) / "cordis.patch.yml"
    actions = [
        action("create" if not dest.exists() else "replace", dest, "profile_plugin"),
        action("update", pkg_path, "profile_bundles"),
        action("update", patch_path, "profile_insert"),
    ]
    warnings = ["does not patch the host or write AGENTS.md"]
    extra = {**extra_for(root, profile, with_state=False), "name": name, "profile": profile}
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        extra["lang"] = lang
    if not yes:
        extra["state"] = "not-installed"
        extra["ready"] = True
        extra["dsh_dir_exists"] = True
        dump(report("deploy", "preview", True, 0, actions=actions, warnings=warnings, extra=extra))
        return 0
    overlay = Path(source).read_bytes() if source else None
    write_plugin(dest, overlay)
    pkg = {}
    if pkg_path.is_file():
        try:
            pkg = json.loads(pkg_path.read_text(encoding="utf-8"))
        except ValueError:
            pkg = {}
    dsh = pkg.setdefault("dsh", {})
    profile_obj = dsh.setdefault("profile", {})
    bundles = list(profile_obj.get("bundles") or [])
    if PLUGIN not in bundles:
        bundles.append(PLUGIN)
    profile_obj["bundles"] = bundles
    pkg_path.parent.mkdir(parents=True, exist_ok=True)
    pkg_path.write_text(json.dumps(pkg, indent=2) + "\n", encoding="utf-8")
    existing = patch_path.read_text(encoding="utf-8") if patch_path.is_file() else ""
    if not has_insert(existing):
        patch_path.write_text(existing + ("" if not existing or existing.endswith("\n") else "\n") + INSERT_BLOCK, encoding="utf-8")
    manifest = {
        "schema_version": 1,
        "tool_version": VERSION,
        "deployment_id": "0" * 32,
        "created_at": "2026-01-01T00:00:00+00:00",
        "profile": profile,
        "profile_plugin": {
            "path": str(dest),
            "after": {"sha256": sha((dest / "lib" / "index.js").read_bytes())},
            "backup": None,
        },
        "profile_bundles": {"path": str(pkg_path), "after": {"sha256": sha(pkg_path.read_bytes())}, "backup": None},
        "profile_insert": {"path": str(patch_path), "after": {"sha256": sha(patch_path.read_bytes())}, "backup": None},
        "previous_manifest": {"fingerprint": None},
    }
    (root / MANIFEST).write_text(json.dumps(manifest), encoding="utf-8")
    dump(report("deploy", "execute", True, 0, actions=actions, warnings=warnings,
                extra={**extra, "deployment_id": "0" * 32, "state": "active",
                       "manifest": manifest_public(manifest)}))
    return 0


def uninstall(root: Path, lang: str | None, yes: bool, profile: str) -> int:
    mode = "execute" if yes else "preview"
    manifest = load_manifest(root)
    if manifest is None:
        message = "No dsh-keysmith manifest found; nothing to uninstall."
        dump(report("uninstall", mode, False, 1, error=message, blockers=[message],
                    extra={"dsh_dir": str(root), "state": "not-installed",
                           "adapter_patch": extra_for(root, profile, with_state=False)["adapter_patch"]}))
        return 1
    used = str(manifest.get("profile") or profile)
    dest = plugin_dest(root, used)
    pkg_path = profile_dir(root, used) / "package.json"
    patch_path = profile_dir(root, used) / "cordis.patch.yml"
    actions = [
        action("delete", dest, "profile_plugin"),
        action("update", pkg_path, "profile_bundles"),
        action("update", patch_path, "profile_insert"),
        action("delete", root / MANIFEST, "manifest"),
    ]
    extra = {**extra_for(root, used, with_state=False), "profile": used, "manifest": manifest_public(manifest)}
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        extra["lang"] = lang
    if not yes:
        extra["state"] = "active"
        dump(report("uninstall", "preview", True, 0, actions=actions, extra=extra))
        return 0
    if dest.exists():
        shutil.rmtree(dest)
    if pkg_path.is_file():
        pkg = json.loads(pkg_path.read_text(encoding="utf-8"))
        bundles = [item for item in ((pkg.get("dsh") or {}).get("profile") or {}).get("bundles") or [] if item != PLUGIN]
        ((pkg.setdefault("dsh", {})).setdefault("profile", {}))["bundles"] = bundles
        pkg_path.write_text(json.dumps(pkg, indent=2) + "\n", encoding="utf-8")
    if patch_path.is_file():
        kept = [line for line in patch_path.read_text(encoding="utf-8").splitlines() if "dsh-keysmith" not in line and line.strip() != "- insert:"]
        patch_path.write_text("\n".join(kept) + ("\n" if kept else ""), encoding="utf-8")
    (root / MANIFEST).unlink(missing_ok=True)
    dump(report("uninstall", "execute", True, 0, actions=actions,
                extra={**extra, "state": "not-installed", "manifest": None}))
    return 0


def recover(root: Path, lang: str | None, yes: bool) -> int:
    mode = "execute" if yes else "preview"
    extra = {"dsh_dir": str(root), "dsh_dir_exists": root.is_dir(), "residue": [],
             "adapter_patch": {"state": "absent", "path": None, "managed": False, "already_patched": None}}
    if os.environ.get("FIXTURE_LANG_ECHO") == "1":
        extra["lang"] = lang
    dump(report("recover", mode, True, 0, extra=extra))
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
    profile = flag(argv, "--profile") or DEFAULT_PROFILE
    yes = "--yes" in argv
    source = flag(argv, "--file")
    if "--status" in argv:
        return status(root, lang, profile)
    if "--uninstall" in argv:
        return uninstall(root, lang, yes, profile)
    if "--recover" in argv:
        return recover(root, lang, yes)
    return deploy(root, lang, name, yes, profile, source)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
