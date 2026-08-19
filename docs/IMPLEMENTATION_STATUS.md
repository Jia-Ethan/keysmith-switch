# Keysmith Switch 实现状态

更新时间：2026-08-20 00:00 +08:00（Asia/Shanghai）

阶段：**Preview 本地验收通过；未发布、未 push。**

工作区：`/Users/ethan/Documents/Codex/2026-08-19/ccswitch-keysmithswith-jia-github`

## 已完成

- 四工具独立提示词库、Markdown 可重建索引、SQLite 元数据、历史、标签、激活状态和跨工具复制。
- Claude 支持 user/project/local；Codex、Grok Build、ZCode 不伪造项目隔离。
- 激活、停用、恢复和官方 CLI 操作均先生成预览，再由用户确认；漂移和冲突不会强制覆盖。
- 真实配置写入只通过现有 Keysmith CLI，命令参数使用 argv；GUI 不直接写 `CLAUDE.md`、`~/.codex`、`~/.grok` 或 `~/.zcode-keysmith`。
- 完整 ZIP 备份包含 SQLite 一致性快照、Markdown、manifest、schema、大小和 SHA-256；恢复前校验，失败回滚，重复恢复不追加重复数据。
- 旧版 ZIP 会先识别为追加导入，并在界面显示不同的确认语义。
- 清除全部数据需要计划、短语和二次确认；目录与数据库先暂存，暂存失败回滚，成功后保留空目录结构。
- 首次启动检查四个 sidecar；无候选文件时仍可完成首次检查。
- 关于页三层结构：应用更新、适配器状态、官方工具；更新必须用户确认，不静默安装。签名失败、离线、损坏 metadata、降级和版本漂移均保留现版。
- 官方 CLI 安装有超时、输出脱敏、运行时间反馈和用户取消；取消会终止正在运行的安装命令。
- macOS 本地包、DMG、四个 arm64 sidecar、托盘、单实例、窗口状态和 close-to-tray 生命周期已验证。
- Release workflow 已加入 updater 公钥与私钥匹配校验、macOS Developer ID/公证门槛、Windows Authenticode 门槛，以及每个平台单一产物断言。

## 本地证据

主机：macOS arm64。以下产物使用仓库内 **TEST ONLY** updater fixture 私钥，仅用于本地验证：

| 文件 | SHA-256 |
| --- | --- |
| `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Keysmith Switch_0.1.0_aarch64.dmg` | `b9402be8d529af2b7871aa9a84b7039054cb65ad9c5a02874f59dcbc09b2bd96` |
| `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Keysmith Switch.app.tar.gz` | `e95d857bad4555d5e48544aa11c2f0c9ce5c11cba6217c6cf37cf9d81e255720` |

`Keysmith Switch.app` 已通过 `codesign --verify --deep --strict` 和 `scripts/verify-bundle.sh`：主程序与四 sidecar 均为 thin arm64，identifier 为 `com.jia-ethan.keysmith-switch`，签名为 **adhoc**，无 TeamIdentifier/Authority。该结果不等于 Developer ID 或公证。

updater `.sig` 已由 fixture key 生成并通过本地 minisign 验证；fixture key 不得用于正式 release。正式构建通过 `KEYSMITH_SWITCH_UPDATER_PUBKEY` 注入与生产私钥匹配的公钥，workflow 会拒绝缺失或不匹配的签名材料。

## 测试证据

| 命令 | 结果 |
| --- | --- |
| `npm test` | 15 files / 44 tests passed |
| `npm run build` | 通过；主 JS chunk 约 898 KB，仍有性能 warning |
| `cargo fmt --check` | 通过 |
| `cargo check --offline` | 通过 |
| `cargo test --offline` | 全部通过：lib 10、adapter 4、data lifecycle 10、db prompts 3、db store 5、official 6、ops errors 5、ops home 1、ops real CLI 1、redact 1、updater 16 |
| `actionlint` | CI/release workflow 通过 |
| `python3 -m py_compile` | 脚本通过 |
| `bash -n scripts/verify-bundle.sh` | 通过 |
| 上游 claude-keysmith | 135 passed |
| 上游 codex-keysmith | 1008 passed, 25 skipped |
| 上游 grok-keysmith | 134 passed |
| 上游 zcode-keysmith | 11 passed, 1 failed；本机 ZCode 0.16.3 在 wrapper `--help` 时被调用，判定为本机环境干扰，未修改上游仓库 |

## 正式发布仍阻塞

- macOS Developer ID Application、notarization、stapling 和真实 Gatekeeper 验收。
- Windows x64 原生 NSIS 安装包、Authenticode、安装/升级/卸载验收；无 Windows 实体机时，CI 交叉编译不能算实体验收。
- 生产 updater 密钥对、私有 release 仓库和 GitHub Secrets；本轮没有生成、配置或发布它们。
- updater 生产 endpoint、生产公钥和 release artifact 的最终绑定。
- 前端主 JS chunk 约 898 KB，正式发布前可做代码分包优化。

## 明确未做

本轮没有 commit、push、创建或更新 PR/Release、创建公开 release 仓库、配置 GitHub Secrets，也没有修改 `work/source-audit-20260819/`。
