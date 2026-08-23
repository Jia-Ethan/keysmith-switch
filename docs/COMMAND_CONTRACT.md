# Keysmith Switch command contract

前后端与测试共用。契约版本 `0.1.0`。envelope schema 固定为 `keysmith-switch/adapter-v1`。

## Tools

`claude` | `codex` | `grok` | `zcode`

- Claude scopes: `user` | `project` | `local`（project/local 必须有绝对 `projectDir`）
- Codex / Grok scopes: 仅 `user`
- ZCode scopes: 仅 `user`；仅 macOS。Windows 返回 `available: false`，原因明确，不执行安装命令

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
  "adapterVersion": "7.1",
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

## Tauri commands

参数/返回均为 camelCase JSON。

| command | 入参 | 返回 |
| --- | --- | --- |
| `list_tools` | `{}` | `{ tools: ToolInfo[] }` |
| `list_prompts` | `{ tool, query?, tag?, sort? }` | `{ prompts: PromptSummary[] }` |
| `get_prompt` | `{ id }` | `PromptDetail` |
| `create_prompt` | `{ tool, title, content, tags }` | `PromptDetail` |
| `update_prompt` | `{ id, title?, content?, tags? }` | `PromptDetail` |
| `delete_prompt` | `{ id }` | `{ ok }` |
| `copy_prompt` | `{ id, targetTool }` | `PromptDetail` |
| `prompt_history` | `{ id }` | `{ versions: PromptVersion[] }` |
| `prompt_diff` | `{ id, fromVersion, toVersion }` | `{ unified, summary }` |
| `restore_prompt_version` | `{ id, version }` | `PromptDetail` |
| `tool_status` | `{ tool, scope?, projectDir? }` | `Envelope` |
| `plan_activate` | `{ promptId, scope, projectDir? }` | `{ operationId, envelope }` |
| `activate` | `{ operationId }` | `{ operationId, envelope }` |
| `plan_deactivate` | `{ promptId?, tool, scope, projectDir? }` | `{ operationId, envelope }` |
| `deactivate` | `{ operationId }` | `{ operationId, envelope }` |
| `recover_tool` | `{ tool, scope?, projectDir? }` | `{ operationId, envelope }` |
| `doctor` | `{ tool }` | `Envelope` |
| `list_activations` | `{ tool }` | `{ activations: Activation[] }` |
| `list_operations` | `{ tool? }` | `{ operations: Operation[] }` |
| `get_settings` | `{}` | `Settings` |
| `update_settings` | `SettingsPatch` | `Settings` |
| `get_about` | `{}` | `AboutInfo` |
| `check_app_update` | `{ channel? }` | `UpdateCheck` |
| `install_app_update` | `{ confirmed: true }` | `UpdateInstall` |
| `plan_official_action` | `{ product, action }` | `OfficialPlan` |
| `confirm_official_action` | `{ planId, confirmed: true }` | `OfficialResult` |
| `list_advanced_tools` | `{}` | `{ tools, enabled }` |
| `run_advanced` | `{ kind, args }` | `AdvancedResult` |

## Settings

```ts
{
  language: "zh-CN" | "zh-TW" | "en";
  updateChannel: "stable" | "beta";
  advancedToolsEnabled: boolean;
  defaultClaudeScope: "user" | "project" | "local";
  recentProjectDirs: string[];
  updaterEndpointOverride: string | null; // 仅本地 fixture
}
```

## Official products

`claude` | `codex` | `grok` | `zcode`

计划必须包含：当前版本、最新版本、是否已安装、可执行路径、安装来源、完整 argv、目标位置、确认后才执行。

## Updater

- 默认 stable endpoint：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`
- beta endpoint：`https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/beta/latest.json`
- 环境覆盖：`KEYSMITH_SWITCH_UPDATER_ENDPOINT`、`KEYSMITH_SWITCH_UPDATER_PUBKEY`
- 禁止静默安装；`install_app_update` 必须 `confirmed: true`
- 签名失败、离线、损坏 metadata、降级、下载中断：保持当前版本
- 失败提供公开 Release 页：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases`

## HOME 与锁

- `KEYSMITH_SWITCH_HOME` 覆盖数据根
- 适配器子进程继承测试 `HOME`
- 并发写使用 `~/.keysmith-switch/.lock`
- 密钥/token/cookie 永不写入日志、toast、DB error、测试快照
