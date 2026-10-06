# Keysmith Switch v0.5.0

「输入替换」现在也支持 **Claude Code** 和 **ZCode**，「输入替换」页也重新设计了。

## 这一版有什么

- **Claude Code 和 ZCode 也能用输入替换。** 在「输入替换」页每个 Agent 一行，点「连接…」即可。
  - 连接前会先告诉你要改哪个文件的哪个字段：
    - Claude Code：只改 `~/.claude/settings.json` 里的 `env.ANTHROPIC_BASE_URL`。
    - ZCode：只改 `~/.zcode/v2/provider_config.json` 里每个个人 provider 的地址。
  - 断开时只改回这一项；如果这期间别的工具改过它，就保持原样。
  - 用 API key、订阅登录或自定义网关的 Claude Code 都可以连接。订阅登录时，登录令牌会经过本机中转，但中转服务不保存、不记录它。
  - 只改你自己打的字。不会改动的内容：
    - Claude Code 的 `<system-reminder>`、工具结果、`!` 命令输出；
    - ZCode 的技能列表和 AGENTS.md 上下文；
    - 斜杠命令和它展开出来的模板；
    - ZCode 子代理发出的请求。
  - 部署或撤销 Claude Code 提示词不会影响连接。清理某个 Agent 之前，会先断开它的输入替换，这样回滚时恢复的是你原来的地址。
- **规则可以只对部分 Agent 生效。** 每张规则表都能选择适用于哪些 Agent，默认是全部。规则包也可以面向任意 Agent。
- **「输入替换」页重新设计。**
  - 开关改成了开关样式。拓展页的启用和停用也改成了开关。
  - 每个 Agent 一行，同时显示它的状态和操作。
  - 不再显示 provider id。
  - 「断开…」改成醒目的红色按钮。
  - 规则表支持按 Agent 筛选、搜索、排序和折叠。
  - 空状态更简洁。
  - 深色和浅色主题下的状态文字都更清楚。

## 目前的限制

- Grok Build 暂不支持，因为它的 Keysmith 适配器卸载时会把整个 `config.toml` 还原，连接会被一起删掉。等适配器支持后再开放。
- Claude Code 暂不支持 Bedrock 和 Vertex。Bedrock 会对每个请求签名，改写后签名会失效。
- ZCode 暂不支持账号登录的 Coding Plan，请改用 API key 的 provider。
- Codex 仍然需要使用自定义 provider（`wire_api = "responses"`）。

## 安装

- 已安装 `v0.1.3` 及以上版本的，可以在应用内点「检查更新」升级到 `v0.5.0`，安装时仍需你确认。
- macOS Apple Silicon：`Keysmith.Switch_0.5.0_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.5.0_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。
