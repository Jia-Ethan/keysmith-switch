# Adapter CLI 映射

统一 command 到四个钉选 CLI 的 argv。所有参数均为独立 argv 元素。

## Claude (`third_party/keysmith/claude/claude-instruct.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version` |
| status | `status --scope <scope> [--project-dir <abs>] [--runtime] --json` |
| plan-activate | `install --scope <scope> --file <abs.md> --name <safe> [--project-dir <abs>] --json` |
| activate | 同上 + `--yes` |
| plan-deactivate | `uninstall --scope <scope> [--project-dir <abs>] --json` |
| deactivate | 同上 + `--yes` |
| doctor | `doctor --json` |
| recover | `recover --scope <scope> [--project-dir <abs>] --json` 确认后加 `--yes` |

stdout JSON schema: `claude-keysmith/v1`。`blockers` 非空或 `ok=false` 即失败关闭。

## Codex (`third_party/keysmith/codex/codex-instruct.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version` |
| status | `--status --codex-dir <abs> --lang en` |
| plan-activate | `--file <abs.md> --name <safe> --codex-dir <abs> --dry-run --lang en` |
| activate | `--file <abs.md> --name <safe> --codex-dir <abs> --yes --lang en` |
| plan-deactivate | `--uninstall --codex-dir <abs> --lang en` |
| deactivate | `--uninstall --codex-dir <abs> --yes --lang en` |
| doctor | 与 status 相同（无独立 doctor） |
| recover | `--recover --codex-dir <abs> --lang en` 确认后加 `--yes` |

无稳定 JSON。解析结构化文本字段：`配置激活状态` / `activation`、`conflict`、`recovery`、journal、paths。scope 仅 `user`。

## Grok (`third_party/keysmith/grok/grok-keysmith.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version` |
| status | `--status --json --grok-dir <abs>` |
| plan-activate | `--file <abs.md> --dry-run --json --grok-dir <abs>` |
| activate | `--file <abs.md> --expected-preview-token <64 hex> --yes --json --grok-dir <abs>` |
| plan-deactivate | `--uninstall --json --grok-dir <abs>` |
| deactivate | `--uninstall --expected-preview-token <64 hex> --yes --json --grok-dir <abs>` |
| doctor | `--status --json --grok-dir <abs>` |
| recover | `--recover --json --grok-dir <abs>`。确认时加 `--expected-preview-token <64 hex> --yes` |
| reconcile | `--reconcile --json --grok-dir <abs>`。确认时加 `--expected-preview-token <64 hex> --yes` |

`--grok-dir` 必须绝对路径。schema: `grok-keysmith.envelope.v1`。状态：`not-installed` / `active-aligned` / `inactive` / `drift` / `conflict` / `recovery-required`。scope 仅 `user`。

确认 token 来自预览 envelope 的 `plan.confirmation_token`，不是 Switch 自己的 operation id。预览本身不带 `--yes`，也不带 token。

## ZCode (`third_party/keysmith/zcode/zcode-keysmith.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version` |
| status | `doctor` |
| plan-activate | `install --system-file <abs.md> --dry-run` |
| activate | `install --system-file <abs.md> --yes` |
| plan-deactivate | `uninstall --dry-run` |
| deactivate | `uninstall --yes` |
| doctor | `doctor` |
| recover | 不支持：envelope `ok=false`，`unavailableReason` 说明 v0.1.0 无 recover |

仅 macOS。Windows 不调用 CLI。scope 仅 `user`。无 JSON，解析 doctor/install 文本行。
