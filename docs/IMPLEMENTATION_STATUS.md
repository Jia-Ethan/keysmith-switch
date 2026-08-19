# Keysmith Switch — 实现状态

更新时间：2026-08-19 17:37 +08:00（Asia/Hong_Kong）  
阶段：**本地实现完成，已 commit ，已停止（不 push）**  
工作区：`/Users/ethan/Documents/Codex/2026-08-19/ccswitch-keysmithswith-jia-github`  
GitHub：`Jia-Ethan/keysmith-switch`（Private）

每次中断或恢复前必须先重读本文件。

## 1. 目标

在本地实现 **Keysmith Switch**：Tauri 2 + React + Rust 桌面工具，用统一 adapter 调用四个既有 Keysmith CLI，管理 Claude Code / Codex / Grok Build 与 ZCode 的提示词库、预览激活、受控停用、历史与恢复。应用更新走独立公开 release 仓库的 metadata；**本次不创建该仓库、不 push、不建 PR、不建 Release、不配置 GitHub Secrets**。

## 2. 范围（本次交付）

- 主界面四工具导航 + 独立提示词库 + 跨工具复制
- Claude `user/project/local`；其余工具只展示 CLI 真实支持的 scope
- 激活必须 `plan-*` 预览 → 用户确认 → `activate` / `deactivate`
- SQLite 索引/历史 + `~/.keysmith-switch/` Markdown 可重建
- 关于页三层：应用更新 / 内置 adapter CLI / 官方产品
- Tauri updater 配置 + 本地 fixture 闭环
- macOS Apple Silicon 本机 `.app` 打包（ad-hoc，非正式签名）
- Windows x64 以 CI/fixture 验证并如实报告：本机无 Windows
- React 单测与构建；Rust fmt/check/test；adapter contract 与 HOME 隔离集成测试
- 四个上游 Keysmith 测试作为 contract 回归（不修改上游仓库）
- 本地 commit 后停止

## 3. 非目标（明确不做）

- `git push`、建立或更新 PR
- 创建或发布 `keysmith-switch-releases`
- 配置 GitHub Secrets / 提交 updater 生产私钥
- 伪造 Developer ID / notarization / Authenticode 签名结果
- GUI 直接改写 `CLAUDE.md`、`~/.codex/config.toml`、hooks、`~/.grok/rules`、`~/.zcode-keysmith`、LaunchAgent
- Linux 首发桌面目标
- 静默安装应用更新
- 内置 Keysmith CLI 独立热更新
- 上传用户提示词
- 修改 `work/source-audit-20260819/` 内四个上游仓库

## 4. 已确认产品决策

| ID | 决策 | 落地 |
| --- | --- | --- |
| 1A | 每工具独立提示词库，支持跨工具复制 | `ops::copy_prompt` + UI 跨工具复制 |
| 2A | Claude Code：user / project / local | adapter argv + UI scope |
| 3A | 激活必须先预览再确认 | `plan_activate` 无 `--yes`；`activate` 需已存 preview |
| 4A | 停用受控卸载/恢复，保留用户未管理修改 | drift 时 deactivate 拒绝并 recovery-required |
| 5A | 内置历史、diff、版本恢复 | `prompt_versions` + unified diff |
| 6A | macOS AS 完整；Windows Claude/Codex/Grok；ZCode Windows 不可用 | ZCode `available: false` |
| 7A | 官方安装/更新展示来源、命令、目标位置，确认后执行 | `official.rs` + 关于页 |
| 8A | SQLite + Markdown 可重建 | `Store::rebuild_from_markdown` |
| 9A | 产品名 Keysmith Switch | tauri.conf / UI |
| 10A | 场景评估、Grok Run、Breaktest 隐藏 | Settings `advancedToolsEnabled` |
| 11A | 更新 metadata 在独立公开仓 | 仅配置 endpoint，未创建仓库 |
| 12A | 启动检查更新，用户点击才安装 | AboutPage 延迟检查 + 确认勾选 |
| 13A | 默认 stable，可加入 beta | Settings `updateChannel` |
| 14A | updater 签名强制；正式签名是门槛 | fixture 签名测试通过；正式凭证缺失 |
| 15A | 应用与四个内置 CLI 原子更新 | `third_party/keysmith/` 钉选 |

## 5. 审计基线（只读，不修改）

