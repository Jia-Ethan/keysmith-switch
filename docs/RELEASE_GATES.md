# Keysmith Switch 发布门槛

本文件记录发布前必须满足的证据。公开 release 仓库与 updater Secrets 已配置；生产私钥不得进入仓库或 Release。

应用版本：`0.1.1`

identifier：`com.jia-ethan.keysmith-switch`

## 当前平台状态

| 平台 | 目标 | 当前状态 |
| --- | --- | --- |
| macOS Apple Silicon | `.app` + `.dmg` | unsigned Preview 已重建并通过 app/DMG 内 sidecar 运行时 smoke；adhoc、关闭 hardened runtime；无 Developer ID、公证或 updater artifact |
| Windows x64 | unsigned Preview NSIS `currentUser` `.exe` | GitHub-hosted Windows runner 原生候选包已构建并校验 SHA-256；无有效 Authenticode，尚无 Windows 实体机安装、启动、升级和卸载验收 |
| Linux | 非首发目标 | 客户端显示 unsupported，不安装 |

## unsigned Preview 通道

- 私有产品仓库已发布 [`v0.1.1` Pre-release](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.1.1)，包含签名 Git tag、DMG、NSIS EXE 和 `SHA256SUMS.txt`。该 Pre-release 不是正式签名发布或 updater 发布通道。
- GitHub Actions [run 32433727011](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32433727011) 已通过 `source-gates`、`macos` 和 `windows`，上传 macOS Apple Silicon DMG 与 Windows x64 NSIS Actions artifacts。
- CI DMG SHA-256 为 `a911d2dd601d127fe0f5d478695bcbf7882b520bc9c913555509cd96ed89a96e`；CI NSIS SHA-256 为 `f37f9926266f596290e183d3248056d168af24c943732919b4a4c93020c5b461`。
- 两个平台的 Release assets 已从 GitHub 重新下载并通过 `SHA256SUMS.txt` 复核。macOS DMG 已在本机重新挂载并通过 bundle、sidecar、版本和 canonical 图标哈希验证；Windows artifact 仍不等同于实体机安装验收。

- `.github/workflows/preview-release.yml` 只允许从 `main` 手动构建。
- 版本必须与 `package.json`、`package-lock.json`、Cargo、Tauri 和 Rust 常量一致。
- macOS 使用 adhoc app 签名且关闭 hardened runtime，避免 PyInstaller sidecar 被 library validation 阻断；Windows 不要求 Authenticode，产物名称明确包含 `unsigned-preview`。
- macOS 最终 app 内四个 sidecar 都必须通过 `--version` 运行时 smoke，不能只验证文件架构和签名完整性。
- Preview 不生成 updater artifact、`.sig` 或 `latest.json`，只保存 DMG/NSIS 和 SHA-256 Actions artifacts。
- `preview-release` workflow 本身不自动创建 GitHub Release；手动发布的 unsigned Pre-release 不等同于正式发布，也不启用关于页的生产应用内更新。

## 签名门槛

### macOS

正式构建必须同时具备：

- Apple Developer ID Application 证书和 `APPLE_SIGNING_IDENTITY`
- `APPLE_CERTIFICATE`、密码、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID`
- notarization 与 stapler 验证通过
- `scripts/verify-bundle.sh --require-developer-id` 通过

本地 `src-tauri/tauri.macos.conf.json` 的 `signingIdentity: "-"` 只用于 Preview adhoc 包，不得作为正式签名证据。

### Windows

正式构建必须同时具备：

- Authenticode 证书、私钥可用的合规签名服务或硬件/云 HSM，以及时间戳服务
- NSIS Setup.exe 的 `Get-AuthenticodeSignature` 状态为 `Valid`
- Windows 原生安装、启动、更新、卸载验收

当前 workflow 的 `WINDOWS_CERTIFICATE` / `WINDOWS_CERTIFICATE_PASSWORD` PFX 接口只是尚未接入证书时的占位实现。现代公共 OV/EV 代码签名私钥通常不可导出；实际采购后应按 CA 能力改用 Tauri `signCommand`、云签名客户端，或连接 USB Token 的自托管 runner，不得为了适配现有 YAML 要求导出私钥。

### updater

- 客户端默认 fixture 公钥仅供测试；生产构建通过 `KEYSMITH_SWITCH_UPDATER_PUBKEY` 和 Tauri config 注入生产公钥。
- `TAURI_SIGNING_PRIVATE_KEY` 与非空 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 只能来自 GitHub Secrets，不得进入仓库。
- 构建后必须用同一生产公钥验证每个 `.app.tar.gz.sig` / `.exe.sig`；公钥不匹配时 workflow 必须失败。
- `latest.json` 只允许 HTTPS、非空签名、正确 SHA-256，并且每个平台恰好一个候选 artifact。

## 发布仓库与渠道

默认 endpoint：

- stable：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`
- beta：`https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/beta/latest.json`

`Jia-Ethan/keysmith-switch-releases` 是公开 updater 仓库，只保存 `latest.json`、签名 updater artifacts 和校验和。stable 与 beta 版本产物都使用不可变版本 Release；beta 的可变指针仅为受保护 `beta` 分支上的 `latest.json`，不覆盖版本 Release 资产。

私有产品仓库的 `release` workflow 只接受与应用版本一致的已签名 tag，并负责构建、平台签名、公证、updater 签名和完整门禁。公开仓库只接收该成功 workflow 的不可变 macOS/Windows 候选包，使用生产公钥再次验证后通过受保护 `production` environment 发布。Apple/Windows 证书接入前，流水线会 fail closed；Linux 验证产物不会创建 Release，也不会进入 stable 或 beta feed。

## 客户端更新策略

- 检查和安装使用同一 channel、endpoint、生产公钥和 platform key。
- 安装必须经过用户确认；不能静默安装。
- 签名失败、离线、损坏 metadata、版本降级、确认后 metadata 版本变化或下载中断时，保留当前版本。
- Linux 和未支持平台只显示不可用，不执行安装。

## 官方工具门槛

- Claude Code：npm `@anthropic-ai/claude-code`
- Codex：npm `@openai/codex`
- Grok Build：只做本机检测，不伪造 latest feed，不自动安装
- ZCode：macOS `/Applications/ZCode.app`；Windows `available/argv` 为空并显示不可用
- 所有安装/更新先显示 argv 计划，只有确认后执行；不使用 shell 拼接。

## 正式构建命令

```bash
npx tauri build --target aarch64-apple-darwin --config src-tauri/tauri.macos.conf.json
npx tauri build --target x86_64-pc-windows-msvc --config src-tauri/tauri.windows.conf.json
```

缺少 Apple/Windows 证书、生产 updater 私钥、公钥匹配证据或原生平台验收时，不得把产物标为正式发布包。
