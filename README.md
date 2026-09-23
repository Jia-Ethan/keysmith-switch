# Keysmith Switch

本地提示词管理桌面工具，面向 Claude Code、Codex、Grok Build 与 ZCode。

应用提供手动检查和“更新并重启”。安装包不使用 Apple Developer ID、公证或 Windows Authenticode；更新 payload 使用独立生产密钥签名，并在安装前由客户端验证。

所有真实配置写入都经过四个随应用交付的 Keysmith sidecar（frozen，不需要用户安装 Python）。GUI 不直接改写 `CLAUDE.md`、`~/.codex`、`~/.grok` 或 `~/.zcode-keysmith`。

界面与发布结构参照 MIT 开源项目 [CC Switch](https://github.com/farion1231/cc-switch) 的桌面产品完成度，业务模型仍是 Keysmith Switch。映射与版权见 [`docs/CC_SWITCH_MAPPING.md`](docs/CC_SWITCH_MAPPING.md)。

## 状态

当前准备发布 `v0.1.6`。源码在 `main`（`01c16ae`，#19）。annotated tag、安装包和公共 feed 还没发出。`v0.1.5` 这个 tag 已被更早的发布占用，不含这次首屏。上一份可下载的稳定版是 [`v0.1.4`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.4)。

`v0.1.4` 接替四个独立桌面安装包。之后只维护 Keysmith Switch。已发出的 Claude、Grok、Codex、Zcode 桌面包保持原样，不撤回，也不改成各自仓库的 Latest。

| 产品 | 最后的桌面安装包 | sidecar |
| --- | --- | --- |
| Claude | [`desktop-v0.1.0-beta.3`](https://github.com/Jia-Ethan/claude-keysmith/releases/tag/desktop-v0.1.0-beta.3) | v7.2 |
| Grok | [`desktop-v0.1.0-beta.5`](https://github.com/Jia-Ethan/grok-keysmith/releases/tag/desktop-v0.1.0-beta.5) | v0.6.1 |
| Codex | [`desktop-v0.6.0-beta.2`](https://github.com/Jia-Ethan/codex-keysmith/releases/tag/desktop-v0.6.0-beta.2) | v0.6.0 |
| Zcode | [`desktop-v0.1.0-beta.1`](https://github.com/Jia-Ethan/zcode-keysmith/releases/tag/desktop-v0.1.0-beta.1)（prerelease） | 0.3.2 |

已安装 `v0.1.3` 或 `v0.1.4` 的 Switch 可以在 `v0.1.6` 发出后从应用内更新。`v0.1.1` 内置测试 updater 公钥，仍须手动安装。范围见 [keysmith-switch#6](https://github.com/Jia-Ethan/keysmith-switch/issues/6)。

公开 updater 仓库 [`Jia-Ethan/keysmith-switch-releases`](https://github.com/Jia-Ethan/keysmith-switch-releases)、来源验证、生产 updater 签名与受保护发布环境已经启用。

详见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) 与 [`docs/RELEASE_GATES.md`](docs/RELEASE_GATES.md)。

## 复制给智能体安装

> 请先阅读本仓库 README 与发布边界，再从公开 Releases 安装当前稳定版。操作前检查系统架构、现有版本和数据目录，列出准确的下载、安装与备份路径；覆盖现有应用或写入托管配置前等待确认。安装后最小验证安装包 SHA-256、应用启动、既有提示词数据和“检查更新”。除非明确要求，不要从源码构建。

## 下载

当前可下载的稳定版仍是 [`v0.1.4`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.4)。`v0.1.6` 的安装包要等这次发布完成。

- macOS Apple Silicon（M 系列，含 Mac Studio M4 Max）：`Keysmith.Switch_0.1.4_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.4_x64-setup.exe`

`v0.1.1` 用户需要手动安装 `v0.1.3` 或 `v0.1.4` 才能完成 updater bootstrap。已安装 `v0.1.3` 或 `v0.1.4` 可以在 `v0.1.6` 发出后应用内更新。

## 开发

开发需要 Node 20+、Rust 1.85+。打包 sidecar 需要 Python 3.9+ 与 PyInstaller。用户安装应用不需要 Python。

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

macOS Apple Silicon 本地开发构建（不生成 updater artifact）：

```bash
npx tauri build --target aarch64-apple-darwin --config src-tauri/tauri.preview.macos.conf.json
```

GitHub Actions 的 `development-candidate` 工作流只用于开发候选验证，不生成 updater artifacts。`release` workflow 从 GitHub-verified tag 构建 macOS/Windows updater payload，使用 minisign 签名，并在 `latest.json` 写入 `minimum_updater_version` 与准确 payload 字节数后交给公开仓库复核。

## 数据位置

- `~/.keysmith-switch/keysmith-switch.db`
- `~/.keysmith-switch/prompts/<tool>/<prompt-id>.md`
- `~/.keysmith-switch/backups/<operation-id>/`
- `~/.keysmith-switch/logs/`

测试可用 `KEYSMITH_SWITCH_HOME` 覆盖数据根。

## 反馈

Bug、需求与破限建议统一提交至 [GitHub Discussions](https://github.com/Jia-Ethan/keysmith-switch/discussions/3)。请勿公开提交 token、完整配置、提示词正文或其他敏感信息。当前仓库尚未启用 Private Vulnerability Reporting，安全漏洞请勿在 Discussion 公开披露。

## 内置 CLI

`third_party/keysmith/` 钉选审计版本，随应用原子更新：

| 工具 | 版本 | HEAD |
| --- | --- | --- |
| claude-keysmith | v7.2 | 7dbfa253 |
| codex-keysmith | v0.6.0 | 33cf4049 |
| grok-keysmith | v0.6.1 | 168f604a |
| zcode-keysmith | 0.3.2（无 `v0.3.2` tag；不是 CLI Latest `v0.3.1`） | 7348b875 |

## 许可

MIT