| 项目 | 路径 | HEAD | 版本 |
| --- | --- | --- | --- |
| CC Switch | `work/source-audit-20260819/cc-switch` | `0b5da510` | IA/更新体验参考 |
| Claude Keysmith | `work/source-audit-20260819/claude-keysmith` | `3fe8902d` | v7.1 |
| Codex Keysmith | `work/source-audit-20260819/codex-keysmith` | `ae068de1` | v0.3.8 |
| Grok Keysmith | `work/source-audit-20260819/grok-keysmith` | `1f49c54a` | v0.4.1 |
| ZCode Keysmith | `work/source-audit-20260819/zcode-keysmith` | `77a27dec` | v0.1.0 |

## 6. 当前进度

- [x] 建立本状态文件与 command 契约
- [x] 钉选四个 Keysmith CLI 到 `third_party/keysmith/`
- [x] Rust 数据层 / adapter / 事务
- [x] React 主界面 / 设置 / 关于 / 隐藏 Advanced Tools
- [x] updater + 官方工具 + 打包配置
- [x] Tauri command 接线
- [x] 测试与本机验证
- [x] 本地 commit （不 push）

## 7. 验证证据

主机：macOS 26.5.2，`arm64`，Node v25.9.0，Python 3.9.6。

| 命令 | 结果 |
| --- | --- |
| `npm test` | 15/15 通过 |
| `npm run build` | Vite 生产构建成功 |
| `cargo fmt --check` | 通过 |
| `cargo check --offline` | 通过 |
| `cargo test --offline` | 51 通过（lib 6 + integration 45） |
| 上游 claude-keysmith pytest | **135 passed** |
| 上游 codex-keysmith pytest | **1008 passed, 25 skipped** |
| 上游 grok-keysmith pytest | **134 passed** |
| 上游 zcode-keysmith pytest | **11 passed, 1 failed**（见下） |
| macOS AS 打包 | `.app` 已生成，identifier `com.jia-ethan.keysmith-switch`，Mach-O arm64，**adhoc/linker-signed**。updater `.tar.gz` 已生成，**无生产 `.sig`**（缺 `TAURI_SIGNING_PRIVATE_KEY`） |
| Windows x64 | **无实体机**。official/updater fixture 覆盖；交叉编译未当作实体验收 |

ZCode 上游失败项：`test_wrapper_logs_invocation_and_verify_reports_last_invocation`。本机真实 ZCode 0.16.3 在 wrapper `--help` 时被调用，fixture 期望 `"node"` 子串。未修改上游仓库。Keysmith Switch 的 ZCode adapter 仍走 argv 调用钉选 CLI。

密钥扫描：产品源码与测试中的 `sk-` 仅出现在脱敏正则和 redact fixture 输入；测试断言输出不含原文。未提交生产 updater 私钥。fixture 私钥被 `.gitignore` 排除。

## 8. 关键路径

- 前端：`src/`
- Tauri command：`src-tauri/src/commands.rs` `src-tauri/src/lib.rs`
- 数据/adapter：`src-tauri/src/db/` `src-tauri/src/adapter/` `src-tauri/src/ops.rs`
- updater/官方：`src-tauri/src/updater.rs` `src-tauri/src/official.rs`
- 钉选 CLI：`third_party/keysmith/`
- 发布门槛：`docs/RELEASE_GATES.md`

## 9. 发布门槛（未执行）

详见 `docs/RELEASE_GATES.md`：

- 无 Developer ID / 公证
- 无 Authenticode
- 公开仓库 `Jia-Ethan/keysmith-switch-releases` 未创建
- `tauri.conf.json` 公钥为 TEST ONLY fixture，正式构建必须轮换
- 生产 updater 私钥只能进 GitHub Secrets；本次不配置
- 本机 `.app` 为 ad-hoc，不是正式签名包
- Windows 无实体机验收

## 10. 需要桥确认的外部动作（commit 后等待）

1. 是否 push 到 `Jia-Ethan/keysmith-switch`
2. 是否创建公开 `keysmith-switch-releases`
3. 是否配置 updater Secrets 与正式签名
4. 是否建立 PR / Release / 安装包

## 11. 验证命令（复现）

```bash
npm test
npm run build
cd src-tauri && cargo fmt --check && cargo check && cargo test
bash scripts/run-upstream-regression.sh
npx tauri build --target aarch64-apple-darwin --bundles app
```
