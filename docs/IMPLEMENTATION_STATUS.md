# Keysmith Switch 实现状态

更新日期：2026-08-24（Asia/Shanghai）

阶段：**`v0.1.3` updater bootstrap 发布中。`v0.1.2` 的已删除 immutable Release 占用了 Tag 名，因此未作为可用版本发布。**

工作区：`/Users/ethan/Documents/Codex/2026-08-19/ccswitch-keysmithswith-jia-github`

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

`cdad6b69e9e4d697e9b6b49c921809421cf41e7e` 已推送至 `main`，CI run [32629636009](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32629636009) 的 frontend、rust、sidecar-macos 和 sidecar-windows 均通过。

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

公开 updater 仓库的 `publish.yml` 同时通过 `actionlint`、YAML 解析和 `git diff --check`。这些结果覆盖源码与 workflow 静态门槛；平台候选构建和 updater 签名仍需在创建 `v0.1.3` tag 后由 release workflow 验证。

## 发布前剩余门槛

- `v0.1.3` 源码和 workflow 变更进入 `main`，远端 CI 全绿。
- 创建 GitHub-verified annotated tag `v0.1.3`。
- `release` workflow 成功生成唯一 macOS/Windows updater 候选、签名、metadata 和 provenance。
- macOS app、DMG 与 updater archive 内 app 通过 sidecar runtime smoke 和 ad-hoc 验证。
- Windows runner 验证 NSIS 为 `NotSigned`、updater minisign 有效、生产公钥已编译进主程序。
- Windows x64 实体机完成 `v0.1.3` 手动安装、启动和卸载验收。
- 公开 publish workflow 经 `production` 审批发布，外部重新下载并复核所有 SHA-256 与 manifest。
- 发布后手动安装 `v0.1.3` 验证 bootstrap；应用内升级验收留到 `v0.1.4` 候选。

## 明确未做

- 尚未创建或推送 `v0.1.3` tag。
- 尚未触发 `v0.1.3` release workflow 或公开 publish workflow。
- 尚未批准 production environment 或创建公开 updater Release。
- 尚未替换 `/Applications/Keysmith Switch.app`。
- 尚未完成 Windows 实体机验收。
