# Keysmith Switch v0.8.2

修复 DSH 部署失败时的状态显示，并避免部署过程为不完整的 profile 自动创建配置文件。

## 这一版有什么

- DSH 部署或停用失败时，不再按请求操作推导为已部署或已移除；错误信息继续保留。
- DSH 的 `profiles/web` 缺少必要配置文件时，跳过配置合成，使用基础补丁并显示警告，不调用 dsh 创建 profile 文件。回退时无法保留应用层额外加入的字段。
- 界面显示的 DSH 适配器版本与随包提供的 dsh-keysmith 0.2.2 一致。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.8.2`，安装仍需你确认。
- macOS Apple Silicon：`Keysmith.Switch_0.8.2_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.8.2_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。

Windows 上如果火绒等杀毒软件提示有病毒：只要是从本页下载的，就是误报，可以放心使用。在杀毒软件的「隔离区」恢复被隔离的文件，再把 Keysmith Switch 加入「信任区」即可，详见 [README](https://github.com/Jia-Ethan/keysmith-switch#-下载)。
