# Keysmith Switch v0.1.4-rc.1

- 加入长期 updater 兼容协议，密钥轮换或旧客户端 bootstrap 时切换到安全的手动更新流程。
- 修复签名 key ID 不匹配直出底层英文、错误流程仍可安装以及更新包显示 `0 B`。
- metadata 记录最低 updater 版本、payload 大小、SHA-256 与签名；源仓库和公开仓库独立校验。
- 本候选不发布 Release 或公共 feed；`v0.1.1` 用户仍需手动安装稳定版 `v0.1.3`。
