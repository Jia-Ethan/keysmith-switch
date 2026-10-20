# Adapter CLI 映射

统一 command 到六个钉选 CLI 的 argv。所有参数均为独立 argv 元素。

## Claude Code (`third_party/keysmith/claude/claude-instruct.py`)

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
| plan-deactivate | `uninstall --dry-run --json` |
| deactivate | `uninstall --yes --json`。`--json` 在 `--yes` 之后 |
| doctor | `doctor` |
| recover | 不支持：envelope `ok=false`，`unavailableReason` 说明无 recover |

仅 macOS。Windows 不调用 CLI。scope 仅 `user`。不支持 recover。

doctor / install 继续解析文本行，不传 `--json`。0.3.2 的 `uninstall --help` 有 `--json`；带上它时 stdout 是 `zcode-keysmith/v1`（`operation=uninstall`，`mode=preview|execute`，`write` 与 `removed`）。plan-deactivate / deactivate 传 `--json`。解析器仍接受不带 `--json` 的文本行。`--json` 不用于 doctor、install 或 `--version`。

## pi (`third_party/keysmith/pi/pi-keysmith.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version`（argparse 文本，不是 envelope） |
| status | `--status --pi-dir <abs> --json --lang en` |
| plan-activate | `--file <abs.md> --name <safe> --pi-dir <abs> --dry-run --json --lang en` |
| activate | `--file <abs.md> --name <safe> --pi-dir <abs> --expected-preview-token <64 hex> --yes --json --lang en` |
| plan-deactivate | `--uninstall --pi-dir <abs> --json --lang en` |
| deactivate | `--uninstall --pi-dir <abs> --expected-preview-token <64 hex> --yes --json --lang en` |
| doctor | 同 status |
| recover | `--recover --pi-dir <abs> --json --lang en`。确认时加 `--expected-preview-token <64 hex> --yes` |
| backups / restore / reconcile | 不支持 |

`--pi-dir` 每次都传，且必须是绝对路径：隔离 home 时是 `<home>/.pi/agent`，否则是真实 home 下的 `~/.pi/agent`。这一点与 Grok 不同。CLI 从不创建这个目录；目录不存在时 status 为 `not-installed`，部署被 `pi-dir-missing` 拦下。schema: `pi-keysmith.envelope.v1`，契约见 pi-keysmith `v0.2.0` 的 `docs/json-contract.md`。scope 仅 `user`；`target.scope` 不是 `user`，或 `version` 低于 `0.2.0`，都按契约破坏处理。只写 `SYSTEM.md` 与它自己的 manifest，不动 `APPEND_SYSTEM.md`、`AGENTS.md`、`CLAUDE.md`、`settings.json`、`models.json`。

`normalize_pi` 与 `normalize_grok` 分开：

- 状态在 `status.*`：`not-installed` / `conflict` / `recovery-required` / `drift` / `active-aligned`（映射为 `active`）。没有 `inactive`。部署预览与确认上的 `drift` 不算需要修复：pi 会备份并替换手改过的 `SYSTEM.md`。
- status 在 `conflict` / `recovery-required` 时 `ok=false`、exit 1，但这是有效状态，不当成 CLI 故障；`drift` 的 exit 是 0。
- `diagnostics` 是 `"<code>: <说明>"`，只按第一个冒号前的 code 匹配。全部进 `warnings`；`windows-beta` 只是 warning。
- blockers：预览和确认取 `plan.blockers`，status 按操作取 `status.blockers.deploy` / `.uninstall`；code 换成对应的 diagnostic 句子。
- 确认 token 只取 `plan.confirmation_token`，必须是 64 位小写 hex。
- `plan.writes[]` 是计划文件（路径拼在 `pi_dir` 下），`status.nodes.system` 是当前规则节点（`fingerprint.sha256` → `currentFingerprint`），`status.residue` 非空时提示 recover。
- 退出码 2（用法错误）、3（`preview-token-mismatch`，提示重新预览）、5（`lock-busy`，可重试）都保证零写入；只有 4（`write-failed`）标记需要 recover。

清理只走 adapter 自己的 uninstall，不动 pi 的会话、设置和其他文件。输入替换不支持 pi。

## DSH (`third_party/keysmith/dsh/dsh-keysmith.py`)

