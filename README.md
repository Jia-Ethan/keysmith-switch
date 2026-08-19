# Keysmith Switch

本地提示词管理桌面工具，面向 Claude Code、Codex、Grok Build 与 ZCode。

所有真实配置写入都经过四个已审计的 Keysmith CLI，GUI 不直接改写 `CLAUDE.md`、`~/.codex`、`~/.grok` 或 `~/.zcode-keysmith`。

## 状态

当前版本 `0.1.0`。源码仓库保持私有。应用更新 metadata 计划放在独立公开仓库 `Jia-Ethan/keysmith-switch-releases`（本次实现不创建该仓库）。

详见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md)。

## 开发

需要 Node 20+、Rust 1.85+、Python 3.9+。

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

macOS Apple Silicon 打包（未签名，正式签名是发布门槛）：

```bash
npx tauri build --target aarch64-apple-darwin
```

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
