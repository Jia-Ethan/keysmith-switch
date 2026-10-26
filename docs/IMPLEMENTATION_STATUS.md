# Keysmith Switch 实现状态

更新日期：2026-10-26（Asia/Shanghai）

阶段：公开稳定版为 [`v0.8.4`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.8.4)，含 macOS Apple Silicon 与 Windows x64 安装包。此文下方 `v0.1.3`／`v0.1.4` 的验证记录是历史记录。

## v0.8.4 发布记录

- [#157](https://github.com/Jia-Ethan/keysmith-switch/pull/157) 已合并（[#155](https://github.com/Jia-Ethan/keysmith-switch/issues/155)）：未设置 `https_proxy` / `all_proxy` 时，macOS 从 `scutil --proxy` 读取系统代理（HTTPS → HTTP → SOCKS）传给 curl；updater 插件的检查和下载在 macOS / Windows 上使用系统 HTTP 代理。回环 endpoint 不走代理。
- 随包 dsh-keysmith 固定到上游 commit `c15f990`（0.4.0，[#156](https://github.com/Jia-Ethan/keysmith-switch/pull/156)）。部署面是 `profiles/web` 的 Cordis 插件，不再 dump `AGENTS.md`。0.4.0 读到 0.2.2 的 manifest 时报 `drift`，停用只撤插件，不还原旧版写下的 home `AGENTS.md` / `cordis.patch.yml`；发布说明提示需要还原的用户先在旧版停用。
- 2026-10-26，发布准备 PR [#158](https://github.com/Jia-Ethan/keysmith-switch/pull/158) 合并为 `271e3edb70ca168990a6171675b4c1cafb438efb`，main CI run [`40221034839`](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/40221034839) 五项检查通过。GitHub-verified annotated tag [`v0.8.4`](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.8.4) 指向该提交。
- Source release run [`40221809784`](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/40221809784) 的 source-gates、macOS、Windows 和 metadata 均成功；公开发布 run [`40224050469`](https://github.com/Jia-Ethan/keysmith-switch-releases/actions/runs/40224050469) 经 `production` 审批后成功。
- 独立下载验证了候选的 8 个发布文件、provenance、SHA-256、`latest.json`（size / sha256 / 签名与文件一致，updater 门槛 0.1.3）和签名 key ID `0D03A636FF048B9F`（与 v0.8.3 相同）。macOS updater 内 sidecar：DSH 0.4.0、ZCode 0.3.4，其余与 v0.8.3 相同；DMG 内 app 与 updater archive 逐文件一致；Windows installer 含 8 个 `keysmith-*.exe`。随包 DSH sidecar 在隔离 home 部署为 `active`，未写 home `AGENTS.md`。
- 公开 Release 为 stable、Latest、非 draft、immutable，8 个公开资产与候选一致，发布说明与 `scripts/release-notes.md` 一致；匿名 stable feed 与候选 `latest.json` 字节一致。
- 开着系统代理的 macOS 上 v0.8.3 → v0.8.4 应用内升级（v0.8.3 本身仍有 #155 的问题，可能需要 TUN 模式或手动下载）、Windows 系统代理下的插件下载，以及 DSH 0.2.2 → 0.4.0 实体机迁移仍需独立验收。

## v0.8.3 发布记录

- 随包 ZCode CLI 固定到上游 commit `3c3bea8f994d3670c2cd571e9bd87449fc868788`（0.3.4，PR #44）。Windows 含 `supportsStorageStartup` 的 ZCode 3.16.x 使用 runtime-patch，不再依赖被忽略的 agent-server 环境覆盖；保留旧版 wrapper 路径。
- Python Windows fixture 覆盖部署、原始 runtime 备份、无 `ZCODE_AGENT_SERVER_COMMAND` 环境覆盖、停用还原和用户环境恢复。
- 2026-10-24，PR [#154](https://github.com/Jia-Ethan/keysmith-switch/pull/154) 合并为 `346e0a96894117c35de8fe31225e32b51ae87609`，main CI run [`40092753978`](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/40092753978) 五项检查通过。GitHub-verified annotated tag [`v0.8.3`](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.8.3) 指向该提交。
- Source release run [`40093446182`](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/40093446182) 的 source-gates、macOS、Windows 和 metadata 均成功；公开发布 run [`40095719745`](https://github.com/Jia-Ethan/keysmith-switch-releases/actions/runs/40095719745) 经 `production` 审批后成功。
- 独立下载验证了候选的 8 个发布文件、provenance、metadata、SHA-256、macOS DMG 和 updater 内全部 sidecar，ZCode 为 0.3.4。公开 Release 为 stable、Latest、非 draft、immutable，stable feed 与候选 `latest.json` 一致。
- Windows ZCode 3.16.x 实体机部署/停用，以及 v0.8.2 → v0.8.3 真实应用内升级仍需独立验收。

## 产品状态

- 首页是每个 Agent 的工作区：顶部显示部署状态与当前生效的提示词（活动记录缺失时按内容指纹在提示词库中匹配，仍无法识别则标注“来源未记录”），下方是提示词库，库中任一提示词可一键生成部署计划。所有部署都是全局（user scope）安装，界面不再提供范围选择；应用也不再提供键盘快捷键。Quick Deploy 以弹出面板打开，可粘贴、导入或拖入 `.md` 文件，将正文保存到本地库，预览计划后确认部署；默认不抓取兄弟仓库的远程提示词。检查更新、移除部署和部署按钮在请求进行中显示加载状态，设置保存以标题旁的轻量提示确认。当前仍由六个随包提供的 legacy adapter 执行写入；Agent registry 与外部 adapter 尚未实现。
- 六个工具（Claude Code、Codex、Grok Build、ZCode、pi、DSH）各自独立的提示词库、Markdown 可重建索引、SQLite 元数据、历史、标签、激活状态和跨工具复制已实现。
- 激活、停用、恢复、清除数据和官方 CLI 操作保留计划、确认、执行、失败保留与恢复门禁。
- 提示词详情与编辑使用独立页面，保留脏状态保护、键盘与无障碍行为。
- 设置页区分 loading、empty、retryable error 和 busy 状态。
- 关于页在应用内更新可用时显示“更新并重启”；低于 updater 门槛或签名 key ID 不匹配时只显示本地化的官方下载入口。
- 更新安装必须显式确认；安装期间阻止关闭和重复提交，并显示下载/安装进度。
- 六个 Keysmith sidecar 仍随应用原子交付（pi 自 [#143](https://github.com/Jia-Ethan/keysmith-switch/issues/143) 起接入，pi-keysmith 0.2.0；DSH 对应 [#148](https://github.com/Jia-Ethan/keysmith-switch/issues/148)，随包 dsh-keysmith 0.4.0，上游 [#6](https://github.com/Jia-Ethan/dsh-keysmith/pull/6)），GUI 不直接改写托管配置。
- 当前界面基线使用 16px 正文、15px 主要控件与编辑器、分层圆角；窄窗口导航通过可点击的 More 菜单收纳，macOS 菜单栏使用单色 template icon，窗口内品牌图标仍保留颜色。

## v0.8.1 发布记录

- GitHub-verified annotated tag [`v0.8.1`](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.8.1) 指向源码 commit `39d37dc39d20d6d34e2c35684c2a7c66c95eb519`；source release run [`39435809911`](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/39435809911) 的 source-gates、Windows 和 macOS 均成功。
- 公开发布 run [`39439791542`](https://github.com/Jia-Ethan/keysmith-switch-releases/actions/runs/39439791542) 在 `production` 审批后成功；公开 Release 为 stable、非 draft、immutable。
- 8 个公开资产（DMG、updater archive、Windows installer、两份签名、`latest.json`、provenance 和 `SHA256SUMS.txt`）已重新下载并通过 checksum、生产 updater 签名和公开 feed 校验。
- 这版新增 DSH（DeepSeek Harness）；发布校验脚本补齐了 DSH verifier sidecar 占位（发布仓库 commit `835db5f`）。

## v0.1.3 bootstrap

已发布的 `v0.1.1` DMG 内嵌 TEST ONLY updater 公钥，无法验证生产 updater 私钥签出的 `v0.1.3`。本机已完成该 bootstrap，不再运行 `v0.1.1`；因此：

- `v0.1.3` 必须手动下载安装。
- `v0.1.3` 构建同时通过 Tauri config 和 Rust 编译环境注入生产 updater 公钥。
- 构建 workflow 扫描 Windows 主程序；macOS 通过编译注入、生产公钥验签 updater payload 和三份 bundle smoke 防止再次产出 fixture-key 客户端。
- 从 `v0.1.3 → v0.1.4` 开始，才能对应用内更新作真实承诺。
- fixture 私钥只用于测试，禁止用于生产 Release。

## v0.1.4-rc.1 updater 兼容协议

- `latest.json` 新增 `minimum_updater_version: "0.1.3"` 与每个平台的准确 payload `size`。
- 新客户端用 `installMode` 和 `reason` 区分应用内安装、手动 bootstrap 与签名密钥轮换，不向界面返回底层验签英文。
- 客户端低于门槛时不会请求 updater payload；安装命令重复执行同一门禁，避免绕过前端。
- metadata 缺少新字段时保留旧 feed 行为；大小缺失时才使用跟随重定向且忽略零值的 HEAD fallback。
- 本候选只验证双平台构建、生产密钥兼容、metadata、签名、大小和发布链；真实应用内安装与重启仍是独立验收项。

## v0.1.4-rc.1 候选验证

- 公开 updater 仓库 PR [#1](https://github.com/Jia-Ethan/keysmith-switch-releases/pull/1) 与源仓库 PR [#5](https://github.com/Jia-Ethan/keysmith-switch/pull/5) 已按顺序合并。
- GitHub-verified signed annotated tag `v0.1.4-rc.1` 指向源码 commit `6019cb989a52ba2bc2535821d8719c137fb49720`。
- source release run [32694603125](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32694603125) 的 source-gates、macOS、Windows 与 metadata job 均成功。
- public validation run [32695888630](https://github.com/Jia-Ethan/keysmith-switch-releases/actions/runs/32695888630) 的 `validate` job 成功；`publish` 未获 `production` 批准并已取消。
- 最终确认没有 `v0.1.4-rc.1` Release、公开资产或 beta/stable feed 更新；Issue [#4](https://github.com/Jia-Ethan/keysmith-switch/issues/4) 保持 OPEN 且未评论。

候选 tag `v0.1.4-rc.1`（`6019cb9`）的源码本地验证为：前端 19 files / 87 tests、updater policy 27 tests、Rust 全套测试、Python 3 tests、构建、七处版本一致性、三语言 279 个叶子键、`actionlint` 与 `git diff --check` 全部通过。

## 发布架构

- 安装包不使用 Apple Developer ID、公证或 Windows Authenticode。
- updater payload 仍使用生产 minisign 密钥签名；客户端和公开发布仓库均独立验证。
- macOS 使用 ad-hoc app、关闭 hardened runtime，避免 PyInstaller sidecar 被 library validation 阻断。
- Windows 产出无 Authenticode 的 NSIS per-user installer；系统 SmartScreen 行为仍需实体机验收。
- 源仓库从 GitHub-verified annotated tag 构建候选和 provenance。
- 公开 `Jia-Ethan/keysmith-switch-releases` 仓库只接收受支持平台的 updater payload、签名、`latest.json`、provenance 和校验和，并经 `production` 审批发布。

## v0.1.3 已完成验证

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

- Windows x64 实体机完成 `v0.8.1` 手动安装、启动和卸载验收。
- `v0.7.0 → v0.8.1` 的真实应用内下载、安装与重启验收；源码和公开发布链验证不能代替该项。

## 明确未做

- 尚未完成 `v0.8.1` Windows 实体机验收。
- 尚未验证 `v0.7.0 → v0.8.1` 的真实应用内升级与重启。

## 本机 bootstrap 验收

2026-08-24 已将 `/Applications/Keysmith Switch.app` 从 `v0.1.1` 手动升级至官方 immutable stable Release `v0.1.3`：DMG SHA-256 与 Release digest 一致，arm64 架构和 ad-hoc codesign 结构验证通过，替换后应用成功重启且无新增崩溃报告。旧版回滚副本保存在 `~/Library/Application Support/Keysmith Switch/Upgrade Backups/`；用户数据目录未改动。

## 本机源码 Preview

2026-08-24，界面可读性与 macOS 菜单栏图标优化已提交为 `bd6bf7f57333de34c5570b61f882d73a6336eb4e` 并推送至 `origin/main`。本机 `/Applications/Keysmith Switch.app` 随后从稳定版 `v0.1.3` 替换为该提交构建的 `v0.1.4-rc.1` Preview：

- 前端 19 files / 92 tests、生产构建、`cargo check --offline` 与 `git diff --check` 通过。
- 最终 bundle 为 arm64、ad-hoc 签名；四个 sidecar 的版本检查、隔离 preview smoke、`codesign --verify --deep --strict` 和数据库 `quick_check` 通过，应用成功启动且无新增崩溃报告。
- `v0.1.3` 应用和升级前数据库已保存在 `~/Library/Application Support/Keysmith Switch/Upgrade Backups/`，用于本机回滚。
- 这次替换不是 `v0.1.3 → v0.1.4-rc.1` 的应用内 updater 验收，也没有创建 Release、发布 RC 资产或更新 beta/stable feed。当时公开稳定版仍是 `v0.1.3`。2026-09-22 起公开稳定版是 `v0.1.4`。
