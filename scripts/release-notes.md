# Keysmith Switch v0.1.5

首屏改为四个 Harness。选中一个之后只有部署和卸载，各一次确认、一次结果。

## 这一版有什么

- 首屏是 Claude Code、Codex、Grok Build、ZCode。提示词库、范围、诊断、搜索、标签、排序和新建离开首屏。已有提示词数据保留。
- 部署拉取该工具仓库当前默认提示词，写入本机库，再经现有 sidecar 部署。卸载走同一条确认后的停用路径。
- Claude 用 `examples/claude-project-rules.md`，Codex 用 `examples/gpt-overlay.md`，Grok 用 `examples/grok-unrestricted.md`，ZCode 用 `examples/system-role.md`。
- 没有可自动部署的情况停在同一个按钮的失败态，原因写在旁边，可以重试。不另开一条安装流程。

## 安装

- 已安装 `v0.1.3` 或 `v0.1.4`：可以在应用内更新到 `v0.1.5`。
- 仍在 `v0.1.1`：内置的是测试 updater 公钥，必须手动安装本版本。
- macOS Apple Silicon：`Keysmith.Switch_0.1.5_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.5_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名。
