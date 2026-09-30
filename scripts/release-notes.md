# Keysmith Switch v0.2.4

修复确认部署被拒绝的问题；Claude Code 部署后不再显示为“未部署”。

## 这一版有什么

- 修复应用要求的适配器版本落后于随包适配器（Claude Code 7.2、Codex 0.6.0、Grok 0.6.1、ZCode 0.3.2），导致确认部署时被判定为“版本不匹配”而拒绝的问题。
- 修复 Claude Code 刚部署完提示词，状态却显示“未部署”的问题：读取状态时不再要求终端包装脚本。
- 新增测试：应用要求的适配器版本与随包适配器不一致时，测试直接失败，避免同类问题再次出现。

## 安装

- 已安装的版本可以在应用内点「检查更新」升级到 `v0.2.4`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.2.4_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.2.4_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
