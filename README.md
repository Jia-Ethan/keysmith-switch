# Keysmith Switch

本地提示词管理桌面工具，面向 Claude Code、Codex、Grok Build 与 ZCode。

**当前发行形态：unsigned Preview。** 安装时 macOS / Windows 系统安全警告是预期行为，产物不得称为已签名正式版。由于 Preview 不发布 updater artifacts，关于页的应用内更新仅在未来生产签名通道启用后可用。

所有真实配置写入都经过四个随应用交付的 Keysmith sidecar（frozen，不需要用户安装 Python）。GUI 不直接改写 `CLAUDE.md`、`~/.codex`、`~/.grok` 或 `~/.zcode-keysmith`。

界面与发布结构参照 MIT 开源项目 [CC Switch](https://github.com/farion1231/cc-switch) 的桌面产品完成度，业务模型仍是 Keysmith Switch。映射与版权见 [`docs/CC_SWITCH_MAPPING.md`](docs/CC_SWITCH_MAPPING.md)。

## 状态

当前版本 `0.1.0` Preview。源码仓库保持私有。应用更新 metadata 计划放在独立公开仓库 `Jia-Ethan/keysmith-switch-releases`（**尚未创建**）。

macOS Apple Silicon 本机已打出品牌 DMG（未公证）。Windows NSIS 需在原生 Windows 上验收后才能标通过。

详见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) 与 [`docs/RELEASE_GATES.md`](docs/RELEASE_GATES.md)。

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

GitHub Actions 的 `preview-release` 工作流只构建并保存 unsigned DMG/NSIS 与 SHA-256，不创建 GitHub Release。正式签名候选继续使用独立的 `release` 工作流。

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
