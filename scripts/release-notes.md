# Keysmith Switch v0.6.0

「输入替换」现在也支持 **Grok Build**：Claude Code、Codex、ZCode、Grok Build 四个 Agent 都可以用了。

## 这一版有什么

- **Grok Build 也能用输入替换。** 在「输入替换」页点 Grok Build 那一行的「连接…」即可。
  - 连接前会先告诉你要改什么：在 `~/.grok/config.toml` 的末尾，给 Grok 的每个模型加一张 `[model."<id>"] base_url`，全部放在一对 `# === keysmith-switch input rewrite ===` 标记之间。标记以外的内容不动。
  - 断开时整段删除，只有这一段仍然是 Keysmith 写入的样子才会删；如果你或其他工具改过，就保持原样。
  - 用 xAI 账号登录或 API key 都可以连接。账号登录时，登录令牌会经过本机中转，中转服务不保存、不记录它。
  - 只改你自己打的字，也就是 `<user_query>` 里面的内容。不会改动的内容：
    - Grok 自带的上下文，例如 `<user_info>`、规则和提醒；
    - 你用 `@` 引用的文件内容，以及自定义命令展开出来的技能内容；
    - 以 `/` 开头的斜杠命令；
    - 会话标题等 Grok 自己发出的辅助请求。
  - 需要 Grok 先运行过一次（让它拉取过模型列表），之后才能连接。如果 Grok 以后新增了模型，页面会提示重新连接。
- **适配器更新到 grok-keysmith v0.7.0。** 它把上面那对标记之间的内容视为 Keysmith Switch 的区域：不当成配置被改动，部署、停用、整理配置和中断恢复时都会保留这一段。没有它，连接会被当成漂移而拒绝停用，撤销部署时还会被悄悄删掉。
- **清理 Grok 之前会先断开它的输入替换**，回滚时恢复的是你原来的配置。

## 目前的限制

- Claude Code 不支持 Bedrock 和 Vertex；ZCode 不支持账号登录的 Coding Plan；Codex 需要使用自定义 provider（`wire_api = "responses"`）。
- Grok 的 `config.toml` 里 Keysmith 的标记区域不能被拆开（只剩开始或结束标记）；出现这种情况时，页面会提示先修好。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.6.0`，安装仍需你确认。
- macOS Apple Silicon：`Keysmith.Switch_0.6.0_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.6.0_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。
