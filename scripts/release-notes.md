# 0.1.1 unsigned Preview 发布笔记

Keysmith Switch v0.1.1 unsigned Preview：完成暖纸张、炭黑与香槟金视觉重构，同时保留 Claude Code、Codex、Grok Build 和 ZCode 的提示词管理、预览、确认、恢复与安全门禁。

私有仓库 Pre-release：`https://github.com/Jia-Ethan/keysmith-switch/releases/tag/v0.1.1`

构建证据：`https://github.com/Jia-Ethan/keysmith-switch/actions/runs/32433727011`

## 产物意图

- macOS Apple Silicon：adhoc `.app` + unsigned `.dmg`
- Windows x64：unsigned NSIS per-user `.exe`
- Linux：非首发

两个平台都附带 `SHA256SUMS.txt`，不得改称正式签名包。

- macOS DMG SHA-256：`a911d2dd601d127fe0f5d478695bcbf7882b520bc9c913555509cd96ed89a96e`
- Windows EXE SHA-256：`f37f9926266f596290e183d3248056d168af24c943732919b4a4c93020c5b461`

## 更新通道

- Preview 不生成 updater artifact 或 `latest.json`
- 应用内更新暂不用于 Preview 分发
- 生产 updater 密钥、公钥、独立公开发布仓库、Secrets 与发布流水线已配置
- 正式更新通道仍需 Apple Developer ID/notarization、Windows Authenticode 与原生平台验收

## 明确未做

- 未发布正式 updater Release、tag 或 beta `latest.json`
- 未完成 Developer ID / 公证 / Authenticode
- 未完成 Windows 实机安装、更新和卸载验收

门槛清单见 `docs/RELEASE_GATES.md`。
