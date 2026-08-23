# Keysmith Switch v0.1.2

`v0.1.2` 是应用内更新通道的 bootstrap 版本。本版本恢复“检查更新”和“更新并重启”，并将生产 updater 公钥编译进 macOS 与 Windows 客户端。

## 主要变化

- 重新提供手动检查、更新确认、下载进度和更新失败重试。
- 前端不再展示 Preview、未签名、Developer ID、公证或 Authenticode 说明。
- 改进提示词详情与编辑流程、设置页空态、错误态和并发操作保护。
- 保留 Claude Code、Codex、Grok Build 和 ZCode 的提示词管理、安全确认与恢复流程。

## 安装与更新

- 现有 `v0.1.1` 内置的是测试 updater 公钥，不能验证本版本的生产 updater 签名，因此必须手动下载安装 `v0.1.2`。
- 安装 `v0.1.2` 后，后续版本可通过应用内“检查更新”完成更新。
- macOS 与 Windows 安装包不使用平台发行签名；系统仍可能显示安全警告或阻止自动启动安装程序。
- updater payload 使用独立生产密钥签名，客户端会在安装前验证签名；签名失败时保留当前版本。

## 支持平台

- macOS 12+，Apple Silicon
- Windows x64
- Linux、Intel Mac 和 Windows ARM64 暂不提供安装包
