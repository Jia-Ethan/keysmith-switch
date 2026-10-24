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
| dsh | dsh/dsh-keysmith.py | `8879d7b538b856413e7c78b16699fbd2d0f2743d` (merged #2; v0.2.2 tag pending) | v0.2.2 |

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

dsh 0.2.2 (`Jia-Ethan/dsh-keysmith`) emits `dsh-keysmith/v1` with `--json` for deploy, `--status`,
`--uninstall` and `--recover`. Switch requires 0.2.1: 0.2.0 shipped the contract without
`version`, `state` or `dsh_dir_exists`, so it cannot report drift. 0.2.2 is taken because it
restates the `system-prompt` row from `dsh --dump-default-config` instead of writing the three
base fields on their own — the older body deleted `personaPrefix`/`personaSuffix`, which
`dsh-web-app` adds to that row and which the home layer then hid from every session. Its
`--json` shape is unchanged, so the adapter needed no parser change; only the pin moved.
dsh-keysmith bundles its built-in prompt as a source constant and reads no data files, so the
sidecar needs no `datas`. It also has no preview token — `--yes` is the whole confirmation —
and its `--json` messages stay English whatever `--lang` says.

The DSH copy also skips composition for incomplete `profiles/web` directories,
falling back with a warning without asking dsh to create unowned profile files.
