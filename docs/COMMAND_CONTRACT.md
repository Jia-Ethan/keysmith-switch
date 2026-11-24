# Keysmith Switch command contract

前后端与测试共用。契约版本 `0.2.0`。envelope schema 固定为 `keysmith-switch/adapter-v1`。

## Tools

`claude` | `codex` | `grok` | `zcode` | `pi` | `dsh`

- Claude Code scopes: `user` | `project` | `local`（project/local 必须有绝对 `projectDir`）
- Codex / Grok scopes: 仅 `user`
- ZCode scopes: 仅 `user`。适配器会在所选 runtime 实际探测可用性；官方没有 Linux 安装源时 WSL 会明确报 unavailable。Switch 不自动安装 ZCode 本体
- pi scopes: 仅 `user`（`~/.pi/agent`）。支持 recover；不支持 backups / restore / reconcile，也不支持输入替换。没有官方产品安装计划
- dsh scopes: 仅 `user`（`~/.dsh`）。支持 recover（0.4.0 为 no-op）；不支持 backups / restore / reconcile，也不支持输入替换。确认不绑定预览（dsh-keysmith 0.4.0 没有预览 token）。没有官方产品安装计划

## Adapter envelope

```json
{
  "schema": "keysmith-switch/adapter-v1",
  "tool": "claude",
  "command": "status",
  "ok": true,
  "preview": true,
  "available": true,
  "unavailableReason": null,
  "adapterVersion": "7.2",
  "cliPath": "/abs/path",
  "argv": ["python3", "...", "status", "--json"],
  "exitCode": 0,
  "status": "inactive",
  "recoveryRequired": false,
  "scopes": [{"id": "user", "supported": true, "reason": null}],
  "targetPaths": [{"path": "~/.claude/CLAUDE.md", "role": "memory", "exists": true}],
  "plannedFiles": [{"path": "...", "action": "write", "detail": "..."}],
  "backups": [{"target": "...", "backupPath": null, "planned": true}],
  "conflicts": [],
  "warnings": [],
  "blockers": [],
  "currentFingerprint": null,
  "targetFingerprint": null,
  "doctor": {"ok": true, "checks": []},
  "reloadRequired": false,
  "reloadHint": null,
  "error": null,
  "redactedStderr": ""
}
```

`status` 取值：`not-installed` | `inactive` | `active` | `drift` | `conflict` | `recovery-required` | `unavailable`

## Runtime targets

```ts
type RuntimeTarget =
  | { kind: "native" }
  | { kind: "wsl"; distribution: string; user?: string };
```

- 运行时按 Agent 单独保存。省略 `runtimeTarget` 等同 `{ kind: "native" }`，保持旧客户端兼容。
- 提示词正文和版本库跨运行时共享；激活状态、机器状态、操作历史和前端机器状态缓存按运行时隔离。
- Windows 枚举已注册的具体发行版，过滤 `docker-desktop*` 并将 WSL1 标记为不可用。每个 WSL2 发行版会从 passwd 数据展开为可登录的 Linux 用户，runtime 身份由发行版和用户共同决定。
- WSL 支持 status、preview、deploy、deactivate、doctor、recover、reconcile、采用现有提示词、清理/快照/回滚、组件、官方 CLI 探测/安装、高级工具以及输入替换。每个 Agent/runtime 可覆盖绝对配置目录。
- WSL 文件写入全部由目标 Linux 用户执行的 helper 完成，不通过 `\\wsl$` / `\\wsl.localhost` 执行托管写入。执行时检查 `python3`，缺失则明确报错。
- 快照保存在 Windows App 数据目录，元数据冻结完整 runtime 身份。回滚使用快照自己的 runtime，不接受当前 UI 的替代目标。
- WSL 输入替换仍由 Windows `keysmith-relay` 执行。relay 仅监听 `127.0.0.1` 和已选 WSL 虚拟网卡的明确主机地址，不绑定 `0.0.0.0`；route、link record、规则表、规则包和开关都按 runtime 隔离。旧数据库中的替换配置迁移为 `native` 配置。
- Claude 的 WSL project/local 路径可使用目标发行版内的 Linux 绝对路径，或匹配该发行版的 `\\wsl.localhost\...` / `\\wsl$\...` UNC 路径。
- 所有 confirm 命令（部署、停用、恢复、整理、清理、回滚、组件、官方安装）从预览记录恢复 runtime，不接受新的目标。

## Tauri commands

参数/返回均为 camelCase JSON。

