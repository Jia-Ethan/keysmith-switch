# Linux 支持

Linux 目前是**源码构建 / 本地运行**目标：应用、五个适配器 sidecar 和输入替换中转服务都能在 Linux 上构建和运行，但还没有发布 Linux 安装包，应用内更新在 Linux 上仍然显示不支持。

## 构建依赖

以 Ubuntu / Debian 为例：

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential curl file libssl-dev pkg-config patchelf \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev
```

还需要 Node 20+、Rust 1.85+、Python 3.9+ 与 PyInstaller（`scripts/build-sidecars.py` 会自动创建虚拟环境并安装 PyInstaller）。

## 构建与运行

```bash
npm install
npm run build
python3 scripts/build-sidecars.py
cd src-tauri
cargo test --workspace
cargo run            # 开发运行
```

打包 `.deb` 与 AppImage：

```bash
cd /path/to/keysmith-switch
npx tauri build --bundles deb,appimage
```

产物在 `src-tauri/target/release/bundle/`。Linux 打包配置在 `src-tauri/tauri.linux.conf.json`，不生成 updater artifact。

如果 AppImage 打包在滚动发行版上失败，日志里出现 `strip: unknown type [0x13] section .relr.dyn`，这是 AppImage 工具链里的旧 `strip` 不认识新格式；跳过 strip 再打包：

```bash
NO_STRIP=1 npx tauri build --bundles appimage
```

## 输入替换（中转服务）

中转服务本身是平台无关的 HTTP 服务，Linux 上会注册为用户级后台服务：

- 有可用的 systemd 用户会话时，写入 `~/.config/systemd/user/keysmith-switch-relay.service`（遵循 `XDG_CONFIG_HOME`），并执行 `systemctl --user enable --now`。
- 没有可用的用户级 systemd（容器、精简会话等）时，退回 XDG 自启动项 `~/.config/autostart/keysmith-switch-relay.desktop`，同时立即启动中转服务。
- 日志写入 `~/.keysmith-switch/logs/relay.log`。断开所有 Agent 后，服务会停止并移除注册项。

如果 systemd 会话可用但没有立即生效，可以手动检查：

```bash
systemctl --user status keysmith-switch-relay.service
journalctl --user -u keysmith-switch-relay.service
```

## Claude Code 的 shell 包装

claude-keysmith 在 Unix 上默认把包装函数写入 `~/.zshrc`。Linux 上 Switch 会按 `$SHELL` 指定正确的文件：

| `$SHELL` | 写入位置 |
| --- | --- |
| zsh | `~/.zshrc` |
| bash | `~/.bashrc` |
| sh / dash / ksh | `~/.profile` |

fish 暂不支持：上游适配器生成的是 POSIX shell 函数，不是 fish 语法。

## 已知限制

- **没有 Linux 发布包，也没有应用内更新。** `updater` 的平台表仍把 Linux 标为 unsupported，避免在缺少签名产物时误下载。
- **ZCode 官方安装被禁用。** 上游 ZCode 只提供 macOS / Windows 安装路径；如果发行版自己提供 ZCode，部署仍会按适配器的能力尝试，失败时以适配器输出为准。
- **Grok 没有自动安装源。** 这一点与 macOS / Windows 相同。
- **输入替换的规则与部署功能不依赖特权。** 中转服务只监听 `127.0.0.1`，不需要 root。

## 测试

`src-tauri/tests/rewrite_service_linux.rs` 使用 XDG autostart 模式做端到端测试：安装中转服务、确认端口可访问、卸载并确认进程退出。测试通过 `KEYSMITH_SWITCH_RELAY_SERVICE=autostart` 强制走回退路径，不会碰真实用户服务。

相关环境变量：

| 变量 | 作用 |
| --- | --- |
| `KEYSMITH_SWITCH_RELAY_SERVICE` | `systemd` 或 `autostart`，覆盖自动探测 |
| `KEYSMITH_SWITCH_SYSTEMD_USER_DIR` | 覆盖 systemd 用户单元目录 |
| `KEYSMITH_SWITCH_AUTOSTART_DIR` | 覆盖 XDG autostart 目录 |
| `KEYSMITH_SWITCH_RELAY_LABEL` | 覆盖服务 label（测试或多实例） |
