# Keysmith Switch v0.8.3

修复 Windows 上使用 ZCode 3.16.x 时，部署提示词显示成功、实际却不生效的问题。

## 这一版有什么

- 随包的 zcode-keysmith 从 0.3.2 升级到 0.3.4，兼容 ZCode 3.12、3.14 与 3.16.x。
- Windows 上识别新版 ZCode 的启动方式，保留官方 agent-server，把提示词接入实际使用的 system-message 路径。
- 部署前备份原始 runtime；停用部署时从备份恢复，不再依赖旧版的环境变量覆盖。
- macOS 同步获得上游 0.3.3–0.3.4 的 runtime 补丁修复：避免项目 `MEMORY.md` 覆盖部署的提示词，runtime-patch 模式下校验不再运行未使用的 wrapper smoke，并增强官方更新后的自动重补丁。
- 其他 Agent 的部署方式不变。升级 App 后，请在 ZCode 页面重新部署提示词，完全退出并重新打开 ZCode，再新建会话验证。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.8.3`，安装仍需你确认。
- macOS Apple Silicon：`Keysmith.Switch_0.8.3_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.8.3_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。

Windows 上如果火绒等杀毒软件提示有病毒：只要是从本页下载的，就是误报，可以放心使用。在杀毒软件的「隔离区」恢复被隔离的文件，再把 Keysmith Switch 加入「信任区」即可，详见 [README](https://github.com/Jia-Ethan/keysmith-switch#-下载)。
