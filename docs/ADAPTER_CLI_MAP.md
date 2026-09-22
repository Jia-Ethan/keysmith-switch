# Adapter CLI 映射

统一 command 到四个钉选 CLI 的 argv。所有参数均为独立 argv 元素。

## Claude (`third_party/keysmith/claude/claude-instruct.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version` |
| status | `status --scope <scope> [--project-dir <abs>] [--runtime] --json` |
| plan-activate | `install --scope <scope> --file <abs.md> --name <safe> [--project-dir <abs>] [--runtime] [--append-file <abs.md>] [--max-tokens <n>] --json` |
| activate | 同上 + `--yes`。`--runtime` 只在 user scope；`--max-tokens` 是正整数 |
| plan-deactivate | `uninstall --scope <scope> [--project-dir <abs>] --json` |
| deactivate | 同上 + `--yes` |
| doctor | `doctor --json` |
| recover | `recover --scope <scope> [--project-dir <abs>] --json` 确认后加 `--yes` |
| backups | `backups --scope <scope> [--project-dir <abs>] --json`。只读，没有 `--yes` |
| restore | `restore --target <abs> --backup <name> [--scope <scope>] [--project-dir <abs>] --json`。预览在前，确认后加 `--yes` |

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
| scenario-list | `--scenario-list --lang en` |
| scenario-status | `--scenario-status [--target-dir <abs>] [--scenario-root <abs>] --lang en` |
| scenario-deploy | `--deploy-scenario <id> [--target-dir <abs>] --lang en`，确认后加 `--yes` |
| scenario-uninstall | `--scenario-uninstall <deployment id> [--target-dir <abs>] --lang en`，确认后加 `--yes` |
| scenario-recover | `--scenario-recover [--target-dir <abs>] --lang en`，确认后加 `--yes` |
| scaffold-list | `--scaffold-list [--pack-dir <abs>] --lang en` |
| scaffold | `--scaffold <pack> [--pack-dir <abs>] --lang en`，确认后加 `--yes` |
| scaffold-uninstall | `--scaffold-uninstall <pack> --lang en`，确认后加 `--yes` |

无稳定 JSON。解析结构化文本字段：`配置激活状态` / `activation`、`conflict`、`recovery`、journal、paths。scope 仅 `user`。不传 `--json`。不搬 `--preset`。内置 overlay 是 `third_party/keysmith/codex/examples/gpt-overlay.md`，跟其他示例一样可导入，不通过 CLI 选择。预览默认不加 `--yes`。

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

进阶 `run` 传 `--prompt` / `--mode` / `--timeout` 等，不再退回 `--help`。`breaktest` 必须带 `--bank`。两者按行发 `advanced-output`，`cancel_advanced` 结束进程。

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
| recover | 不支持：envelope `ok=false`，`unavailableReason` 说明无 recover |

仅 macOS。Windows 不调用 CLI。scope 仅 `user`。不支持 recover。

doctor / install 继续解析文本行，不传 `--json`。0.3.2 的 uninstall 在带 `--json` 时输出 `zcode-keysmith/v1`；解析器两者都接受。现有 argv 仍是文本模式。
