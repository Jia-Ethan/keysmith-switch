# Keysmith Switch

本地提示词管理桌面工具，面向 Claude Code、Codex、Grok Build 与 ZCode。

**当前发行形态：unsigned Preview。** 安装时 macOS / Windows 系统安全警告是预期行为，产物不得称为已签名正式版。由于 Preview 不发布 updater artifacts，关于页的应用内更新仅在未来生产签名通道启用后可用。

所有真实配置写入都经过四个随应用交付的 Keysmith sidecar（frozen，不需要用户安装 Python）。GUI 不直接改写 `CLAUDE.md`、`~/.codex`、`~/.grok` 或 `~/.zcode-keysmith`。

界面与发布结构参照 MIT 开源项目 [CC Switch](https://github.com/farion1231/cc-switch) 的桌面产品完成度，业务模型仍是 Keysmith Switch。映射与版权见 [`docs/CC_SWITCH_MAPPING.md`](docs/CC_SWITCH_MAPPING.md)。

## 状态

当前版本 `0.1.1` 已发布为私有仓库 [unsigned Pre-release](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.1.1)。生产 updater 密钥、公开仓库 [`Jia-Ethan/keysmith-switch-releases`](https://github.com/Jia-Ethan/keysmith-switch-releases)、Secrets 与发布流水线已经配置，但尚未发布正式 updater Release；Apple/Windows 平台签名材料缺失时流水线会 fail closed。

macOS Apple Silicon DMG 已完成挂载、sidecar 与隔离 HOME 启动验证。Windows NSIS EXE 已由 GitHub-hosted Windows runner 原生构建，但仍需 Windows 实体机安装、启动、升级和卸载验收。

详见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) 与 [`docs/RELEASE_GATES.md`](docs/RELEASE_GATES.md)。

## 下载

从 [`v0.1.1` Pre-release](https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.1.1) 下载对应平台产物，并使用随附的 `SHA256SUMS.txt` 校验：

- macOS Apple Silicon：`Keysmith.Switch_0.1.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.1_x64-setup.exe`

两个安装包均未使用平台生产签名，系统安全警告属于当前 Preview 的已知边界。

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

macOS Apple Silicon 打包（未签名 Preview，不生成 updater artifact）：

```bash
npx tauri build --target aarch64-apple-darwin --config src-tauri/tauri.preview.macos.conf.json
```

GitHub Actions 的 `preview-release` 工作流只构建并保存 unsigned DMG/NSIS 与 SHA-256，不自动创建 GitHub Release；`v0.1.1` Pre-release 由已验证的 Actions 产物手动发布。正式签名候选继续使用独立的 `release` 工作流。

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