| command | 入参 | 返回 |
| --- | --- | --- |
| `list_tools` | `{}` | `{ tools: ToolInfo[] }` |
| `list_runtimes` | `{}` | `{ runtimes: RuntimeInfo[] }` |
| `list_prompts` | `{ tool, query?, tag?, sort?, runtimeTarget? }` | `{ prompts: PromptSummary[] }` |
| `get_prompt` | `{ id }` | `PromptDetail` |
| `create_prompt` | `{ tool, title, content, tags }` | `PromptDetail` |
| `create_pasted_prompt` | `{ tool, title, content }` | `PromptDetail`（新内容存入库并标记 `pasted`；同 Agent 同正文复用既有条目，返回其原有标题与标签） |
| `update_prompt` | `{ id, title?, content?, tags? }` | `PromptDetail` |
| `delete_prompt` | `{ id }` | `{ ok }` |
| `copy_prompt` | `{ id, targetTool }` | `PromptDetail` |
| `prompt_history` | `{ id }` | `{ versions: PromptVersion[] }` |
| `prompt_diff` | `{ id, fromVersion, toVersion }` | `{ unified, summary }` |
| `restore_prompt_version` | `{ id, version }` | `PromptDetail` |
| `tool_status` | `{ tool, scope?, projectDir?, runtimeTarget? }` | `Envelope` |
| `plan_activate` | `{ promptId, scope, projectDir?, runtimeTarget? }` | `{ operationId, envelope }` |
| `activate` | `{ operationId }` | `{ operationId, envelope }` |
| `plan_deactivate` | `{ promptId?, tool, scope, projectDir?, runtimeTarget? }` | `{ operationId, envelope }` |
| `deactivate` | `{ operationId }` | `{ operationId, envelope }` |
| `harness_state` | `{ tool, runtimeTarget? }` | `{ tool, deployed, error, promptId?, promptTitle? }`（`promptId`/`promptTitle` 仅在已部署且能在库中识别时返回） |
| `adopt_live_prompt` | `{ tool, title, runtimeTarget? }` | `PromptDetail` |
| `remove_harness` | `{ tool, runtimeTarget? }` | `HarnessOutcome` |
| `plan_reconcile` | `{ tool, runtimeTarget? }` | `{ operationId, envelope }` |
| `recover_tool` | `{ tool, scope?, projectDir?, runtimeTarget? }` | `{ operationId, envelope }` |
| `doctor` | `{ tool, runtimeTarget? }` | `Envelope` |
| `list_activations` | `{ tool, runtimeTarget? }` | `{ activations: Activation[] }` |
| `list_operations` | `{ tool?, runtimeTarget? }` | `{ operations: Operation[] }`；指定 `tool` 而省略目标时只返回本机历史，不指定 `tool` 时返回全局历史 |
| `get_settings` | `{}` | `Settings` |
| `update_settings` | `SettingsPatch` | `Settings` |
| `get_about` | `{}` | `AboutInfo` |
| `check_app_update` | `{ channel? }` | `UpdateCheck`（含 `installMode` 与 `reason`） |
| `install_app_update` | `{ confirmed: true }` | `UpdateInstall`（含 `installMode` 与 `reason`） |
| `plan_official_action` | `{ product, action, runtimeTarget? }` | `OfficialPlan`；探测、npm 查询和确认安装均在该 runtime 中执行 |
| `confirm_official_action` | `{ planId, confirmed: true }` | `OfficialResult` |
| `list_advanced_tools` | `{}` | `{ tools, enabled }` |
| `run_advanced` | `{ kind, args, runtimeTarget? }` | `AdvancedResult`；脚本、配置目录和 Linux 绝对路径均在所选 runtime 中解析 |
| `extension_plan_harness` | `{ packId, runtimeTarget? }` | `HarnessPlan`：组件包每个文件的 Agent、落点、动作（`add` / `update` / `same` / `keep-modified` / `remove` / `release` / `conflict`）、全文或 diff；`blocked` 为真时不能确认。只读不写 |
| `extension_confirm_harness` | `{ packId, planId, runtimeTarget? }` | `{ view, report }`；重新读包并重新规划，并校验 runtime |
| `rewrite_state` | `{ runtimeTarget? }` | `RewriteView` |
| `rewrite_set_switches` / `rewrite_set_table_tools` / `rewrite_save_user_rules` / `rewrite_set_table_enabled` / `rewrite_reorder_tables` / `rewrite_copy_to_user` / `rewrite_accept_update` | 原参数加 `{ runtimeTarget? }` | 选定 runtime 的 `RewriteView` |
| `rewrite_connect_*` / `rewrite_disconnect_*` | `{ runtimeTarget? }` | 连接或断开选定 runtime 中的 Agent，返回 `RewriteView` |

## Settings

```ts
{
  language: "zh-CN" | "zh-TW" | "en";
  updateChannel: "stable" | "beta";
  advancedToolsEnabled: boolean;
  defaultClaudeScope: "user" | "project" | "local";
  recentProjectDirs: string[];
  updaterEndpointOverride: string | null; // 仅本地 fixture
  agentRuntimeTargets: Partial<Record<ToolId, RuntimeTarget>>;
  agentDirectories: Array<{ target: RuntimeTarget; directories: Partial<Record<ToolId, string>> }>;
  extensionRuntimeTarget: RuntimeTarget;
  rewriteRuntimeTarget: RuntimeTarget;
}
```

## Official products

`claude` | `codex` | `grok` | `zcode`

计划必须包含：当前版本、最新版本、是否已安装、可执行路径、安装来源、完整 argv、目标位置、确认后才执行。

## Updater

- 默认 stable endpoint：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`
- beta endpoint：`https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/beta/latest.json`
- 环境覆盖：`KEYSMITH_SWITCH_UPDATER_ENDPOINT`、`KEYSMITH_SWITCH_UPDATER_PUBKEY`
- 代理：未设置 `https_proxy` / `all_proxy` 时，检查、下载和扩展请求使用 macOS / Windows 系统代理（updater 插件下载只用 HTTP 代理）；回环地址永不走代理
- 禁止静默安装；`install_app_update` 必须 `confirmed: true`
- metadata 可用 `minimum_updater_version` 声明能验证当前签名密钥的最早客户端；低于门槛时只返回目标版本的手动下载入口，不请求 updater payload
- `installMode` 为 `none | inApp | manual`；`reason` 为 `bootstrapRequired | signatureKeyMismatch | null`
- metadata 平台项优先提供正整数 `size`；旧 feed 缺失时才使用跟随重定向的 HEAD fallback
- 签名失败、离线、损坏 metadata、降级、下载中断：保持当前版本
- 失败提供公开 Release 页：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases`

## HOME 与锁

- `KEYSMITH_SWITCH_HOME` 覆盖数据根
- 适配器子进程继承测试 `HOME`
- 并发写使用 `~/.keysmith-switch/.lock`
- 密钥/token/cookie 永不写入日志、toast、DB error、测试快照
