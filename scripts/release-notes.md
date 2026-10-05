# Keysmith Switch v0.3.5

Windows 版现在可以使用 ZCode 了。

## 这一版有什么

- **Windows 上的 ZCode 不再显示「不可用」**：之前 Windows 版把 ZCode 整个禁用，页面只显示「ZCode Keysmith is not available on Windows」。现在 Windows 上可以像 macOS 一样部署、查看状态和移除 ZCode 的提示词。Switch 不会替你安装 ZCode 本体，请先从 ZCode 官方渠道安装。
- **部署后能认出当前生效的提示词**：Windows 上写入的文件使用 Windows 换行，之前这会让应用认不出刚部署的是哪条提示词。现在按文本内容识别，部署后会正确显示「部署中」。
- **读取状态时不再闪黑窗**：Windows 上每次读取状态都会短暂弹出一个控制台窗口，现在不会了。

## 已知情况

- 这一版的 Windows ZCode 支持首次发布，我们没有在真实 Windows 机器上完整走过一遍部署。如果遇到问题，请通过应用内的反馈入口告诉我们。
- 在 Windows 上部署 ZCode 会清除你的用户级环境变量 `NODE_OPTIONS`，移除提示词时不会恢复它。如果你自己设置过，部署后请重新设置。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.3.5`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.3.5_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.3.5_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
