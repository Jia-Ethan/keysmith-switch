# Keysmith Switch v0.8.4

修复 macOS 开着系统代理时检查更新超时、或能检查到更新但下载失败的问题；随包 DSH 适配器升级到 0.4.0。

## 这一版有什么

- 检查更新、下载更新、扩展和公告拉取都会使用 macOS 系统代理（Clash、Surge 等设置的 HTTP / HTTPS / SOCKS 代理）。从访达打开 App 时没有代理环境变量，以前会直连 GitHub 并报 `curl: (28) Connection timed out`。[#155](https://github.com/Jia-Ethan/keysmith-switch/issues/155)
- 应用内下载更新现在也会用 macOS 和 Windows 的系统 HTTP 代理，不再出现“能检查到更新、下载却失败”。
- 设置了 `https_proxy` / `all_proxy` 时仍以环境变量为准；本机地址永远不走代理。
- 随包的 dsh-keysmith 从 0.2.2 升级到 0.4.0：DSH 部署改为把 Cordis 插件装进 `profiles/web`，不再写 `~/.dsh/AGENTS.md` 或 `~/.dsh/cordis.patch.yml`。

## 升级前请注意（DSH）

- 0.4.0 停用部署时不会还原旧版写下的 `~/.dsh/AGENTS.md` 和 `~/.dsh/cordis.patch.yml`。如果你在 v0.8.1–v0.8.3 部署过 DSH 并想回到原样，请**先在旧版里停用 DSH 部署，再升级 App**。
- 升级后，旧版的 DSH 部署会显示「已漂移」，旧的提示词仍然生效。重新部署即可换成新方式；旧版写下的 `~/.dsh/AGENTS.md` 和 `~/.dsh/cordis.patch.yml` 会留在原处，旧版部署前的 `AGENTS.md` 备份在同目录的 `AGENTS.md.bak_*`。
- 部署后需完全退出并重新打开 DSH。

## 安装

- 已安装 `v0.1.3` 及以上的版本，可以在应用内点「检查更新」升级到 `v0.8.4`，安装仍需你确认。如果 v0.8.3 检查更新超时，请临时开启代理软件的 TUN / 增强模式，或从下方下载安装包手动安装。
- macOS Apple Silicon：`Keysmith.Switch_0.8.4_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.8.4_x64-setup.exe`

安装包没有使用 Apple Developer ID、公证或 Windows Authenticode。更新包用独立的生产 minisign 密钥签名，这不是平台代码签名。

Windows 上如果火绒等杀毒软件提示有病毒：只要是从本页下载的，就是误报，可以放心使用。在杀毒软件的「隔离区」恢复被隔离的文件，再把 Keysmith Switch 加入「信任区」即可，详见 [README](https://github.com/Jia-Ethan/keysmith-switch#-下载)。
