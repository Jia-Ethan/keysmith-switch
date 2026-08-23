# Keysmith Switch 实现状态

更新日期：2026-08-24（Asia/Shanghai）

阶段：**`v0.1.3` updater bootstrap 已发布。`v0.1.2` 的已删除 immutable Release 占用了 Tag 名，因此未作为可用版本发布。**

## 产品状态

- 四工具独立提示词库、Markdown 可重建索引、SQLite 元数据、历史、标签、激活状态和跨工具复制已实现。
- 激活、停用、恢复、清除数据和官方 CLI 操作保留计划、确认、执行、失败保留与恢复门禁。
- 提示词详情与编辑使用独立页面，保留脏状态保护、键盘与无障碍行为。
- 设置页区分 loading、empty、retryable error 和 busy 状态。
- 关于页保留“检查更新”和“更新并重启”，不展示 Preview、平台签名、Developer ID、公证或 Authenticode 说明。
- 更新安装必须显式确认；安装期间阻止关闭和重复提交，并显示下载/安装进度。
- 四个 Keysmith sidecar 仍随应用原子交付，GUI 不直接改写托管配置。

## v0.1.3 bootstrap

已发布的 `v0.1.1` DMG 和本机 `/Applications/Keysmith Switch.app` 都确认内嵌 TEST ONLY updater 公钥。它们无法验证生产 updater 私钥签出的 `v0.1.3`，因此：

- `v0.1.3` 必须手动下载安装。
- `v0.1.3` 构建同时通过 Tauri config 和 Rust 编译环境注入生产 updater 公钥。
- 构建 workflow 扫描 Windows 主程序；macOS 通过编译注入、生产公钥验签 updater payload 和三份 bundle smoke 防止再次产出 fixture-key 客户端。
- 从 `v0.1.3 → v0.1.4` 开始，才能对应用内更新作真实承诺。
- fixture 私钥只用于测试，禁止用于生产 Release。

## 发布架构

- 安装包不使用 Apple Developer ID、公证或 Windows Authenticode。
- updater payload 仍使用生产 minisign 密钥签名；客户端和公开发布仓库均独立验证。
- macOS 使用 ad-hoc app、关闭 hardened runtime，避免 PyInstaller sidecar 被 library validation 阻断。
- Windows 产出无 Authenticode 的 NSIS per-user installer；系统 SmartScreen 行为仍需实体机验收。
- 源仓库从 GitHub-verified annotated tag 构建候选和 provenance。
- 公开 `Jia-Ethan/keysmith-switch-releases` 仓库只接收受支持平台的 updater payload、签名、`latest.json`、provenance 和校验和，并经 `production` 审批发布。

## 当前验证

发布源码 commit `0ade54c2f0c36dd21624a808d14392055ec3b03b` 已推送至 `main`。CI run [32650939449](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32650939449) 的 frontend、rust、sidecar-macos 和 sidecar-windows 均通过。

`v0.1.3` 发布准备的本地验证：

| 命令 | 结果 |
| --- | --- |
| `npm test` | 19 files / 81 tests passed |
| `npm run build` | 通过；主 JS chunk 约 907 KB，保留既存性能 warning |
| `cargo fmt --check` | 通过 |
| `cargo check` | 通过 |
| `cargo test` | 全部通过：lib 10、adapter 4、data lifecycle 10、db prompts 3、db store 5、official 6、ops errors 5、ops home 1、ops real CLI 1、redact 1、updater 16 |
| `python3 scripts/check-version.py --expected 0.1.3` | 七处版本一致 |
| `python3 -m py_compile scripts/*.py` | 通过 |
| `bash -n scripts/verify-bundle.sh` | 通过 |
| `actionlint .github/workflows/*.yml` | 通过 |
| `git diff --check` | 通过 |

GitHub-verified annotated tag `v0.1.3` 指向发布源码 commit。source release run [32651146623](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32651146623) 和 public publish run [32652433093](https://github.com/Jia-Ethan/keysmith-switch-releases/actions/runs/32652433093) 均成功。公开 Release 为 stable、Latest、immutable；八个资产、SHA-256、provenance、feed 和两个 updater payload URL 已重新下载验证。

## 发布后剩余验收

- Windows x64 实体机完成 `v0.1.3` 手动安装、启动和卸载验收。
- 发布后手动安装 `v0.1.3` 验证 bootstrap；应用内升级验收留到 `v0.1.4` 候选。

## 明确未做

- 尚未替换 `/Applications/Keysmith Switch.app`。
- 尚未完成 Windows 实体机验收。
- 尚未验证 `v0.1.3 → v0.1.4` 的真实应用内升级。
