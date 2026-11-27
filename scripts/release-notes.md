# Keysmith Switch v0.9.1

重构 Agent 清理与回滚体验：改为默认只清 Keysmith 自身部署的分项勾选模式，并将历史会话纳入本地版本快照可一键回滚；同步支持 ZCode 3.17–4.0、DSH Desktop 配置档及小窗口侧栏自适应。

## 这一版有什么

- **清理改为分项勾选，默认不碰个人资产。** 打开任一 Agent 的「清理」弹窗时，默认仅勾选「Keysmith 提示词部署与残留」；用户级指令文件（`CLAUDE.md` / `AGENTS.md`）、记忆文件夹、自定义扩展（`skills` / `rules` / `agents` / `commands` / `hooks`）、会话历史以及登录/缓存默认全部不勾选，顶部明确展示保持不变的个人资产。
- **会话历史支持版本快照与一键回滚。** 勾选清理 Claude Code、Codex、Grok Build 或 ZCode 的会话历史与状态数据库（含 SQLite `-wal` / `-shm` 附属文件）时，不再直接删除，而是按同卷原子整组移入本机 `~/.keysmith-switch/snapshots/`，可在「设置 → 版本」随时恢复，且回滚前会自动保存当前状态并支持撤销。
- **高危项独立门禁与安全加固。** 登录凭证（钥匙串、`auth.json`）、可能内嵌密钥的配置文件（备份前剥离登录字段）及插件/缓存归入独立红色危险区，必须二次勾选确认并退出运行中的 Agent；补充祖先目录符号链接/Windows junction 越界防护、并发确认原子锁与 ZCode 旧版 runtime 备份保护。
- **ZCode 3.17–4.0 兼容。** 随包升级 `zcode-keysmith` 0.3.5，适配新版 ZCode 的运行时注入锚点。
- **DSH Desktop 配置档支持。** 随包升级 `dsh-keysmith` 0.4.1，检测到 DSH Desktop 存在时优先部署到 `profiles/desktop`；若此前部署在旧的 `profiles/web`，顶部会提示一键重新部署。
- **小窗口侧栏适配。** 窗口高度较矮时自动折叠侧栏 Agent 名称、保持「设置」按钮始终可见，且首次启动不会超出屏幕工作区。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.9.1`，安装仍需你确认。如果 v0.8.3 及更早的版本开着代理时检查更新超时，请临时开启代理软件的 TUN / 增强模式，或从下方下载安装包手动安装。
- macOS Apple Silicon：`Keysmith.Switch_0.9.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.9.1_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。

Windows 上如果火绒等杀毒软件提示有病毒：只要是从本页下载的，就是误报，可以放心使用。在杀毒软件的「隔离区」恢复被隔离的文件，再把 Keysmith Switch 加入「信任区」即可，详见 [README](https://github.com/Jia-Ethan/keysmith-switch#-下载)。
