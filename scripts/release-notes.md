# Keysmith Switch v0.6.1

修复 Windows 上部署 Codex 时，权限不足的报错以混合语言显示的问题。

## 这一版有什么

- **Windows 权限错误提示现在跟随界面语言。** 设置为中文时，Codex 部署因目录权限不足（Windows ACL）失败的报错，不再是英文 Python 标签混着中文系统提示的形式，而是完整的本地化说明，并告知解决方法。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.6.1`，安装仍需你确认。
- macOS Apple Silicon：`Keysmith.Switch_0.6.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.6.1_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。
