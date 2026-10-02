# Keysmith Switch v0.3.1

新增“公告”：Keysmith 可以随时给你发公告，不用等发新版本。

## 这一版有什么

- **公告**：左侧栏“设置”上方多了一个铃铛，有新公告时会出现小红点。点开可以看到全部公告，比如这一版有什么、下一版预告，以及各个 Agent 推荐的登录方式和支持的模型版本。
- **置顶公告**：重要的公告会显示在 Agent 页面顶部，点右上角可以关掉，关掉后不再出现。有些公告只在对应的 Agent 页面显示。
- **三种语言**：公告支持简体中文、English 和繁體中文，跟随 App 的语言设置。
- **安全**：公告只从 Keysmith 的官方仓库读取，只显示纯文本；链接只会在浏览器里打开官方网站。读取失败时不影响使用。

## 安装

- 已安装 `v0.1.3` 及以上的版本可以在应用内点「检查更新」升级到 `v0.3.1`；安装仍需用户确认。
- macOS Apple Silicon：`Keysmith.Switch_0.3.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.3.1_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
