# Keysmith Switch 发布门槛

本次只记录门槛，不创建公开 release 仓库，不配置 GitHub Secrets，不提交生产私钥，不把本机 ad-hoc 签名写成“已正式签名”。

应用版本 `0.1.0`，identifier `com.jia-ethan.keysmith-switch`。

## 打包目标

| 平台 | 目标三元组 | 产物 | 状态 |
| --- | --- | --- | --- |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.app` + `.dmg` + updater `.tar.gz` / `.sig` | **2026-08-19 本机已产出 .app 与 .dmg**（21 MB，SHA-256 见 IMPLEMENTATION_STATUS）。adhoc/linker-signed。**无** Developer ID / 公证。updater `.tar.gz` 有，**无生产 `.sig`**（缺 `TAURI_SIGNING_PRIVATE_KEY`，tauri build 因此以退出码 1 结束，但 DMG 已写出） |
| Windows x64 | `x86_64-pc-windows-msvc` | NSIS `currentUser` + updater `.exe.sig` | 配置已写入；无实体机时交叉编译不算实体验收 |
| Linux | `linux-x86_64` / `linux-aarch64` | 非首发桌面目标 | updater 平台表包含 linux 键，值为 **unsupported**，不安装 |

配置文件：

- `src-tauri/tauri.conf.json` — 默认 stable endpoint、fixture 公钥、`createUpdaterArtifacts: true`
- `src-tauri/tauri.macos.conf.json` — `targets: ["app", "dmg"]`，`hardenedRuntime: true`，**未**设置 `signingIdentity`
- `src-tauri/tauri.windows.conf.json` — `targets: ["nsis"]`，`installMode: currentUser`，**未**设置 `certificateThumbprint`

## 未完成的签名门槛

### macOS Developer ID + 公证

当前未配置：

- Apple Developer ID Application 证书
- `signingIdentity`
- 公证用 `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` / `APP_SPECIFIC_PASSWORD`
- entitlements 与 stapling 流水线

本地 `tauri build` 若走到签名，只可能是 ad-hoc。**ad-hoc ≠ Developer ID，更不是公证。** 未满足本门槛前不得把 dmg 标成正式发布包。

### Windows Authenticode

当前未配置：

- Authenticode 证书 / `certificateThumbprint`
- 时间戳服务
- 证书私钥（Secrets）

未满足本门槛前不得把 NSIS 安装包标成已签名正式包。

### updater 私钥

应用内只保留公钥。`tauri.conf.json` 现在嵌入的是 **TEST ONLY fixture 公钥**（见 `src-tauri/fixtures/updater/`）。

正式发布前必须：

1. 用 `tauri signer generate` 在隔离环境生成**新的**生产密钥对
2. 把生产公钥写入构建配置（替换 fixture 公钥）
3. 把生产私钥只放进 GitHub Secrets：`TAURI_SIGNING_PRIVATE_KEY`，可选 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
4. 本次 **不** 创建这些 Secrets，**不** 把生产私钥提交进仓库

fixture 私钥 `TEST_ONLY.minisign.key` 仅供 `cargo test`。丢失或泄露不影响生产，但生产密钥一旦投入使用后丢失则无法再给已安装用户签名更新。

## 公开 release 仓库

默认 metadata：

- stable: `https://github.com/Jia-Ethan/keysmith-switch-releases/releases/latest/download/latest.json`
- beta: `https://github.com/Jia-Ethan/keysmith-switch-releases/releases/download/beta-latest/latest.json`
- 失败回落页: `https://github.com/Jia-Ethan/keysmith-switch-releases/releases`

`Jia-Ethan/keysmith-switch-releases` **尚未创建**。本次不创建、不 push、不发 Release。

覆盖方式（本地 fixture / 调试）：

```text
KEYSMITH_SWITCH_UPDATER_ENDPOINT
KEYSMITH_SWITCH_UPDATER_ENDPOINT_BASE
KEYSMITH_SWITCH_UPDATER_PUBKEY
KEYSMITH_SWITCH_UPDATE_CHANNEL
```

客户端永不附加 GitHub token。

## 更新策略门槛（已接线）

- 禁止静默安装：`install_update` / `install_app_update` 必须 `confirmed=true`
- 签名失败、离线、损坏 metadata、降级、下载中断：保持当前版本
- Linux 平台键存在于 map 中，但标记 unsupported
- 启动延迟检查由前端负责

## 官方工具门槛

- Claude Code：npm `@anthropic-ai/claude-code`，文档 `https://docs.anthropic.com/en/docs/claude-code`
- Codex：npm `@openai/codex`
- Grok Build：只做本机 `which` + `--version`；latest 保持 `null`；无自动安装 argv
- ZCode：macOS `/Applications/ZCode.app`；Windows `available/argv` 为空并给出原因
- 未 `confirmed=true` 不得执行安装命令；argv 仅数组，不走 shell 拼接

## 构建命令（凭证齐备后才算正式包）

```bash
# macOS Apple Silicon
npx tauri build --target aarch64-apple-darwin --config src-tauri/tauri.macos.conf.json

# Windows x64（在 Windows 或已配置的交叉环境）
npx tauri build --target x86_64-pc-windows-msvc --config src-tauri/tauri.windows.conf.json
```

未设置 `TAURI_SIGNING_PRIVATE_KEY` 时，updater `.sig` 不会按生产密钥生成。未设置 Apple / Authenticode 凭证时，产物不是正式签名包。
