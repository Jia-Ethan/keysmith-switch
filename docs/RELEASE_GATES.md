# Keysmith Switch 发布门槛

本文件记录 `v0.1.3` 及后续版本的发布边界。应用安装包不使用 Apple Developer ID、公证或 Windows Authenticode；应用内更新仍使用独立生产密钥签名并在客户端安装前验证。

应用版本：`0.1.3`

identifier：`com.jia-ethan.keysmith-switch`

## 当前平台状态

| 平台 | 目标 | 发布要求 |
| --- | --- | --- |
| macOS Apple Silicon | `.app`、`.dmg`、`.app.tar.gz` | ad-hoc 签名，关闭 hardened runtime；app、DMG 和 updater archive 内 app 的四个 sidecar 均通过运行时 smoke |
| Windows x64 | NSIS `currentUser` `.exe` | 无 Authenticode；构建 runner 验证状态为 `NotSigned`，并产出 updater `.sig` |
| Linux、Intel Mac、Windows ARM64 | 不发布 | 客户端显示 unsupported，不执行安装 |

## v0.1.3 bootstrap 边界

- 已公开的 `v0.1.1` 安装包内置 TEST ONLY updater 公钥，不能验证生产 updater 私钥签出的 `v0.1.3`。
- `v0.1.3` 必须手动下载安装，不能宣称可从 `v0.1.1` 直接应用内升级。
- `v0.1.3` 构建必须同时把 `TAURI_UPDATER_PUBLIC_KEY` 写入 Tauri updater config，并通过 `KEYSMITH_SWITCH_UPDATER_PUBKEY` 编译进 Rust 自定义检查逻辑。
- 构建前必须验证 Tauri release config 使用生产 updater 公钥，并通过 Rust 编译环境注入；Windows 构建后继续扫描最终 PE，macOS 通过生产公钥验签 updater payload 和三份 bundle smoke 验证发布闭环；不得使用仓库 fixture 私钥签生产产物。
- 可靠的应用内更新承诺从 `v0.1.3 → v0.1.4` 开始。

## updater 签名门槛

- `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`、`TAURI_UPDATER_PUBLIC_KEY` 只来自源仓库 GitHub Secrets。
- source gate 先用生产私钥签临时文件，再使用生产公钥验证，证明密钥配对正确。
- macOS `.app.tar.gz` 和 Windows NSIS `.exe` 必须由 Tauri 生成非空 `.sig`。
- 每个平台 payload 必须用同一生产公钥通过独立 `verify_updater` 验证。
- `latest.json` 只允许 HTTPS、非空签名、正确 SHA-256，并且仅包含 `darwin-aarch64` 与 `windows-x86_64`。
- 签名失败、离线、损坏 metadata、版本降级、确认后 metadata 变化或下载中断时，客户端保留当前版本。

## macOS 门槛

- `src-tauri/tauri.macos.conf.json` 使用 `signingIdentity: "-"` 和 `hardenedRuntime: false`。
- 最终 `.app` 必须通过 `codesign --verify --deep --strict`，且为 `Signature=adhoc`、`TeamIdentifier=not set`、无 Authority。
- 最终 `.app` 和 updater `.app.tar.gz` 解包后的 app 都运行 `scripts/verify-bundle.sh`。
- 四个 sidecar 都必须通过架构、`--version` 和隔离 HOME 预览 smoke，不能只验证文件存在。
- DMG 必须通过 `hdiutil verify`。

## Windows 门槛

- 构建必须生成唯一 NSIS `.exe` 和对应 `.exe.sig`。
- `Get-AuthenticodeSignature` 必须返回 `NotSigned`；如果意外出现其他状态，workflow 失败。
- updater 公钥必须能验证 NSIS payload；Tauri plugin 和 Rust 自定义检查逻辑必须使用同一生产公钥，最终主程序必须包含该公钥。
- `v0.1.3` 发布后仍需在 Windows x64 实体机验证手动安装、启动和卸载。
- 在承诺 `v0.1.3 → v0.1.4` 自动更新前，必须验证应用内下载、SmartScreen/系统拦截行为、安装、重启和失败保留现版。

## 发布仓库与渠道

- stable：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`
- beta：`https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/beta/latest.json`
- 不可变版本资产：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/vX.Y.Z`

源仓库的 `release` workflow 只接受与应用版本一致的 GitHub-verified annotated tag，构建 updater-signed 候选并上传短期 Actions artifact。公开 `Jia-Ethan/keysmith-switch-releases` 仓库重新验证 tag/commit provenance、平台白名单、minisign 和 SHA-256，再经受保护 `production` environment 发布不可变 Release。

公开仓库不得保存生产 updater 私钥、源代码、Linux 产物或不受支持平台资产。公开 workflow 应绑定源 run 的仓库、workflow path、成功状态、tag、commit 和 artifact provenance。

## 客户端更新策略

- 检查和安装使用同一 channel、endpoint、生产公钥和 platform key。
- 安装必须经过用户确认；不能静默安装。
- 用户界面不展示 Preview、平台签名、Developer ID、公证或 Authenticode 说明。
- 发布文档必须准确说明 bootstrap 和系统警告边界，不得把 updater minisign 描述成平台代码签名。

## 发布顺序

1. 版本、发布说明和 workflow 进入 `main`，CI 全部通过。
2. 创建 GitHub-verified annotated tag `v0.1.3`。
3. 手动触发源仓库 `release` workflow，参数为 `source_tag=v0.1.3`、`channel=stable`。
4. 独立下载并验证 source candidate artifact、provenance、payload、签名和 SHA-256。
5. 手动触发公开仓库 publish workflow，经 `production` 审批后发布。
6. 从公开 Release 重新下载全部资产，复核 SHA-256、`latest.json` 和可公开访问性。
7. 手动安装 `v0.1.3` 完成 bootstrap 验收；不从 `v0.1.1` 测试自动更新。

创建 tag、触发 workflow、批准 production 或创建公开 Release 均属于外部发布动作，需要当次明确确认。
