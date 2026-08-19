# 0.1.0 本地发布笔记（不公开）

本文件只留在源码仓，不对应 GitHub Release。公开仓库 `keysmith-switch-releases` 尚未创建。

## 产物意图

- macOS Apple Silicon：`.app` + `.dmg` + `*.app.tar.gz` updater artifact
- Windows x64：NSIS per-user + `*.exe.sig`
- Linux：非首发

## 更新通道

- 默认 stable metadata：`releases/latest/download/latest.json`
- beta metadata：`releases/download/beta-latest/latest.json`
- 公钥：当前为 fixtures/updater TEST ONLY 公钥；正式发布前必须轮换
- 安装必须用户确认；失败则保留 0.1.0

## 明确未做

- 未 push
- 未建 PR
- 未创建 `Jia-Ethan/keysmith-switch-releases`
- 未配置 GitHub Secrets
- 未生成生产 updater 私钥并入库
- 未完成 Developer ID / 公证 / Authenticode

门槛清单见 `docs/RELEASE_GATES.md`。
