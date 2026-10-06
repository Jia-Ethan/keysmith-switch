#!/usr/bin/env python3
"""Freeze the four Keysmith CLIs into platform-native sidecars, and build the input
rewrite relay (Rust) next to them.

Users of the packaged app do not need Python. Dev `tauri dev` and cargo tests
still fall back to python + vendored scripts when these binaries are absent.
"""

from __future__ import annotations

import os
import platform
import shutil
import stat
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VENDOR = ROOT / "third_party" / "keysmith"
OUT = ROOT / "src-tauri" / "binaries"
WORK = ROOT / "sidecar-build"


def target_triple() -> str:
    if env := os.environ.get("TAURI_ENV_TARGET_TRIPLE") or os.environ.get("TARGET"):
        return env
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Darwin":
        arch = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
        return f"{arch}-apple-darwin"
    if system == "Windows":
        arch = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
        return f"{arch}-pc-windows-msvc"
    arch = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
    return f"{arch}-unknown-linux-gnu"


def venv_python() -> Path:
    venv = WORK / "venv"
    if sys.platform == "win32":
        python = venv / "Scripts" / "python.exe"
        pyinstaller = venv / "Scripts" / "pyinstaller.exe"
    else:
        python = venv / "bin" / "python"
        pyinstaller = venv / "bin" / "pyinstaller"
    if not pyinstaller.exists():
        WORK.mkdir(parents=True, exist_ok=True)
        subprocess.check_call([sys.executable, "-m", "venv", str(venv)])
        subprocess.check_call(
            [str(python), "-m", "pip", "install", "--upgrade", "pip", "pyinstaller"],
        )
    return python


SPECS = [
    {
        "name": "keysmith-claude",
        "script": VENDOR / "claude" / "claude-instruct.py",
        "datas": [
            VENDOR / "claude" / "examples" / "claude-project-rules.md",
            VENDOR / "claude" / "examples" / "claude-append-prompt.md",
        ],
        "hidden": [],
    },
    {
        "name": "keysmith-codex",
        "script": VENDOR / "codex" / "codex-instruct.py",
        "datas": [
            VENDOR / "codex" / "examples" / "gpt-unrestricted.md",
            VENDOR / "codex" / "examples" / "gpt-contract.md",
            VENDOR / "codex" / "examples" / "gpt-overlay.md",
        ],
        "hidden": [],
    },
    {
        "name": "keysmith-grok",
        "script": VENDOR / "grok" / "grok-keysmith.py",
        "datas": [VENDOR / "grok" / "examples" / "grok-unrestricted.md"],
        "hidden": ["grok_keysmith_runner", "grok_keysmith_breaktest"],
        "paths": [VENDOR / "grok"],
    },
    {
        "name": "keysmith-zcode",
        "script": VENDOR / "zcode" / "zcode-keysmith.py",
        "datas": [
            VENDOR / "zcode" / "examples" / "system-role.md",
            # The adapter copies its own source file while installing (`Path(__file__)
            # .read_text()`); a frozen build has no such file unless it is bundled.
            (VENDOR / "zcode" / "zcode-keysmith.py", "."),
        ],
        "must_contain": ["zcode-keysmith.py"],
        "hidden": [],
    },
]


def add_data_arg(src: Path, dest: str = "examples") -> str:
    sep = ";" if sys.platform == "win32" else ":"
    return f"{src}{sep}{dest}"


def assert_bundled(python: Path, binary: Path, required: list) -> None:
    """Fail the build when a file the adapter reads at run time is not inside the binary."""
    if not required:
        return
    listing = subprocess.run(
        [
            str(python),
            "-c",
            "import sys; from PyInstaller.utils.cliutils.archive_viewer import run; "
            "sys.argv = ['x', '-l', '-b', sys.argv[1]]; run()",
            str(binary),
        ],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    entries = {line.strip() for line in listing}
    missing = [name for name in required if name not in entries]
    if missing:
        raise SystemExit(f"{binary.name} does not bundle {missing}; the adapter reads these when it runs")


def build_one(python: Path, spec: dict, triple: str) -> Path:
    name = spec["name"]
    suffix = ".exe" if sys.platform == "win32" else ""
    dest = OUT / f"{name}-{triple}{suffix}"
    if dest.is_file() and dest.stat().st_size > 1024 * 1024 and not os.environ.get("KEYSMITH_SWITCH_REBUILD_SIDECARS"):
        print(f"reuse {dest}")
        return dest
    dist = WORK / "dist"
    workpath = WORK / "work" / name
    workpath.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(python),
        "-m",
        "PyInstaller",
        "--noconfirm",
        "--clean",
        "--onefile",
        "--name",
        name,
        "--distpath",
        str(dist),
        "--workpath",
        str(workpath),
        "--specpath",
        str(workpath),
    ]
    for data in spec.get("datas") or []:
        src, dest = data if isinstance(data, tuple) else (data, "examples")
        if Path(src).is_file():
            cmd.extend(["--add-data", add_data_arg(Path(src), dest)])
    for hidden in spec.get("hidden") or []:
        cmd.extend(["--hidden-import", hidden])
    for extra in spec.get("paths") or []:
        cmd.extend(["--paths", str(extra)])
    cmd.append(str(spec["script"]))
    subprocess.check_call(cmd, cwd=str(ROOT))
    produced = dist / (name + (".exe" if sys.platform == "win32" else ""))
    if not produced.is_file():
        raise SystemExit(f"PyInstaller did not produce {produced}")
    assert_bundled(python, produced, spec.get("must_contain") or [])
    OUT.mkdir(parents=True, exist_ok=True)
    suffix = ".exe" if sys.platform == "win32" else ""
    dest = OUT / f"{name}-{triple}{suffix}"
    shutil.copy2(produced, dest)
    if sys.platform != "win32":
        dest.chmod(dest.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
        alias = OUT / name
        shutil.copy2(dest, alias)
        alias.chmod(dest.stat().st_mode)
    print(f"wrote {dest}")
    return dest


def build_relay(triple: str) -> Path:
    """Build keysmith-relay for the same target and place it where Tauri expects sidecars."""
    manifest = ROOT / "src-tauri" / "Cargo.toml"
    cmd = ["cargo", "build", "--release", "-p", "keysmith-relay", "--manifest-path", str(manifest)]
    host = subprocess.run(["rustc", "-vV"], capture_output=True, text=True).stdout
    cross = f"host: {triple}" not in host
    if cross:
        cmd += ["--target", triple]
    subprocess.check_call(cmd, cwd=str(ROOT))
    suffix = ".exe" if sys.platform == "win32" else ""
    target = ROOT / "src-tauri" / "target"
    produced = (target / triple if cross else target) / "release" / f"keysmith-relay{suffix}"
    if not produced.is_file():
        raise SystemExit(f"cargo did not produce {produced}")
    OUT.mkdir(parents=True, exist_ok=True)
    dest = OUT / f"keysmith-relay-{triple}{suffix}"
    shutil.copy2(produced, dest)
    print(f"wrote {dest}")
    return dest


def main() -> None:
    if not VENDOR.is_dir():
        raise SystemExit(f"missing vendored CLIs at {VENDOR}")
    python = venv_python()
    triple = target_triple()
    built = []
    for spec in SPECS:
        built.append(build_one(python, spec, triple))
    built.append(build_relay(triple))
    missing = [path.name for path in built if not path.is_file()]
    if missing:
        raise SystemExit(f"sidecar missing: {missing}")
    print("sidecars ok:", ", ".join(str(path.name) for path in built))


if __name__ == "__main__":
    main()
