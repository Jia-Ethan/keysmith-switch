# Keysmith Switch v0.1.4

这一版接替四个独立桌面安装包。之后只维护 Keysmith Switch。已发出的安装包不撤回，也不改成各自仓库的 Latest。

- Claude [`desktop-v0.1.0-beta.3`](https://github.com/Jia-Ethan/claude-keysmith/releases/tag/desktop-v0.1.0-beta.3)，sidecar v7.2
- Grok [`desktop-v0.1.0-beta.5`](https://github.com/Jia-Ethan/grok-keysmith/releases/tag/desktop-v0.1.0-beta.5)，sidecar v0.6.1
- Codex [`desktop-v0.6.0-beta.2`](https://github.com/Jia-Ethan/codex-keysmith/releases/tag/desktop-v0.6.0-beta.2)，sidecar v0.6.0
- Zcode [`desktop-v0.1.0-beta.1`](https://github.com/Jia-Ethan/zcode-keysmith/releases/tag/desktop-v0.1.0-beta.1)，sidecar 0.3.2，仍是 prerelease

范围见 [keysmith-switch#6](https://github.com/Jia-Ethan/keysmith-switch/issues/6)。

## 这一版有什么

- sidecar 钉到上面四个桌面正在发的版本。Zcode 没有 `v0.3.2` tag，钉的是桌面源码 `7348b875`，不是 CLI Latest `v0.3.1`。
- Claude 安装补上 `--runtime`、`--append-file`、`--max-tokens`。可以列出备份，并按名称还原。
- Grok 确认时带上 CLI 的 preview token，并支持 `--reconcile`。`run` 和 `breaktest` 是真实调用，输出按行显示，可以取消。
- Codex 补上 scenario 的 status、deploy、uninstall、recover，以及 fixture 的 scaffold、list、uninstall。仍是英文文本，不使用 `--preset`。内置 overlay 作为可导入的提示词。
- `v0.1.1` 检测到正式版时不再提供必然失败的「更新并重启」，改为手动安装说明和官方下载页。签名不符的英文错误留在详情里。未知或为零的体积不再显示 `0 B`。签名失败仍保留当前版本。

## 安装

- 已安装 `v0.1.3`：可以在应用内更新到 `v0.1.4`。
- 仍在 `v0.1.1`：内置的是测试 updater 公钥，必须手动安装本版本。
- macOS Apple Silicon：`Keysmith.Switch_0.1.4_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.1.4_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名。
