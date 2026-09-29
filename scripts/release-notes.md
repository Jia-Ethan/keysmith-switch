# Keysmith Switch v0.2.1

「问题上报」和「提需求」现在对所有用户都能用了；设置按钮换成了真正的齿轮图标。

## 这一版有什么

- 反馈默认在浏览器中打开仓库的 GitHub 表单，版本、系统、描述和联系方式都已预填。用你自己的 GitHub 账号检查后提交即可，截图可以直接拖进网页。
- 本机已安装并登录 GitHub CLI 时，仍会直接创建 issue。不再要求账号对仓库有推送权限：任何 GitHub 账号都能在公开仓库提交 issue。
- 描述很长时，表单只预填前面一部分并提示在网页补全，不会打不开。
- 修复中文描述超过约 3,300 字时被错误拒绝的问题：长度限制改为按字符计算。
- 设置按钮的图标由放射线（看起来像太阳）换成八齿齿轮。

## 安装

- 已安装 `v0.1.3` 及以上：可以在应用内检查更新到 `v0.2.1`；安装仍需用户确认。
- 仍在 `v0.1.1`：内置的是测试 updater 公钥，必须手动安装本版本。
- macOS Apple Silicon：`Keysmith.Switch_0.2.1_aarch64.dmg`
- Windows x64：`Keysmith.Switch_0.2.1_x64-setup.exe`

安装包不使用 Apple Developer ID、公证或 Windows Authenticode。更新包使用独立的生产 minisign 密钥签名；这不是平台代码签名。
