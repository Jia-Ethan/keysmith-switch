# Keysmith Switch v0.5.1

修复：Grok Build 部署后无法停用（[#95](https://github.com/Jia-Ethan/keysmith-switch/issues/95)）。

## 修了什么

- **Grok Build 可以停用了。** 部署之后如果 `config.toml` 被改过，而且部署前的配置备份也不见了，原来点「停用」会被拒绝，「从备份恢复」显示成功但其实什么都没改，「清理」也会失败。
  - 现在停用弹窗会说明备份已经找不到，并提供「保留当前配置并停用…」。确认后，`config.toml` 保持你现在的样子，只移除 Keysmith 写入的那一段；提示词文件和 hooks 照常还原。
  - 只有改动仅限于配置文件本身时才会提供这个选项；如果还有其他改动，仍然会拒绝，不会强行覆盖。
- **「清理」不再删除 Grok 适配器自己的配置备份。** 之前清理会把 `config.toml.keysmith-backup-*` 当作旧副本一起删掉，这很可能就是备份不见的原因。
- Grok 适配器更新到 [grok-keysmith v0.6.2](https://github.com/Jia-Ethan/grok-keysmith/releases/tag/v0.6.2)。

已经遇到这个问题的用户：升级后再点一次「停用」即可。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.5.1`，安装仍需你确认。
- macOS Apple Silicon：`Keysmith.Switch_0.5.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.5.1_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。
