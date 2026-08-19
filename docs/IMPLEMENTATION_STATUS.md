# Keysmith Switch — 实现状态

更新时间：2026-08-19 21:35 +08:00（Asia/Hong_Kong）  
阶段：**Preview 未签名本地实现。macOS Apple Silicon 已产出 .dmg。Windows 无原生实机。不 push / 不 Release / 不配置 Secrets。**  
工作区：`/Users/ethan/Documents/Codex/2026-08-19/ccswitch-keysmithswith-jia-github`

每次中断或恢复前必须先重读本文件。

现场备份（仓库外，未改动上一轮 WIP）：  
`/Users/ethan/Documents/Codex/2026-08-19/_backups/keysmith-switch-wip-20260819-201935`

## 1. 目标

把 Keysmith Switch 做成可安装、可更新、开箱即用的桌面 GUI：Tauri 2 + React，四个 frozen sidecar，Preview 未签名。不复制 CC Switch 的供应商/MCP/Skills/会话/用量业务。

## 2. 本轮完成（有证据）

- CC Switch 源码映射：`docs/CC_SWITCH_MAPPING.md`（MIT 改编文件已标版权头）
- 前端：CC Switch 式顶栏 + 四工具 Logo、设置横向标签、About 进入设置、已激活/未激活栏目、全屏 Markdown 编辑器、Error Boundary、首次导入对话框、数据恢复对话框、全局 Update Provider（禁止静默安装）
- 桌面生命周期：单实例、托盘（显示主窗口 / 检查更新 / 退出 Keysmith Switch）、关窗进托盘、开机启动、静默启动、窗口位置恢复
- 四个 PyInstaller sidecar（arm64 Mach-O），打进 `.app/Contents/MacOS/`
- macOS Apple Silicon **实际生成 DMG**（含 app、Applications 拖放、背景图）
- updater：确认后才下载；签名失败/离线/损坏/降级保留现版（cargo fixture）。真实 `tauri-plugin-updater` 安装路径已接线。**未**生成生产 `.sig`（缺 `TAURI_SIGNING_PRIVATE_KEY`，符合 Preview）
- 普通卸载不删除 `~/.keysmith-switch`；应用内清除全部数据需短语 + 二次确认

## 3. 未完成 / 被环境阻塞

| 项 | 状态 |
| --- | --- |
| Developer ID / 公证 | 未做。产物 **adhoc/linker-signed** |
| Authenticode | 未做 |
| Windows x64 NSIS Setup.exe | **无原生 Windows 实机**。配置与 CI 模板已写，交叉编译不算通过 |
| 公开 `keysmith-switch-releases` | 未创建 |
| 生产 updater 私钥 / GitHub Secrets | 未生成、未配置 |
| 完整页面截图（设置/About/全屏编辑/首次导入/错误恢复/浅色/1180 实窗） | 仅拿到打包应用深色主界面窗口截图；后续 AX/screencapture 对窗口截图失败 |
| 列表在打包应用截图时仍显示「加载中」 | 截图时刻 webview 尚未结束首屏加载；隔离 HOME 的 sqlite 已创建 |

## 4. 产物证据

| 文件 | 大小 | SHA-256 |
| --- | --- | --- |
| `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Keysmith Switch_0.1.0_aarch64.dmg` | 21 MB | `59fbbbc30f0d9eba5d3ff6dee32f8e10df98c5ffd5815e0611ac9fff02ef4432` |
| `.../macos/Keysmith Switch.app.tar.gz` | 21 MB | `9fe412445a72726dd4b2b658a7a016f0e0a8e3aad638a2c0e5b58cf4f8b75d5d` |

`npx tauri build` 退出码 1：**公钥在配置中，但没有 `TAURI_SIGNING_PRIVATE_KEY`，updater `.sig` 未生成。** DMG 与 `.app` 已写出。这不是签名失败冒充成功。

codesign：`flags=0x20002(adhoc,linker-signed)`，`TeamIdentifier=not set`。

### DMG 挂载内容

- `Keysmith Switch.app`
- `Applications` → `/Applications`
- `.background/dmg-background.png`

### .app 内 sidecar

`Contents/MacOS/keysmith-claude|codex|grok|zcode` 均为 `Mach-O 64-bit executable arm64`。主程序 `keysmith-switch` 12 MB。identifier `com.jia-ethan.keysmith-switch`。

## 5. 测试证据

主机：macOS 26.5.2 arm64。

| 命令 | 结果 |
| --- | --- |
| `npm test` | **35 passed** |
| `npm run build` | Vite 生产构建成功 |
| `cargo fmt --check` | 通过 |
| `cargo check --offline` | 通过 |
| `cargo test --offline` | 通过（含 data_lifecycle 6、updater_policy 15、auto_launch 3） |
| sidecar `--version`，`PATH=/bin`（无 python） | claude v7.1 / codex 0.3.8 / grok 0.4.1 / zcode 0.1.0 |
| otool sidecar | 仅 `libSystem` + `libz`，无 Python.framework |
| 隔离 HOME 启动打包应用 | 数据写入 `/tmp/ks-gui-20260819/.keysmith-switch`（21:29）。真实 `~/.keysmith-switch` mtime 仍为 17:49 |
| 上游 claude-keysmith pytest | **135 passed** |
| 上游 codex-keysmith pytest | **1008 passed, 25 skipped** |
| 上游 grok-keysmith pytest | **134 passed** |
| 上游 zcode-keysmith pytest | **11 passed, 1 failed**（`test_wrapper_logs_invocation_and_verify_reports_last_invocation`：本机真实 ZCode 0.16.3 在 wrapper `--help` 时被调用。未修改上游仓库） |

## 6. Preview 安装提示

界面标题 **Keysmith Switch Preview**，顶栏 **PREVIEW · 未签名**。macOS Gatekeeper 会警告未识别开发者。这是预期，不是正式签名版。

## 7. 非目标（仍然不做）

- git push、PR、Release、公开 release 仓库、Secrets、生产私钥
- 修改 `work/source-audit-20260819/`
- Linux 首发
- 静默安装更新

## 8. 验证命令

```bash
npm test
npm run build
cd src-tauri && cargo fmt --check && cargo check && cargo test
python3 scripts/build-sidecars.py
npx tauri build --target aarch64-apple-darwin --config src-tauri/tauri.macos.conf.json
bash scripts/verify-bundle.sh "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Keysmith Switch.app"
```
