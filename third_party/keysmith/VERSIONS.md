# Vendored Keysmith CLI pins

These copies are taken from the product repositories at the sidecar each desktop
ships. Do not edit them to change upstream behaviour. They ship atomically with
Keysmith Switch.

| Tool | Script | Source | Version |
| --- | --- | --- | --- |
| claude | claude/claude-instruct.py | `7dbfa253` (`v7.2`) | v7.2 |
| codex | codex/codex-instruct.py | `33cf4049` (`v0.6.0`) | v0.6.0 |
| grok | grok/grok-keysmith.py | `168f604a` (`v0.6.1`) | v0.6.1 |
| zcode | zcode/zcode-keysmith.py | `7348b875` (no `v0.3.2` tag; desktop `desktop-v0.1.0-beta.1`) | 0.3.2 |

`v0.3.1` is the Zcode CLI Latest tag and is not this pin. `v0.2.0` is older. The
desktop prerelease is the source of the 0.3.2 tree; it is not a CLI release and
is not Latest.

Advanced Tools also vendor `grok_keysmith_runner.py` and `grok_keysmith_breaktest.py`
from the same Grok commit. Zcode 0.3.2 emits `zcode-keysmith/v1` when `--json` is
passed. doctor and install stay on text. plan-deactivate and deactivate pass
`--json` on uninstall; the parser still accepts the text lines.
