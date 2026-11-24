#!/usr/bin/env python3
"""Destructive helper checks confined to a newly-created /tmp/keysmith-* directory."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def call(helper: Path, payload: dict[str, object], *, succeeds: bool = True) -> dict[str, object]:
    result = subprocess.run(
        [sys.executable, str(helper)],
        input=json.dumps(payload),
        capture_output=True,
        check=False,
        text=True,
        timeout=30,
    )
    response = json.loads(result.stdout)
    if succeeds:
        assert result.returncode == 0 and response.get("ok") is True, (result, response)
    else:
        assert result.returncode != 0 and response.get("ok") is False, (result, response)
    return response


def main() -> None:
    if len(sys.argv) not in {2, 3}:
        raise SystemExit("usage: test-wsl-runtime-helper.py RUNTIME_HELPER [CODEX_ADAPTER]")
    helper = Path(sys.argv[1]).resolve()
    assert helper.is_file()
    adapter = Path(sys.argv[2]).resolve() if len(sys.argv) == 3 else None
    assert adapter is None or adapter.is_file()
    root = Path(tempfile.mkdtemp(prefix="keysmith-wsl-helper-", dir="/tmp"))
    try:
        relay = call(helper, {"command": "relayAddress"})
        assert relay.get("address")

        rewrite = root / "custom-codex"
        rewrite.mkdir()
        config = rewrite / "config.toml"
        config.write_text('model_provider = "one"\n', encoding="utf-8")
        read = call(
            helper,
            {"command": "rewriteRead", "directory": str(rewrite), "files": ["config.toml"]},
        )
        before = read["files"]["config.toml"]["sha256"]
        call(
            helper,
            {
                "command": "rewriteWrite",
                "directory": str(rewrite),
                "operations": [
                    {
                        "name": "config.toml",
                        "beforeSha256": before,
                        "content": 'model_provider = "two"\n',
                        "mode": 0o600,
                    }
                ],
            },
        )
        assert config.read_text(encoding="utf-8") == 'model_provider = "two"\n'
        config.write_text('model_provider = "changed"\n', encoding="utf-8")
        refused = call(
            helper,
            {
                "command": "rewriteWrite",
                "directory": str(rewrite),
                "operations": [
                    {"name": "config.toml", "beforeSha256": before, "content": "stale\n"}
                ],
            },
            succeeds=False,
        )
        assert "changed since" in str(refused.get("error"))

        agent = root / "agent"
        rules = agent / "rules"
        history = agent / "history"
        rules.mkdir(parents=True)
        history.mkdir()
        (rules / "review.md").write_text("review carefully\n", encoding="utf-8")
        (history / "session.jsonl").write_text('{"prompt":"private"}\n', encoding="utf-8")
        settings = agent / "settings.json"
        settings.write_text(
            json.dumps({"theme": "dark", "apiToken": "secret", "nested": {"account": "private"}}),
            encoding="utf-8",
        )
        memory = agent / "AGENTS.md"
        memory.write_text("local instructions\n", encoding="utf-8")
        scan_payload = {
            "command": "cleanupScan",
            "tool": "codex",
            "memoryPath": str(memory),
            "entries": [
                {"name": "rules", "path": str(rules), "kind": "saved"},
                {"name": "settings.json", "path": str(settings), "kind": "saved-without-login"},
                {"name": "history", "path": str(history), "kind": "erased"},
            ],
        }
        scan = call(helper, scan_payload)
        assert len(scan["extras"]) == 3

        stale = agent / "stale"
        stale.mkdir()
        stale_file = stale / "value.txt"
        stale_file.write_text("before", encoding="utf-8")
        stale_scan = call(
            helper,
            {
                "command": "cleanupScan",
                "tool": "codex",
                "entries": [{"name": "stale", "path": str(stale), "kind": "erased"}],
            },
        )
        stale_file.write_text("after!", encoding="utf-8")
        rejected = call(
            helper,
            {"command": "cleanupApply", "extras": stale_scan["extras"]},
            succeeds=False,
        )
        assert "changed since preview" in str(rejected.get("error"))
        assert stale.exists()

        snapshot = root / "snapshot"
        call(
            helper,
            {
                "command": "snapshotPrepare",
                "staging": str(snapshot),
                "memory": scan["memory"],
                "extras": scan["extras"],
            },
        )
        done = call(
            helper,
            {
                "command": "cleanupApply",
                "snapshot": str(snapshot),
                "memory": scan["memory"],
                "extras": scan["extras"],
            },
        )
        assert done["memoryCleared"] is True and memory.read_text(encoding="utf-8") == ""
        assert not rules.exists() and not settings.exists() and not history.exists()
        sanitized = json.loads((snapshot / "extras" / "settings.json").read_text(encoding="utf-8"))
        assert sanitized == {"theme": "dark", "nested": {}}

        restored = call(
            helper,
            {
                "command": "rollbackApply",
                "snapshot": str(snapshot),
                "memory": scan["memory"],
                "extras": [item for item in scan["extras"] if item["kind"] != "erased"],
            },
        )
        assert restored["memoryRestored"] is True
        assert memory.read_text(encoding="utf-8") == "local instructions\n"
        assert (rules / "review.md").read_text(encoding="utf-8") == "review carefully\n"
        assert json.loads(settings.read_text(encoding="utf-8")) == sanitized

        official = call(
            helper,
            {
                "command": "officialInspect",
                "executable": "codex",
                "package": "@openai/codex",
            },
        )
        assert "npmAvailable" in official and "executablePath" in official

        if adapter is not None:
            codex_dir = root / "adapter-codex"
            codex_dir.mkdir()
            (codex_dir / "config.toml").write_text("", encoding="utf-8")
            prompt = root / "smoke.md"
            prompt.write_text("Use the isolated test prompt.\n", encoding="utf-8")

            def adapter_run(*args: str) -> str:
                result = subprocess.run(
                    [sys.executable, str(adapter), "--codex-dir", str(codex_dir), *args],
                    capture_output=True,
                    check=False,
                    text=True,
                    timeout=30,
                )
                assert result.returncode == 0, result.stderr
                return result.stdout

            adapter_run("--file", str(prompt), "--name", "wsl-smoke", "--dry-run")
            assert not list(codex_dir.rglob("wsl-smoke.md"))
            adapter_run("--file", str(prompt), "--name", "wsl-smoke", "--yes")
            assert list(codex_dir.rglob("wsl-smoke.md"))
            adapter_run("--status")
            adapter_run("--uninstall", "--yes")
            assert not list(codex_dir.rglob("wsl-smoke.md"))
        print(json.dumps({"ok": True, "root": str(root), "relayAddress": relay["address"]}))
    finally:
        resolved = root.resolve()
        if resolved.parent == Path("/tmp") and resolved.name.startswith("keysmith-wsl-helper-"):
            shutil.rmtree(resolved)


if __name__ == "__main__":
    main()
