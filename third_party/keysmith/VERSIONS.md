# Vendored Keysmith CLI pins

These copies are taken from the product repositories at the sidecar each desktop
ships. Do not edit them to change upstream behaviour. They ship atomically with
Keysmith Switch.

| Tool | Script | Source | Version |
| --- | --- | --- | --- |
| claude | claude/claude-instruct.py | `7dbfa253` (`v7.2`) | v7.2 |
| codex | codex/codex-instruct.py | `33cf4049` (`v0.6.0`) | v0.6.0 |
| grok | grok/grok-keysmith.py | `d87def09` (`v0.8.0`) | v0.8.0 |
| zcode | zcode/zcode-keysmith.py | `7348b875` (no `v0.3.2` tag; desktop `desktop-v0.1.0-beta.1`) | 0.3.2 |
| pi | pi/pi-keysmith.py | `37f69cac` (`v0.2.0`) | v0.2.0 |
| dsh | dsh/dsh-keysmith.py | `4889f2d` (`v0.2.1`) | v0.2.1 |

`v0.3.1` is the Zcode CLI Latest tag and is not this pin. `v0.2.0` is older. The
desktop prerelease is the source of the 0.3.2 tree; it is not a CLI release and
is not Latest.

Advanced Tools also vendor `grok_keysmith_runner.py` and `grok_keysmith_breaktest.py`
from the same Grok commit. Zcode 0.3.2 emits `zcode-keysmith/v1` when `--json` is
passed. doctor and install stay on text. plan-deactivate and deactivate pass
`--json` on uninstall; the parser still accepts the text lines.

pi 0.2.0 (`Jia-Ethan/pi-keysmith`) emits `pi-keysmith.envelope.v1` with `--json`; its contract
is `docs/json-contract.md` at the same tag. `pi/examples/system-prompt.md` is the CLI's
bundled default prompt from that tag.

dsh 0.2.1 (`Jia-Ethan/dsh-keysmith`) emits `dsh-keysmith/v1` with `--json` for deploy, `--status`,
`--uninstall` and `--recover`. Switch requires 0.2.1: 0.2.0 shipped the contract without
`version`, `state` or `dsh_dir_exists`, so it cannot report drift. dsh-keysmith bundles its
built-in prompt as a source constant and reads no data files, so the sidecar needs no `datas`.
It also has no preview token — `--yes` is the whole confirmation — and its `--json` messages
stay English whatever `--lang` says.
