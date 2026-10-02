# Keysmith Switch v0.3.0

“清理”现在会把选中的 Agent 清回刚安装时的样子，四个 Agent 都支持。

## 这一版有什么

- **完整清理**：Claude Code、Codex、Grok 和 ZCode 的页面都有“清理”。它会移除 Keysmith 部署的内容，也会移除你自己加的规则、自定义 agents、命令、hooks、skills 和配置，让 Agent 打开后和第一次装好时一样。
- **登录和历史也会清掉**：登录文件、Claude Code 在系统钥匙串里的登录、会话历史、插件和缓存会直接删除，**不会保存**，清理后需要重新登录。
- **配置存一份不含登录的版本**：`settings.json`、`config.toml` 等配置会先存进版本里，存之前去掉所有 key、token 和账号信息；如果去掉后仍检测到登录信息，清理会停下，什么都不改。
- **回滚**：“设置 → 版本”里可以把规则、skills、配置等放回去。登录和会话历史不会回来；如果你已经重新登录，现有的配置文件不会被覆盖。回滚对话框会列出要恢复的内容。
- **部署预览无法确认时**：预览里会出现“清理”按钮，直接打开清理弹窗。配置被手动改过（drift）时也能清理。
- **不会动的东西**：你项目目录里的文件（包括 git 项目，以及 ZCode 的 `~/.zcode/workspace`）、你的提示词库、Agent 软件本身。ZCode 应用仍被 Keysmith 改动且无法还原时，清理会停下，先要求修复。
- **清理前要先退出 Agent**：Agent 正在运行时不能清理。Claude 桌面版不算 Claude Code。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.3.0`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.3.0_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.3.0_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
