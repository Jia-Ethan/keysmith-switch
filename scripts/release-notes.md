# Keysmith Switch v0.4.0

新模块：**输入替换**。你在 Codex 里照常打字，发给模型前按你的规则替换字面文字；输入框和会话历史里仍然是你打的原文。

## 这一版有什么

- **输入替换页**：在左侧栏打开。
  - 总开关和「对 Codex 生效」开关。关闭后不会替换任何内容，你的规则会保留。
  - 「我的规则」默认是空的，由你自己填写。规则按字面匹配、区分大小写；同一位置取最长的那条；替换后的文字不会再被替换；以 `/` 开头的消息整条跳过。
  - 只替换你自己输入的文字（包括历史里你之前说过的话）。助手回复、工具输出和 Codex 自带的上下文都不会改动。
- **连接 Codex**：点「连接 Codex」后，Switch 会在本机启动一个只监听 `127.0.0.1` 的中转服务，并在 Codex 的 `config.toml` 里新增一个 provider 指向它。你原来的 provider 配置不会被修改，写入前会自动备份。中转服务会开机自启，Switch 没打开时也能工作。
  - 「断开」会让 Codex 改回直接连接，并停止中转服务；你的规则会保留。
  - 部署或撤销 Codex 提示词都不会影响这个连接。
  - 如果其他工具（例如 cc-switch）把 Codex 切到了别的 provider，输入替换页会提示「没有经过中转」，你可以重新连接。
- **规则包**：拓展仓库现在也可以发布输入替换规则包。安装前会先列出全部规则，你可以选择「安装并启用」或「只安装」。已经启用的规则包有新版本时不会自动生效，要你在输入替换页看过差异、确认后才切换。目前官方没有上架任何规则包。

## 目前的限制

- 只支持 Codex。Claude Code、Grok Build 和 ZCode 以后再接入。
- Codex 必须使用自定义 provider（`config.toml` 里的 `[model_providers.xxx]`，`wire_api = "responses"`）。用 Codex 内置的 OpenAI 登录时暂时不能连接，页面上会说明原因。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.4.0`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.4.0_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.4.0_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
