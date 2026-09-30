# Keysmith Switch v0.2.3

部署状态不再和提示词列表对不上；授权改为 PolyForm Noncommercial 1.0.0。

## 这一版有什么

- 机器上已经部署了提示词、但提示词库里没有对应条目时，“当前部署”分组会出现一张标着“部署中”的卡片，不再显示“还没有提示词”。库里能匹配到的提示词同样标为“部署中”。
- ZCode 的图标换回官方应用图标。
- 授权由 MIT 改为 [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0/)：源码公开，个人使用、学习和研究可以自由使用，商业使用需另行取得授权。`v0.2.2` 及更早版本已按 MIT 发布，不受影响。改编自 CC Switch 的三个文件仍保留其 MIT 声明，见 `THIRD_PARTY_NOTICES.md`。

## 安装

- 已安装的版本可以在应用内点「检查更新」升级到 `v0.2.3`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.2.3_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.2.3_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
