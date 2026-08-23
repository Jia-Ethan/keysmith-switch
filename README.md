# Keysmith Switch

本地提示词管理桌面工具，面向 Claude Code、Codex、Grok Build 与 ZCode。

应用提供手动检查和“更新并重启”。安装包不使用 Apple Developer ID、公证或 Windows Authenticode；更新 payload 使用独立生产密钥签名，并在安装前由客户端验证。

所有真实配置写入都经过四个随应用交付的 Keysmith sidecar（frozen，不需要用户安装 Python）。GUI 不直接改写 `CLAUDE.md`、`~/.codex`、`~/.grok` 或 `~/.zcode-keysmith`。

界面与发布结构参照 MIT 开源项目 [CC Switch](https://github.com/farion1231/cc-switch) 的桌面产品完成度，业务模型仍是 Keysmith Switch。映射与版权见 [`docs/CC_SWITCH_MAPPING.md`](docs/CC_SWITCH_MAPPING.md)。

## 状态

下一版本 `0.1.3` 正在准备发布。它是生产 updater 公钥的 bootstrap 版本：现有 `0.1.1` 用户需要手动安装一次 `0.1.3`，后续版本才能通过应用内更新。

公开 updater 仓库 [`Jia-Ethan/keysmith-switch-releases`](https://github.com/Jia-Ethan/keysmith-switch-releases)、生产 updater Secrets、来源验证与受保护发布环境已经配置。公开 feed 在 `v0.1.3` 正式发布前仍返回 404。

详见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) 与 [`docs/RELEASE_GATES.md`](docs/RELEASE_GATES.md)。

## 下载

当前公开可下载版本仍为 [`v0.1.1` Pre-release](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.1.1)：

- macOS Apple Silicon：`Keysmith.Switch_0.1.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.1_x64-setup.exe`

`v0.1.3` 发布后需要手动下载安装以完成 updater bootstrap。macOS 和 Windows 安装包不使用平台发行签名，系统可能显示安全警告。

## 开发

开发需要 Node 20+、Rust 1.85+。打包 sidecar 需要 Python 3.9+ 与 PyInstaller。用户安装 Preview 包不需要 Python。

```bash
npm install
npm test
npm run build
cd src-tauri && cargo fmt --check && cargo check && cargo test
```

桌面调试：

```bash
npx tauri dev
```

macOS Apple Silicon 本地无平台签名构建（不生成 updater artifact）：

```bash
npx tauri build --target aarch64-apple-darwin --config src-tauri/tauri.preview.macos.conf.json
```

GitHub Actions 的 `preview-release` 工作流只用于本地候选验证，不生成 updater artifacts。`release` workflow 从 GitHub-verified tag 构建 macOS/Windows updater payload，使用 minisign 签名并交给公开仓库复核发布；它不执行 Apple 或 Windows 平台代码签名。

## 数据位置

- `~/.keysmith-switch/keysmith-switch.db`
- `~/.keysmith-switch/prompts/<tool>/<prompt-id>.md`
- `~/.keysmith-switch/backups/<operation-id>/`
- `~/.keysmith-switch/logs/`

测试可用 `KEYSMITH_SWITCH_HOME` 覆盖数据根。

## 内置 CLI

`third_party/keysmith/` 钉选审计版本，随应用原子更新：

| 工具 | 版本 | HEAD |
| --- | --- | --- |
| claude-keysmith | v7.1 | 3fe8902d |
| codex-keysmith | v0.3.8 | ae068de1 |
| grok-keysmith | v0.4.1 | 1f49c54a |
| zcode-keysmith | v0.1.0 | 77a27dec |

## 许可

MIT
