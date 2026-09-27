# Keysmith Switch v0.1.9

Quick Deploy 不再绑定四套 Keysmith 官方提示词正文。粘贴自己的提示词，选择 Agent 与范围，审阅部署计划并确认后，再由现有 adapter 写入目标配置。

## 这一版有什么

- 快速部署支持粘贴提示词正文与自定义标题，选择支持的 scope；需要项目范围时指定项目目录。先保存到本地提示词库，再展示目标路径、写入动作、备份、警告与阻塞项，由用户确认执行。
- 默认粘贴路径离线工作，不再从四个兄弟仓库的远程分支抓取提示词。仍保留用户明确选择本地 Markdown 文件的导入路径。
- 同一 Agent 下相同正文复用既有条目，不会悄悄覆盖该条目的标题或标签；部署失败后条目仍在，可以重新预览和重试。
- 交给 adapter 的临时输入只包含提示词正文，不包含库条目的 Markdown 元数据；预览或执行完成后清理。确认时重新校验内容哈希、提示词版本及 adapter 状态，过期预览须重新生成。
- 保留 v0.1.8 的每次运行状态缓存、显式刷新与关闭窗口即退出行为。四个现有 Agent 继续使用随包提供的 legacy adapter；Agent registry 与外部 adapter 尚未加入本版。

## 安装

- 已安装 `v0.1.3` 及以上：可以在应用内检查更新到 `v0.1.9`；安装仍需用户确认。
- 仍在 `v0.1.1`：内置的是测试 updater 公钥，必须手动安装本版本。
- macOS Apple Silicon：`Keysmith.Switch_0.1.9_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.9_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
