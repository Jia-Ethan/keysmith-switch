# Keysmith Switch v0.9.0

拓展页新增组件包：把技能、斜杠命令、子 Agent 和附加规则文件装进 Claude Code、Codex 和 Grok Build 自己的目录。

## 这一版有什么

- **组件包（`kind: "harness"`）。** 每个组件写进对应 Agent 的目录：
  - Claude Code：`~/.claude/{skills,commands,agents,rules}`。
  - Codex：子 Agent 转成 `~/.codex/agents/<名字>.toml`；技能写进共享目录 `~/.agents/skills`，ZCode、pi、DSH 也会读到，安装前会提示。
  - Grok Build：`~/.grok/{skills,commands,agents}` 和 `~/.grok/rules/60-<名字>.md`。

  这台电脑上没装的 Agent 会跳过，不会替它新建目录。
- **装上不运行任何代码。** 只收纯文本组件。`allowed-tools`、`hooks`、`model`、`mcpServers` 这类会改权限或运行代码的设置一律拒绝，会执行 shell 的写法也拒绝。App 写入前会按格式规则再查一遍。
- **先看再装。** 安装和更新都先列出每个文件：给哪个 Agent、写到哪里、完整内容；更新时显示差异。你确认后才写。组件包不参与「全部更新」。
- **不覆盖你的文件。** 要写的位置已经有不是 Keysmith 写的文件时，整次安装会被拦下，并标出是哪个文件。卸载只删你没改过的文件；你改过的会保留，更新时也不覆盖。
- **清理和回滚。** 清理 Agent 时，组件包装的文件随所在目录一起存进快照，回滚时一起放回。
- 组件包需要 v0.9.0。旧版 App 不显示组件包，提示词包和规则包照常使用。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.9.0`，安装仍需你确认。如果 v0.8.3 及更早的版本开着代理时检查更新超时，请临时开启代理软件的 TUN / 增强模式，或从下方下载安装包手动安装。
- macOS Apple Silicon：`Keysmith.Switch_0.9.0_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.9.0_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。

Windows 上如果火绒等杀毒软件提示有病毒：只要是从本页下载的，就是误报，可以放心使用。在杀毒软件的「隔离区」恢复被隔离的文件，再把 Keysmith Switch 加入「信任区」即可，详见 [README](https://github.com/Jia-Ethan/keysmith-switch#-下载)。
