# Keysmith Switch v0.3.8

部署被拦截时，现在会用你的语言说清楚原因和下一步。

## 这一版有什么

- **拦截提示不再是英文原文**：之前如果 Codex 的 `config.toml` 被手动改过或被其他工具改过，部署会被拦住，但提示是一整段英文，也没说该怎么办。现在提示按界面语言（简体中文、繁體中文、English）显示，并说明你的 `config.toml` 没有被改动。
- **告诉你下一步点哪里**：遇到这种情况，提示会引导你点左下角的「清理」清掉过期的部署记录，你的 `config.toml` 会原样保留，之后可以重新部署。

## 已知情况

- 这一版只翻译了上面这一类拦截提示，其他少数拦截原因仍可能显示英文原文，后续版本会继续补齐。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.3.8`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.3.8_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.3.8_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
