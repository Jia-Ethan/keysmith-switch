# 0.1.0 unsigned Preview 发布笔记

当前只准备未签名 Preview 候选，不对应 GitHub Release。公开仓库 `keysmith-switch-releases` 尚未创建。

## 产物意图

- macOS Apple Silicon：adhoc `.app` + unsigned `.dmg`
- Windows x64：unsigned NSIS per-user `.exe`
- Linux：非首发

两个平台都附带 SHA-256。Actions artifact 名称包含 `unsigned-preview`，不得改称正式签名包。

## 更新通道

- Preview 不生成 updater artifact 或 `latest.json`
- 应用内更新暂不用于 Preview 分发
- 正式更新通道仍需生产 updater 密钥、公钥和独立发布仓库

## 明确未做

- 未创建 `Jia-Ethan/keysmith-switch-releases`
- 未配置 GitHub Secrets
- 未完成 Developer ID / 公证 / Authenticode
- 未完成 Windows 实机安装、更新和卸载验收

门槛清单见 `docs/RELEASE_GATES.md`。
