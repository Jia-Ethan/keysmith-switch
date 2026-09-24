# Keysmith Switch v0.1.7

首屏只留一个按钮。未部署是「部署」，已部署是「卸载」。设置收成齿轮，工具页只列四个 Keysmith。

## 这一版有什么

- 打开页面和切换工具都先读本机状态。部署成功后只剩「卸载」，卸载成功后回到「部署」。失败停在原来那颗按钮旁边，可以重试。
- 四个工具都是整机部署一次。设置里不再有开机启动、静默启动、Claude 默认范围。
- 右上角是齿轮。浅色主题仍用太阳。
- 工具页是 Claude Keysmith、Codex Keysmith、Grok Keysmith、Zcode Keysmith，各自带仓库链接。不再显示路径、官方产品或高级工具。
- 问题上报会找到用户终端里已经登录的 GitHub CLI。找不到或没登录时，仍打开预填好的浏览器草稿。截图要到浏览器里补。

## 安装

- 已安装 `v0.1.3` 或 `v0.1.4`：可以在应用内更新到 `v0.1.7`。
- 仍在 `v0.1.1`：内置的是测试 updater 公钥，必须手动安装本版本。
- macOS Apple Silicon：`Keysmith.Switch_0.1.7_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.7_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名。
