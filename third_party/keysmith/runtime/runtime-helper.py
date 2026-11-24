#!/usr/bin/env python3
"""Filesystem bridge for agent state owned by a selected WSL user.

The Windows app sends one JSON document on stdin. Every path must be an
absolute Linux path; operations use lstat and never follow symlinks while
deciding whether a managed file may be changed.
"""

from __future__ import annotations

import hashlib
import ipaddress
import json
import os
import shutil
import stat
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import Any

MAX_READ = 2 * 1024 * 1024
SECRET_NAMES = {"auth.json", ".credentials.json", "credentials.json", ".netrc", ".env"}
SECRET_KEYS = (
    "key", "token", "secret", "password", "credential", "account", "oauth",
    "session", "cookie", "authorization", "refresh", "access",
)


def fail(message: str) -> None:
    print(json.dumps({"ok": False, "error": message}, ensure_ascii=False))
    raise SystemExit(1)


def absolute(raw: Any) -> Path:
    if not isinstance(raw, str) or not raw.startswith("/") or "\0" in raw:
        raise ValueError("runtime path must be an absolute Linux path")
    return Path(raw)


def sha_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(128 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def node(path: Path, read_text: bool = False) -> dict[str, Any]:
    try:
        meta = path.lstat()
    except FileNotFoundError:
        return {"kind": "absent"}
    except OSError:
        return {"kind": "other"}
    if stat.S_ISLNK(meta.st_mode) or not stat.S_ISREG(meta.st_mode):
        return {"kind": "directory" if stat.S_ISDIR(meta.st_mode) else "other"}
    result: dict[str, Any] = {"kind": "file", "sha256": sha_file(path), "bytes": meta.st_size}
    if read_text and meta.st_size <= MAX_READ:
        try:
            result["text"] = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            pass
    return result


def tree_stats(path: Path) -> tuple[int, int]:
    meta = path.lstat()
    if stat.S_ISLNK(meta.st_mode):
        raise ValueError(f"refusing symbolic link: {path}")
    if stat.S_ISREG(meta.st_mode):
        return 1, meta.st_size
    if not stat.S_ISDIR(meta.st_mode):
        raise ValueError(f"unsupported file type: {path}")
    files = 0
    total = 0
    pending = [path]
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                item = Path(entry.path)
                item_meta = item.lstat()
                if stat.S_ISLNK(item_meta.st_mode):
                    files += 1
                    total += item_meta.st_size
                elif stat.S_ISDIR(item_meta.st_mode):
                    pending.append(item)
                else:
                    files += 1
                    total += item_meta.st_size
    return files, total


def tree_fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    root = path.parent if path.is_file() else path
    pending = [path]
    while pending:
        current = pending.pop()
        meta = current.lstat()
        relative = str(current.relative_to(root)).encode("utf-8", errors="surrogateescape")
        kind = b"l" if stat.S_ISLNK(meta.st_mode) else b"d" if stat.S_ISDIR(meta.st_mode) else b"f"
        digest.update(kind + b"\0" + relative + b"\0")
        digest.update(str(meta.st_size).encode() + b":" + str(meta.st_mtime_ns).encode() + b"\0")
        if stat.S_ISREG(meta.st_mode):
            with current.open("rb") as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(chunk)
        elif stat.S_ISLNK(meta.st_mode):
            digest.update(os.readlink(current).encode("utf-8", errors="surrogateescape"))
        elif stat.S_ISDIR(meta.st_mode):
            pending.extend(sorted((Path(entry.path) for entry in os.scandir(current)), reverse=True))
    return digest.hexdigest()


def holds_project_or_secret(path: Path) -> bool:
    if path.name in SECRET_NAMES:
        return True
    if not path.is_dir():
        return False
    pending = [path]
    while pending:
        directory = pending.pop()
        try:
            entries = list(os.scandir(directory))
        except OSError:
            return True
        for entry in entries:
            if entry.name == ".git" or entry.name in SECRET_NAMES:
                return True
            try:
                if entry.is_symlink():
                    continue
                if entry.is_dir(follow_symlinks=False):
                    pending.append(Path(entry.path))
            except OSError:
                return True
    return False


def extra_info(request: dict[str, Any]) -> dict[str, Any] | None:
    path = absolute(request.get("path"))
    name = request.get("name")
    kind = request.get("kind")
    if not isinstance(name, str) or not name or kind not in {"saved", "saved-without-login", "erased"}:
        raise ValueError("invalid cleanup entry")
    try:
        meta = path.lstat()
    except FileNotFoundError:
        return None
    if stat.S_ISLNK(meta.st_mode) or not (stat.S_ISREG(meta.st_mode) or stat.S_ISDIR(meta.st_mode)):
        return None
    if kind == "saved" and holds_project_or_secret(path):
        return None
    if kind == "saved-without-login" and not stat.S_ISREG(meta.st_mode):
        return None
    files, size = tree_stats(path)
    if files == 0:
        return None
    return {
        "name": name,
        "path": str(path),
        "files": files,
        "bytes": size,
        "fingerprint": tree_fingerprint(path),
        "kind": kind,
    }


def expanded_entries(payload: dict[str, Any]) -> list[dict[str, Any]]:
    requests = list(payload.get("entries", []))
    for prefix in payload.get("prefixes", []):
        directory = absolute(prefix.get("directory"))
        logical = prefix.get("logicalFolder")
        starts = prefix.get("prefix")
        kind = prefix.get("kind")
        owned = prefix.get("ownedPrefixes", [])
        if not isinstance(logical, str) or not isinstance(starts, str) or kind not in {"saved", "erased"}:
            raise ValueError("invalid cleanup prefix")
        try:
            names = sorted(item.name for item in os.scandir(directory))
        except (FileNotFoundError, NotADirectoryError, PermissionError):
            names = []
        for filename in names:
            if not filename.startswith(starts) or len(filename) == len(starts):
                continue
            if any(filename.startswith(value) for value in owned if isinstance(value, str)):
                continue
            name = f"{logical}/{filename}" if logical else filename
            requests.append({"name": name, "path": str(directory / filename), "kind": kind})
    unique: dict[str, dict[str, Any]] = {}
    for request in requests:
        name = request.get("name")
        if isinstance(name, str) and name not in unique:
            unique[name] = request
    return list(unique.values())


def process_running(tool: str) -> bool:
    expected = {
        "codex": ("codex",),
        "claude": ("claude",),
        "grok": ("grok",),
        "zcode": ("zcode",),
        "pi": ("pi",),
        "dsh": ("dsh",),
    }.get(tool, ())
    if not expected:
        return False
    try:
        pids = os.listdir("/proc")
    except OSError:
        return False
    for pid in pids:
        if not pid.isdigit():
            continue
        try:
            name = (Path("/proc") / pid / "comm").read_text(encoding="utf-8").strip().lower()
        except OSError:
            continue
        if any(name == item or (item == "zcode" and name.startswith("zcode")) for item in expected):
            return True
    return False


def cleanup_scan(payload: dict[str, Any]) -> dict[str, Any]:
    extras = []
    for request in expanded_entries(payload):
        info = extra_info(request)
        if info is not None:
            extras.append(info)
    memory = None
    if payload.get("memoryPath"):
        path = absolute(payload["memoryPath"])
        current = node(path)
        if current.get("kind") == "file":
            size = int(current["bytes"])
            if size > int(payload.get("maxMemoryBytes", 8 * 1024 * 1024)):
                raise ValueError("the memory file is too large to snapshot")
            data = path.read_bytes()
            memory = {
                "path": str(path),
                "bytes": size,
                "lines": data.count(b"\n") + (1 if data and not data.endswith(b"\n") else 0),
                "sha256": current["sha256"],
                "mode": path.stat().st_mode & 0o7777,
            }
    memories = None
    if payload.get("memoriesPath"):
        path = absolute(payload["memoriesPath"])
        try:
            meta = path.lstat()
            if stat.S_ISDIR(meta.st_mode) and not stat.S_ISLNK(meta.st_mode):
                files, size = tree_stats(path)
                if files:
                    memories = {
                        "path": str(path),
                        "files": files,
                        "bytes": size,
                        "fingerprint": tree_fingerprint(path),
                    }
            elif stat.S_ISLNK(meta.st_mode) or not stat.S_ISDIR(meta.st_mode):
                raise ValueError("the memories folder is a link or not a folder")
        except FileNotFoundError:
            pass
    return {
        "ok": True,
        "memory": memory,
        "memories": memories,
        "extras": extras,
        "agentRunning": process_running(str(payload.get("tool", ""))),
    }


def secret_key(name: str) -> bool:
    lowered = name.lower().replace("-", "_")
    return any(part in lowered for part in SECRET_KEYS)


def strip_json(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: strip_json(item) for key, item in value.items() if not secret_key(str(key))}
    if isinstance(value, list):
        return [strip_json(item) for item in value]
    return value


def sanitized_bytes(path: Path) -> bytes:
    data = path.read_bytes()
    if path.suffix.lower() == ".json" or path.name.lower().endswith(".json"):
        value = json.loads(data.decode("utf-8"))
        return (json.dumps(strip_json(value), ensure_ascii=False, indent=2) + "\n").encode()
    text = data.decode("utf-8")
    kept = []
    for line in text.splitlines(keepends=True):
        key = line.split("=", 1)[0].strip().strip('"').strip("'") if "=" in line else ""
        if not key or not secret_key(key):
            kept.append(line)
    return "".join(kept).encode()


def copy_piece(source: Path, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    meta = source.lstat()
    if stat.S_ISLNK(meta.st_mode):
        raise ValueError(f"refusing symbolic link: {source}")
    if stat.S_ISDIR(meta.st_mode):
        shutil.copytree(source, target, symlinks=True)
    elif stat.S_ISREG(meta.st_mode):
        shutil.copy2(source, target)
    else:
        raise ValueError(f"unsupported file type: {source}")


def remove_piece(path: Path) -> None:
    meta = path.lstat()
    if stat.S_ISLNK(meta.st_mode):
        raise ValueError(f"refusing symbolic link: {path}")
    if stat.S_ISDIR(meta.st_mode):
        shutil.rmtree(path)
    elif stat.S_ISREG(meta.st_mode):
        path.unlink()
    else:
        raise ValueError(f"unsupported file type: {path}")


def verify_meta(path: Path, expected: dict[str, Any]) -> None:
    files, size = tree_stats(path)
    if files != expected.get("files") or size != expected.get("bytes"):
        raise ValueError(f"cleanup target changed since preview: {path}")
    fingerprint = expected.get("fingerprint")
    if fingerprint is not None and tree_fingerprint(path) != fingerprint:
        raise ValueError(f"cleanup target changed since preview: {path}")


def snapshot_prepare(payload: dict[str, Any]) -> dict[str, Any]:
    staging = absolute(payload.get("staging"))
    staging.mkdir(parents=True, exist_ok=False)
    memory = payload.get("memory")
    if isinstance(memory, dict):
        source = absolute(memory.get("path"))
        current = node(source)
        if current.get("sha256") != memory.get("sha256"):
            raise ValueError("memory changed since preview")
        shutil.copy2(source, staging / "memory.bin")
        if sha_file(staging / "memory.bin") != memory.get("sha256"):
            raise ValueError("snapshot memory copy could not be verified")
    for saved in payload.get("extras", []):
        if saved.get("kind") != "saved-without-login":
            continue
        source = absolute(saved.get("path"))
        verify_meta(source, saved)
        target = staging / "extras" / str(saved.get("name"))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(sanitized_bytes(source))
    return {"ok": True}


def cleanup_apply(payload: dict[str, Any]) -> dict[str, Any]:
    snapshot = absolute(payload.get("snapshot")) if payload.get("snapshot") else None
    updated = []
    cleared = 0
    erased = 0
    # Freeze every target before changing the first one. A failed precondition leaves the
    # entire cleanup untouched instead of producing a partial cleanup from a stale preview.
    for saved in payload.get("extras", []):
        source = absolute(saved.get("path"))
        try:
            current = extra_info(saved)
        except OSError as error:
            raise ValueError(str(error)) from error
        if current is None:
            raise ValueError(f"cleanup target changed since preview: {source}")
        verify_meta(source, saved)
    memory = payload.get("memory")
    if isinstance(memory, dict):
        source = absolute(memory.get("path"))
        if node(source).get("sha256") != memory.get("sha256"):
            raise ValueError("memory changed since preview")
    memories = payload.get("memories")
    if isinstance(memories, dict):
        verify_meta(absolute(memories.get("path")), memories)
    for saved in payload.get("extras", []):
        source = absolute(saved.get("path"))
        try:
            current = extra_info(saved)
        except OSError as error:
            raise ValueError(str(error)) from error
        if current is None:
            continue
        kind = saved.get("kind")
        if kind == "erased":
            remove_piece(source)
            erased += 1
            continue
        if snapshot is None:
            raise ValueError("snapshot is required for saved cleanup entries")
        target = snapshot / "extras" / str(saved.get("name"))
        if kind == "saved-without-login":
            if not target.is_file():
                raise ValueError(f"sanitized snapshot copy is missing: {saved.get('name')}")
        else:
            copy_piece(source, target)
            verify_meta(target, current)
        remove_piece(source)
        updated.append(current)
        cleared += 1
    memory_cleared = False
    if isinstance(memory, dict) and memory.get("bytes", 0) > 0:
        path = absolute(memory.get("path"))
        if node(path).get("sha256") != memory.get("sha256"):
            raise ValueError("memory changed since preview")
        with path.open("r+b") as stream:
            stream.truncate(0)
            stream.flush()
            os.fsync(stream.fileno())
        memory_cleared = True
    memories_cleared = False
    if isinstance(memories, dict):
        if snapshot is None:
            raise ValueError("snapshot is required for the memories folder")
        source = absolute(memories.get("path"))
        verify_meta(source, memories)
        target = snapshot / "memories"
        copy_piece(source, target)
        verify_meta(target, memories)
        mode = source.stat().st_mode & 0o7777
        remove_piece(source)
        source.mkdir(mode=mode)
        memories_cleared = True
    return {
        "ok": True,
        "extras": updated,
        "extrasCleared": cleared,
        "erased": erased,
        "memoryCleared": memory_cleared,
        "memoriesCleared": memories_cleared,
    }


def restore_piece(source: Path, target: Path, allow_existing: bool) -> bool:
    if target.exists() or target.is_symlink():
        if allow_existing:
            return False
        meta = target.lstat()
        if not stat.S_ISDIR(meta.st_mode) or any(target.iterdir()):
            raise ValueError(f"restore target is not empty: {target}")
        target.rmdir()
    staging = target.parent / f".{target.name}.keysmith-restore-{uuid.uuid4().hex}"
    copy_piece(source, staging)
    target.parent.mkdir(parents=True, exist_ok=True)
    os.replace(staging, target)
    return True


def rollback_apply(payload: dict[str, Any]) -> dict[str, Any]:
    snapshot = absolute(payload.get("snapshot"))
    restored = 0
    for saved in payload.get("extras", []):
        source = snapshot / "extras" / str(saved.get("name"))
        if not source.exists():
            raise ValueError(f"snapshot extra is missing: {saved.get('name')}")
        verify_meta(source, saved) if saved.get("kind") == "saved" else None
        target = absolute(saved.get("path"))
        if restore_piece(source, target, saved.get("kind") == "saved-without-login"):
            restored += 1
    memory_restored = False
    memory = payload.get("memory")
    if isinstance(memory, dict):
        source = snapshot / "memory.bin"
        if sha_file(source) != memory.get("sha256"):
            raise ValueError("snapshot memory copy does not match")
        target = absolute(memory.get("path"))
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary = target.parent / f".{target.name}.keysmith-restore-{uuid.uuid4().hex}"
        shutil.copy2(source, temporary)
        if memory.get("mode") is not None:
            temporary.chmod(int(memory["mode"]))
        os.replace(temporary, target)
        memory_restored = True
    memories_restored = False
    memories = payload.get("memories")
    if isinstance(memories, dict):
        source = snapshot / "memories"
        verify_meta(source, memories)
        target = absolute(memories.get("path"))
        restore_piece(source, target, False)
        memories_restored = True
    return {
        "ok": True,
        "extrasRestored": restored,
        "memoryRestored": memory_restored,
        "memoriesRestored": memories_restored,
    }


def blocked(path: Path, root: Path) -> bool:
    try:
        relative = path.relative_to(root)
    except ValueError:
        return True
    current = root
    parts = relative.parts
    for index, part in enumerate(parts):
        if part in {"", ".", ".."}:
            return True
        current = current / part
        try:
            meta = current.lstat()
        except FileNotFoundError:
            return False
        except OSError:
            return True
        last = index + 1 == len(parts)
        if stat.S_ISLNK(meta.st_mode) or (not last and not stat.S_ISDIR(meta.st_mode)):
            return True
    return False


def missing_dirs(path: Path) -> list[str]:
    missing: list[str] = []
    current = path.parent
    while current != current.parent:
        try:
            current.lstat()
            break
        except FileNotFoundError:
            missing.append(str(current))
            current = current.parent
        except OSError:
            break
    missing.reverse()
    return missing


def inspect(payload: dict[str, Any]) -> dict[str, Any]:
    paths: dict[str, Any] = {}
    for request in payload.get("paths", []):
        path = absolute(request.get("path"))
        root_raw = request.get("root")
        result = node(path, bool(request.get("readText")))
        result["missingDirs"] = missing_dirs(path)
        result["blocked"] = blocked(path, absolute(root_raw)) if root_raw else False
        paths[str(path)] = result
    directories = {
        str(path): node(path)["kind"]
        for path in (absolute(raw) for raw in payload.get("directories", []))
    }
    return {"ok": True, "paths": paths, "directories": directories}


def create_directories(raw_dirs: list[Any]) -> None:
    for raw in raw_dirs:
        path = absolute(raw)
        try:
            meta = path.lstat()
            if not stat.S_ISDIR(meta.st_mode) or stat.S_ISLNK(meta.st_mode):
                continue
        except FileNotFoundError:
            try:
                path.mkdir()
            except OSError:
                continue


def atomic_write(path: Path, content: bytes, create: bool) -> None:
    temporary = path.parent / f".keysmith-switch-{uuid.uuid4().hex}"
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        if create:
            os.link(temporary, path)
        else:
            os.replace(temporary, path)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def apply(payload: dict[str, Any]) -> dict[str, Any]:
    create_directories(payload.get("directories", []))
    failed: list[str] = []
    for operation in payload.get("operations", []):
        path = absolute(operation.get("path"))
        root = absolute(operation["root"]) if operation.get("root") else None
        before = operation.get("before")
        after = operation.get("after")
        current = node(path)
        current_sha = current.get("sha256") if current.get("kind") == "file" else None
        if current_sha == after and current.get("kind") != "other":
            continue
        matches = current.get("kind") == "absent" if before is None else current_sha == before
        if not matches or (after is not None and root is not None and blocked(path, root)):
            failed.append(str(path))
            continue
        try:
            if after is None:
                path.unlink()
            else:
                content = operation.get("content")
                if not isinstance(content, str) or hashlib.sha256(content.encode()).hexdigest() != after:
                    raise ValueError("content fingerprint mismatch")
                atomic_write(path, content.encode(), before is None)
        except (OSError, ValueError):
            failed.append(str(path))
    for raw in sorted(payload.get("removeDirectories", []), key=lambda item: len(str(item)), reverse=True):
        try:
            absolute(raw).rmdir()
        except OSError:
            pass
    return {"ok": True, "failed": failed}


def relay_address() -> dict[str, Any]:
    candidates: list[str] = []
    try:
        route = subprocess.run(
            ["ip", "-4", "route", "show", "default"],
            capture_output=True,
            check=False,
            text=True,
            timeout=3,
        )
        for line in route.stdout.splitlines():
            fields = line.split()
            if "via" in fields:
                index = fields.index("via")
                if index + 1 < len(fields):
                    candidates.append(fields[index + 1])
    except (OSError, subprocess.SubprocessError):
        pass
    try:
        for line in Path("/etc/resolv.conf").read_text(encoding="utf-8").splitlines():
            fields = line.split()
            if len(fields) >= 2 and fields[0] == "nameserver":
                candidates.append(fields[1])
    except (OSError, UnicodeDecodeError):
        pass
    for raw in candidates:
        try:
            address = ipaddress.ip_address(raw)
        except ValueError:
            continue
        if isinstance(address, ipaddress.IPv4Address) and not (
            address.is_unspecified or address.is_multicast
        ):
            return {"ok": True, "address": str(address)}
    raise ValueError("cannot resolve the Windows host address from this WSL runtime")


def safe_leaf(raw: Any) -> str:
    if not isinstance(raw, str) or not raw or raw in {".", ".."} or "/" in raw or "\\" in raw:
        raise ValueError("invalid rewrite file name")
    return raw


def rewrite_read(payload: dict[str, Any]) -> dict[str, Any]:
    directory = absolute(payload.get("directory"))
    try:
        meta = directory.lstat()
        directory_exists = stat.S_ISDIR(meta.st_mode) and not stat.S_ISLNK(meta.st_mode)
    except FileNotFoundError:
        directory_exists = False
    if not directory_exists:
        return {"ok": True, "directoryExists": False, "files": {}}
    files: dict[str, Any] = {}
    total = 0
    for raw in payload.get("files", []):
        name = safe_leaf(raw)
        path = directory / name
        try:
            meta = path.lstat()
        except FileNotFoundError:
            files[name] = {"kind": "absent"}
            continue
        if stat.S_ISLNK(meta.st_mode) or not stat.S_ISREG(meta.st_mode):
            files[name] = {"kind": "other"}
            continue
        if meta.st_size > 2 * 1024 * 1024:
            raise ValueError(f"rewrite file is too large: {path}")
        data = path.read_bytes()
        total += len(data)
        if total > 4 * 1024 * 1024:
            raise ValueError("rewrite configuration is too large")
        files[name] = {
            "kind": "file",
            "content": data.decode("utf-8"),
            "sha256": hashlib.sha256(data).hexdigest(),
            "mode": meta.st_mode & 0o7777,
        }
    return {"ok": True, "directoryExists": True, "files": files}


def rewrite_write(payload: dict[str, Any]) -> dict[str, Any]:
    directory = absolute(payload.get("directory"))
    meta = directory.lstat()
    if stat.S_ISLNK(meta.st_mode) or not stat.S_ISDIR(meta.st_mode):
        raise ValueError("rewrite configuration directory is unavailable")
    changed = 0
    for operation in payload.get("operations", []):
        name = safe_leaf(operation.get("name"))
        path = directory / name
        before = operation.get("beforeSha256")
        content = operation.get("content")
        if before is not None and not isinstance(before, str):
            raise ValueError("invalid rewrite precondition")
        if not isinstance(content, str):
            raise ValueError("invalid rewrite content")
        current = node(path)
        current_sha = current.get("sha256") if current.get("kind") == "file" else None
        expected_absent = before is None
        if (expected_absent and current.get("kind") != "absent") or (not expected_absent and current_sha != before):
            raise ValueError(f"rewrite config changed since it was read: {path}")
        data = content.encode("utf-8")
        if len(data) > 2 * 1024 * 1024:
            raise ValueError("rewrite configuration is too large")
        if current_sha == hashlib.sha256(data).hexdigest():
            continue
        if current.get("kind") == "file":
            stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
            backup = directory / f"{name}.keysmith-relay-{stamp}.bak"
            suffix = 0
            while backup.exists():
                suffix += 1
                backup = directory / f"{name}.keysmith-relay-{stamp}-{suffix}.bak"
            shutil.copy2(path, backup)
        atomic_write(path, data, current.get("kind") == "absent")
        if operation.get("mode") is not None:
            path.chmod(int(operation["mode"]))
        changed += 1
    return {"ok": True, "changed": changed}


def command_output(argv: list[str], timeout: int) -> str | None:
    try:
        result = subprocess.run(
            argv,
            capture_output=True,
            check=False,
            text=True,
            timeout=timeout,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    line = result.stdout.strip().splitlines()
    return line[0].strip() if line and line[0].strip() else None


def official_inspect(payload: dict[str, Any]) -> dict[str, Any]:
    executable = payload.get("executable")
    package = payload.get("package")
    if not isinstance(executable, str) or not executable or any(ch in executable for ch in "/\\\0"):
        raise ValueError("invalid official executable")
    if package is not None and (not isinstance(package, str) or not package):
        raise ValueError("invalid official package")
    executable_path = shutil.which(executable)
    current_version = command_output([executable_path, "--version"], 5) if executable_path else None
    npm_path = shutil.which("npm")
    latest_version = command_output([npm_path, "view", package, "version"], 10) if npm_path and package else None
    npm_root = command_output([npm_path, "root", "-g"], 10) if npm_path else None
    return {
        "ok": True,
        "npmAvailable": npm_path is not None,
        "executablePath": executable_path,
        "currentVersion": current_version,
        "latestVersion": latest_version,
        "npmRoot": npm_root,
    }


def main() -> None:
    try:
        payload = json.load(sys.stdin)
        command = payload.get("command")
        if command == "inspect":
            result = inspect(payload)
        elif command == "apply":
            result = apply(payload)
        elif command == "environment":
            result = {
                "ok": True,
                "home": str(Path.home()),
                "user": os.environ.get("USER") or os.environ.get("LOGNAME") or "",
            }
        elif command == "cleanupScan":
            result = cleanup_scan(payload)
        elif command == "snapshotPrepare":
            result = snapshot_prepare(payload)
        elif command == "cleanupApply":
            result = cleanup_apply(payload)
        elif command == "rollbackApply":
            result = rollback_apply(payload)
        elif command == "relayAddress":
            result = relay_address()
        elif command == "rewriteRead":
            result = rewrite_read(payload)
        elif command == "rewriteWrite":
            result = rewrite_write(payload)
        elif command == "officialInspect":
            result = official_inspect(payload)
        else:
            raise ValueError("unknown runtime helper command")
        print(json.dumps(result, ensure_ascii=False))
    except (OSError, ValueError, TypeError, json.JSONDecodeError) as error:
        fail(str(error))


if __name__ == "__main__":
    main()
