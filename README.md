# Keysmith Switch

独立的 Agent 提示词管理与快速部署桌面工具。用户可以粘贴自己的提示词，选择目标 Agent，先审阅部署计划，再通过对应适配器全局安装。

Switch 不绑定某一组官方提示词正文。提示词可以来自粘贴，或用户明确选择的本地 Markdown 文件；快速部署默认离线工作，不从其他 Keysmith 仓库的远程分支抓取内容。同一 Agent 的相同正文复用既有库条目；如果原条目标题或标签不同，返回并部署原条目，不会暗中改写其元数据。

所有真实配置写入都经过现有四个 Agent adapter。GUI 不直接改写目标 Agent 的配置文件；适配器负责目标格式，Switch 负责提示词库、版本、哈希校验、计划预览及操作记录。备份、漂移检测和恢复能力取决于当前 adapter。库中的 Markdown 带有元数据；交给 adapter 的临时文件只包含提示词正文，完成预览或执行后会清理。

界面与发布结构参照 MIT 开源项目 [CC Switch](https://github.com/farion1231/cc-switch) 的桌面产品完成度，业务模型仍是 Keysmith Switch。映射与版权见 [`docs/CC_SWITCH_MAPPING.md`](docs/CC_SWITCH_MAPPING.md)。

## 状态

公开稳定版是 [`v0.2.3`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.3)；`v0.2.4`（修复确认部署被适配器版本检查拒绝、Claude Code 部署后显示为未部署）正在准备中，尚未创建 tag 或安装包。`v0.2.3` 机器上已部署但库里没有对应条目时显示“部署中”卡片、ZCode 换回官方图标、授权改为 PolyForm Noncommercial 1.0.0（[#39](https://github.com/Jia-Ethan/keysmith-switch/pull/39)）；[`v0.2.2`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.2) 显示当前生效的提示词、去掉范围与快捷键、补全操作反馈（[#37](https://github.com/Jia-Ethan/keysmith-switch/pull/37)）；[`v0.2.1`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.1) 修复了反馈表单并更换设置图标（[#34](https://github.com/Jia-Ethan/keysmith-switch/pull/34)）；[`v0.2.0`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.0) 带来了界面改版 [#30](https://github.com/Jia-Ethan/keysmith-switch/pull/30)。上一版 [`v0.1.9`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.1.9) 带来了 Quick Deploy [#28](https://github.com/Jia-Ethan/keysmith-switch/pull/28)。

`v0.1.4` 接替四个独立桌面安装包。之后只维护 Keysmith Switch。已发出的 Claude、Grok、Codex、Zcode 桌面包保持原样，不撤回，也不改成各自仓库的 Latest。

| 产品 | 最后的桌面安装包 | sidecar |
| --- | --- | --- |
| Claude | [`desktop-v0.1.0-beta.3`](https://github.com/Jia-Ethan/claude-keysmith/releases/tag/desktop-v0.1.0-beta.3) | v7.2 |
| Grok | [`desktop-v0.1.0-beta.5`](https://github.com/Jia-Ethan/grok-keysmith/releases/tag/desktop-v0.1.0-beta.5) | v0.6.1 |
| Codex | [`desktop-v0.6.0-beta.2`](https://github.com/Jia-Ethan/codex-keysmith/releases/tag/desktop-v0.6.0-beta.2) | v0.6.0 |
| Zcode | [`desktop-v0.1.0-beta.1`](https://github.com/Jia-Ethan/zcode-keysmith/releases/tag/desktop-v0.1.0-beta.1)（prerelease） | 0.3.2 |

已安装的 Switch 可以在应用内检查更新。范围见 [keysmith-switch#6](https://github.com/Jia-Ethan/keysmith-switch/issues/6)。

公开 updater 仓库 [`Jia-Ethan/keysmith-switch-releases`](https://github.com/Jia-Ethan/keysmith-switch-releases)、来源验证、生产 updater 签名与受保护发布环境已经启用。

详见 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) 与 [`docs/RELEASE_GATES.md`](docs/RELEASE_GATES.md)。

## 复制给智能体安装

> 请先阅读本仓库 README 与发布边界，再从公开 Releases 安装当前稳定版。操作前检查系统架构、现有版本和数据目录，列出准确的下载、安装与备份路径；覆盖现有应用或写入托管配置前等待确认。安装后最小验证安装包 SHA-256、应用启动、既有提示词数据和“检查更新”。除非明确要求，不要从源码构建。

## 下载

当前可下载的稳定版是 [`v0.2.3`](https://github.com/Jia-Ethan/keysmith-switch-releases/releases/tag/v0.2.3)。

- macOS Apple Silicon（M 系列，含 Mac Studio M4 Max）：`Keysmith.Switch_0.2.3_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.2.3_x64-setup.exe`

已安装的用户可以在应用内检查更新；未来版本仍需完成独立发布流程。

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

## Agent adapter artifacts

当前发行包仍包含四个 legacy adapter sidecar，用于兼容 Claude Code、Codex、Grok Build 与 ZCode 的目标配置协议。它们是执行实现，不是 Quick Deploy 的默认提示词来源；Quick Deploy 不会自动抓取这些仓库的提示词，也不要求用户安装对应的 Keysmith 仓库。

`third_party/keysmith/` 中的版本锁只约束 legacy adapter artifact 的构建与审计。后续 Agent registry 会把这些 artifact 迁移到统一的 adapter manifest；用户提示词库和适配器供应链保持独立。

## 许可

[PolyForm Noncommercial 1.0.0](LICENSE)：源码公开，个人使用、学习和研究可以自由使用；商业使用需另行取得授权。`v0.2.2` 及更早版本按 MIT 发布，不受影响。改编自 CC Switch 的三个文件仍保留其 MIT 声明，见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

## 适配器版本

每个 Agent 的适配器（`third_party/keysmith/`）和应用要求的版本（`ToolKind::expected_version`）必须一致，否则确认部署会被拒绝。`src-tauri/tests/version_pins.rs` 在两者不一致时让测试失败。升级适配器时一起改这两处以及 `src-tauri/tests/fixtures/cli/` 里的默认版本号。
