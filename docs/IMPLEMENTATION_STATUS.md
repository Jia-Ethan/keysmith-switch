# Keysmith Switch 实现状态

更新时间：2026-08-20 00:00 +08:00（Asia/Shanghai）

阶段：**Preview 本地验收通过；源码已 push，尚未发布。**

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
- macOS 旧 Preview 包只完成架构和签名检查，后续发现 hardened runtime 会阻断 PyInstaller sidecar；旧 DMG 已作废，新包已关闭 hardened runtime 并通过 sidecar 运行时 smoke。
- Release workflow 已加入 updater 公钥与私钥匹配校验、macOS Developer ID/公证门槛、Windows Authenticode 门槛，以及每个平台单一产物断言。
- 独立 unsigned Preview workflow 已准备：固定从 `main` 构建，不生成 updater artifacts，只保存明确标注未签名的 DMG/NSIS 与 SHA-256；macOS 验证包含四个 sidecar 的实际执行。

## 本地证据

主机：macOS arm64。以下产物不包含 updater artifact，也未使用 updater 私钥：

| 文件 | SHA-256 |
| --- | --- |
| `outputs/keysmith-switch-0.1.0-unsigned-preview/Keysmith Switch_0.1.0_aarch64.dmg` | `4f4726b56730755dc72309f3c8185398647d3682d731a5f43bc90f69bb47fef8` |

旧 `Keysmith Switch.app` 曾通过 `codesign --verify --deep --strict`，但该证据没有执行 sidecar。2026-08-20 复核确认旧包内四个 PyInstaller sidecar 因 `adhoc,runtime` library validation 返回 255，旧 DMG 和 updater archive 已作废。

当前 unsigned Preview 已关闭 hardened runtime 并重新构建。新 app 及从 DMG 挂载后的 app 均通过 `scripts/verify-bundle.sh`：主程序和四 sidecar 为 thin arm64，四 sidecar 的 `--version` 与隔离 HOME 预览动作实际通过，未写入托管配置；app 为 adhoc、无 TeamIdentifier/Authority。主程序在清空环境和隔离 HOME 下保持运行 6 秒并只创建自己的数据库与日志。DMG 通过 `hdiutil verify` 并包含 app、Applications 链接和品牌背景。该证据不等于 Developer ID、公证或 Gatekeeper 接受。

unsigned Preview 不生成 updater `.sig` 或 `latest.json`。仓库内 fixture key 只用于 updater 策略测试；正式构建通过 `KEYSMITH_SWITCH_UPDATER_PUBKEY` 注入与生产私钥匹配的公钥，workflow 会拒绝缺失或不匹配的签名材料。

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

尚未创建或更新 PR/Release、公开 release 仓库或 GitHub Secrets，也没有修改 `work/source-audit-20260819/`。
