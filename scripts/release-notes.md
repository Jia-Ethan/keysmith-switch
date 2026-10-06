# Keysmith Switch v0.3.9

补上 v0.3.8 提示里漏掉的一句话：清理会删除聊天记录。

## 这一版有什么

- **清理前说清楚会删什么**：v0.3.8 在 Codex 部署被拦截时，会引导你点左下角的「清理」，但提示只说会清掉过期的部署记录、保留 `config.toml`，没有提到清理同时会**永久删除这个 Agent 的会话历史和登录**。这些内容不会进回收站，也不在「设置 → 版本」里，删除后无法恢复。现在提示会写明这一点，并建议想保留聊天记录的话，先备份整个 `.codex` 文件夹，再决定要不要清理。
- 「清理」本身的行为没有变化。

## 如果你在 v0.3.8 里已经点过清理

被删除的会话历史只能从你自己的备份（例如 macOS 的 Time Machine）里找回。恢复前请先退出 Codex，避免新的会话写入。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.3.9`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.3.9_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.3.9_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
