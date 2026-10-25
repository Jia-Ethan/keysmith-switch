# Vendored Keysmith CLI pins

These copies are taken from the product repositories at the sidecar each desktop
ships. Do not edit them to change upstream behaviour. They ship atomically with
Keysmith Switch.

| Tool | Script | Source | Version |
| --- | --- | --- | --- |
| claude | claude/claude-instruct.py | `7dbfa253` (`v7.2`) | v7.2 |
| codex | codex/codex-instruct.py | `33cf4049` (`v0.6.0`) | v0.6.0 |
| grok | grok/grok-keysmith.py | `d87def09` (`v0.8.0`) | v0.8.0 |
| zcode | zcode/zcode-keysmith.py | `3c3bea8f994d3670c2cd571e9bd87449fc868788` (merged #44; v0.3.4 tag pending) | 0.3.4 |
| pi | pi/pi-keysmith.py | `37f69cac` (`v0.2.0`) | v0.2.0 |
| dsh | dsh/dsh-keysmith.py | `c15f990` (merged #6; v0.4.0 tag pending) | v0.4.0 |

ZCode 0.3.4 recognizes `supportsStorageStartup` in `resources/app.asar` on Windows,
keeps the official agent-server, and reversibly patches `resources/glm/zcode.cjs`.
The uninstaller restores the original runtime from its recorded backup.

Advanced Tools also vendor `grok_keysmith_runner.py` and `grok_keysmith_breaktest.py`
from the same Grok commit. Zcode 0.3.4 emits `zcode-keysmith/v1` when `--json` is
passed. doctor and install stay on text. plan-deactivate and deactivate pass
`--json` on uninstall; the parser still accepts the text lines.

pi 0.2.0 (`Jia-Ethan/pi-keysmith`) emits `pi-keysmith.envelope.v1` with `--json`; its contract
is `docs/json-contract.md` at the same tag. `pi/examples/system-prompt.md` is the CLI's
bundled default prompt from that tag.

dsh 0.4.0 (`Jia-Ethan/dsh-keysmith`, merged [#6](https://github.com/Jia-Ethan/dsh-keysmith/pull/6))
emits `dsh-keysmith/v1` with `--json` for deploy, `--status`, `--uninstall` and `--recover`.
Switch requires 0.4.0: 0.2.0 shipped the contract without `version`, `state` or
`dsh_dir_exists`. 0.4.0 is a Cordis plugin installer: it copies `plugin/` into
`profiles/<name>/node_modules/dsh-keysmith` (default `web`), appends
`dsh.profile.bundles`, and writes a body insert. It does not dump `AGENTS.md`,
does not rewrite the home `cordis.patch.yml`, and does not patch
`@deepseek-ai` `node_modules`. `--file` overlays `plugin/lib/contract.md`.
`--recover` is a documented no-op (no journals). `adapter_patch` is always
`absent`. The sidecar bundles `plugin/` and `examples/dsh-unrestricted.md`.
The vendored CLI has a local frozen-path shim (`sys._MEIPASS`) so the
PyInstaller onefile can find `plugin/` next to the script.
There is no preview token — `--yes` is the whole confirmation — and `--json`
messages stay English whatever `--lang` says. Live Switch packs in
`$DSH_HOME/AGENTS.md` stay untouched.
