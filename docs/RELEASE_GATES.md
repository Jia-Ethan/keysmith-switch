# Keysmith Switch 发布门槛

本文件只记录发布前必须满足的证据，不创建公开 release 仓库，不配置 GitHub Secrets，不提交任何生产私钥。

应用版本：`0.1.0`

identifier：`com.jia-ethan.keysmith-switch`

## 当前平台状态

| 平台 | 目标 | 当前状态 |
| --- | --- | --- |
| macOS Apple Silicon | `.app` + `.dmg` | unsigned Preview 已重建并通过 app/DMG 内 sidecar 运行时 smoke；adhoc、关闭 hardened runtime；无 Developer ID、公证或 updater artifact |
| Windows x64 | unsigned Preview NSIS `currentUser` `.exe` | GitHub-hosted Windows runner 原生候选包已构建并校验 SHA-256；无有效 Authenticode，尚无 Windows 实体机安装、启动、升级和卸载验收 |
| Linux | 非首发目标 | 客户端显示 unsupported，不安装 |

## unsigned Preview 通道

- 私有产品仓库已发布 [`v0.1.0` Pre-release](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.1.0)，包含签名 Git tag、DMG、NSIS EXE 和 `SHA256SUMS.txt`。该 Pre-release 不是正式签名发布或 updater 发布通道。
- GitHub Actions [run 32323351213](https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32323351213) 已通过 `source-gates`、`macos` 和 `windows`，上传 macOS Apple Silicon DMG 与 Windows x64 NSIS Actions artifacts。
- CI DMG SHA-256 为 `9b429800d3ce55f3d71ac37e84258a5ded2677910232033f19cb147d50f79786`；CI NSIS SHA-256 为 `692d6796891c795116feb54f6f6c3d09372322efa93e58f7102275026bb33e6f`。
- macOS Actions artifact 已下载并在本机重新挂载验证；Windows artifact 仅有 Windows runner 构建、无有效 Authenticode 断言和校验和，不等同于实体机验收。

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

- Authenticode PFX、密码、时间戳服务
- NSIS Setup.exe 的 `Get-AuthenticodeSignature` 状态为 `Valid`
- Windows 原生安装、启动、更新、卸载验收

### updater

- 客户端默认 fixture 公钥仅供测试；生产构建通过 `KEYSMITH_SWITCH_UPDATER_PUBKEY` 和 Tauri config 注入生产公钥。
- `TAURI_SIGNING_PRIVATE_KEY` 只能来自 GitHub Secrets，不得进入仓库。
- 构建后必须用同一生产公钥验证每个 `.app.tar.gz.sig` / `.exe.sig`；公钥不匹配时 workflow 必须失败。
- `latest.json` 只允许 HTTPS、非空签名、正确 SHA-256，并且每个平台恰好一个候选 artifact。

## 发布仓库与渠道

默认 endpoint：

- stable：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`
- beta：`https://github.com/Jia-Ethan/keysmith-switch-releases/releases/download/beta-latest/latest.json`

`Jia-Ethan/keysmith-switch-releases` 尚未创建。本轮不创建、不 push、不发 Release。

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