| 统一命令 | argv |
| --- | --- |
| version | `--version`（argparse 文本，不是 envelope） |
| status | `--status --dsh-dir <abs> --json --lang en` |
| plan-activate | `--file <abs.md> --name <safe> --dsh-dir <abs> --dry-run --json --lang en` |
| activate | `--file <abs.md> --name <safe> --dsh-dir <abs> --yes --json --lang en` |
| plan-deactivate | `--uninstall --dsh-dir <abs> --json --lang en` |
| deactivate | `--uninstall --dsh-dir <abs> --yes --json --lang en` |
| doctor | 同 status |
| recover | `--recover --dsh-dir <abs> --json --lang en`。确认时加 `--yes` |
| backups / restore / reconcile | 不支持 |

`--dsh-dir` 每次都传，且必须是绝对路径：隔离 home 时是 `<home>/.dsh`，否则解析真实 home——`$DSH_HOME` 设了就用它（与 CLI 自己的 `_default_dsh_home` 一致），否则 `~/.dsh`。唯一的例外是 Mirasim 的会话挂载点：Mirasim 用 `DSH_HOME=~/.mirasim/agent-homes/dsh/<id>` 启动每个 dsh 会话，那个目录是一栋指回 `~/.dsh` 的符号链接楼，只属于当次会话，所以继承到的值若落在 `agent-homes/` 下面就忽略，仍然部署到 `~/.dsh`。CLI 从不创建它，也不会替用户跑一次 dsh；目录不存在时 `dsh_dir_exists=false`、state 为 `not-installed`，部署以 exit 2 被拦下。schema: `dsh-keysmith/v1`，随包版本 0.2.1。

`normalize_dsh` 的约定：

- **没有预览 token。** 0.2.1 的 CLI 不产生 `confirmation_token`，所以 `--yes` 是全部确认，`uses_preview_token(Dsh)` 为 false。预览与确认之间若有人手改了 `AGENTS.md`，dsh-keysmith 会先备份再覆盖，不会丢内容；部署计划里照实写明这一点。
- 状态由 CLI 直接给出：`state` 取 `not-installed` / `active` / `drift` / `recovery-required`。`drift` 时 `ok` 仍是 true、exit 0：部署还在，只是不再等于当初写下的内容。
- 部署预览与确认上的 `drift` 映射为 `inactive` 而非 `drift`：dsh-keysmith 会备份并替换手改过的 `AGENTS.md`，标成 `drift` 会读成「先修复」。
- 只有 `--status` / `--recover` 报 `state`；`deploy` / `--uninstall` 报的是操作结果。确认写入成功后，`activate` 按 `active`、`deactivate` 按 `not-installed` 推导，省掉一次往返。
- `mode` 是 `preview` / `execute`，envelope 的 `preview` 布尔由它换算。
- `blockers` / `warnings` / `error` 原样取用，`--lang` 不影响它们（CLI 的 JSON 消息恒为英文），所以 Switch 统一传 `--lang en`；i18n 由界面负责。
- `manifest.resources.agents_md.after_sha256` → `currentFingerprint`，用来认出线上生效的是库里哪条提示词。
- `actions[]` 里 `noop` / `skip` 不进 `plannedFiles`：它们描述已经到位的资源，列成计划写入会夸大确认的效果。
- `adapter_patch.state` 为 `unknown` 或 `absent` 时必定加一条 warning。这是唯一会随上游 npm 更新静默失效的资源；不能让它读成「一切正常」。
- 退出码 2 是用法或目录缺失，保证零写入；exit 1 带 blocker 是漂移拒绝或事务残留。

托管三个资源：`~/.dsh/AGENTS.md`（部署的提示词本身）、`~/.dsh/cordis.patch.yml`（关掉 `includeHarnessIdentity`，即去掉 DSH 固定的身份句）、以及 `profiles/node_modules/@deepseek-ai/dsh-llm-deepseek/lib/index.js` 的流式 tool_call 热修。`settings.yaml`、`.credentials.yaml`、`sessions/`、`storages/` 一律不碰，清理时整块跳过。

漂移的处理与 pi 不同：`recover` 只收尾中断的事务，对漂移返回 0 actions，所以 `ops::plan_deactivate` 的漂移分支排除 DSH——沿用 pi 的「recover instead」会把用户指向一个什么都不做的按钮。adapter 自己的 `target drifted from the manifest` 已经指明了出路是重新部署。输入替换不支持 DSH。
