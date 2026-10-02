# Keysmith Switch v0.3.2

修复一个状态显示矛盾的问题。

## 这一版有什么

- **部署状态不再自相矛盾**：如果 Agent 的配置在部署后被手动改过，之前页面顶部会显示“当前未部署 / 还没有部署提示词”，下面的列表却把同一条提示词标成“部署中”。现在顶部会正常显示正在使用的提示词，并提示“配置在部署后被手动改过”，你可以重新部署，或用“清理”从头开始。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.3.2`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.3.2_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.3.2_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
