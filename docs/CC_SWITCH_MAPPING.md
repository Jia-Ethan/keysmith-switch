# CC Switch 源码映射与 MIT 归属

静态分析基线：`work/source-audit-20260819/cc-switch` HEAD `0b5da510`，MIT License，Copyright (c) 2025 Jason Young。

本文件只记录**桌面产品结构、视觉系统、状态反馈与发布质量**的对应关系。不复制 CC Switch 的供应商、路由、MCP、Skills、会话和用量业务，也不使用其品牌名。

## 产品对应

| CC Switch | Keysmith Switch |
| --- | --- |
| 多应用供应商切换器 | 四工具提示词库管理器 |
| 当前供应商 / 其余供应商 | 已激活提示词 / 未激活提示词 |
| 供应商卡片 + 全屏编辑 | 提示词摘要 + 全屏 Markdown 编辑 |
| 设置横向标签 + About 子页 | 设置横向标签：通用 / 工具 / 数据与备份 / 更新 / 高级 / 关于 |
| 官方 CLI 安装行 | Claude / Codex 确认后 argv 安装；Grok / ZCode 只展示来源、命令、目标并打开官网 |
| 全局 Update Provider | 启动延迟检查，确认后下载，禁止静默安装 |
| 托盘 / 开机启动 / 静默启动 / 关窗进托盘 | 同结构，菜单含「退出 Keysmith Switch」 |

## 组件映射

| CC Switch 源码 | Keysmith Switch | 复用方式 |
| --- | --- | --- |
| `src/App.tsx` 顶栏、HEADER_HEIGHT、右侧设置/更新 | `src/components/AppShell.tsx` | 布局思路；不复制供应商/MCP/用量 |
| `src/components/AppSwitcher.tsx` 图标切换、溢出「更多」、选中态 | `src/components/AppShell.tsx` + `src/components/ToolLogos.tsx` | 交互模式；上一轮已按此改导航 |
| `src/components/providers/ProviderList.tsx` 搜索 + 当前/其余分组 | `src/components/PromptList.tsx` | 栏目结构，业务换成提示词 |
| `src/components/providers/ProviderCard.tsx` 卡片选中/状态 | `src/components/PromptList.tsx` 卡片 | 视觉密度与选中态，不含供应商字段 |
| `src/components/prompts/PromptFormPanel.tsx` | `src/components/PromptFormPanel.tsx` | 结构复用：全屏面板 + 标题 + Markdown |
| `src/components/MarkdownEditor.tsx` | `src/components/MarkdownEditor.tsx` | **MIT 改编**：CodeMirror Markdown + 查找 |
| `src/components/common/FullScreenPanel.tsx` | `src/components/FullScreenPanel.tsx` | **MIT 改编**：Portal、Escape、页脚；去掉 framer-motion |
| `src/components/FrontendErrorBoundary.tsx` | `src/components/ErrorBoundary.tsx` | **MIT 改编** |
| `src/components/DatabaseUpgrade.tsx` | `src/components/DataRecoveryDialog.tsx` | 可理解的恢复界面，不静默吞掉损坏 |
| `src/components/FirstRunNoticeDialog.tsx` | `src/components/FirstRunDialog.tsx` | 首次扫描导入预览，勾选导入，绝不自动激活 |
| `src/components/settings/SettingsPage.tsx` 顶部 Tabs | `src/pages/SettingsPage.tsx` | 横向标签，不用左侧大导航 |
| `src/components/settings/AboutSection.tsx` | `src/pages/AboutPage.tsx`（设置 > 关于） | 三层：应用 / 内置适配器 / 官方工具 |
| `src/components/settings/WindowSettings.tsx` | 设置 > 通用 | 关窗进托盘、开机启动、静默启动 |
| `src/contexts/UpdateContext` / `UpdateBadge` | `src/components/UpdateProvider.tsx` | 全局延迟检查、非打扰状态、确认后下载 |
| `src-tauri/src/lib.rs` CloseRequested / 单实例 / 窗口状态 | `src-tauri/src/lib.rs` + `desktop.rs` | 关窗进托盘、单实例聚焦、位置恢复 |
| `src-tauri/src/tray.rs` | `src-tauri/src/desktop.rs` | 精简菜单：显示主窗口 / 检查更新 / 退出 |
| `src-tauri/src/auto_launch.rs` | `src-tauri/src/auto_launch.rs` | **MIT 改编**：应用名改为 Keysmith Switch |
| `.github/workflows/ci.yml` `release.yml` | `.github/workflows/` | 本地模板，不触发远程、不配置 Secrets |
| DMG 背景、Applications 拖放、NSIS currentUser、WebView2、多尺寸 ICO | `src-tauri/tauri.*.conf.json` + `scripts/` | 发布结构与跨平台安装体验 |

## MIT 改编文件（保留版权声明）

以下文件含 CC Switch 的实质结构或逻辑，文件头保留 MIT 版权与许可：

- `src/components/MarkdownEditor.tsx`
- `src/components/FullScreenPanel.tsx`
- `src/components/ErrorBoundary.tsx`
- `src-tauri/src/auto_launch.rs`

未整段复制、仅参考交互的文件不标版权头，本映射已说明来源。

## 明确不复制

- 品牌名「CC Switch」及 `com.ccswitch.desktop`
- 供应商预设、Failover、代理接管、用量脚本体、MCP/Skills/会话
- CC Switch 的一键安装命令表（含 Hermes/Pi/OpenCode 等）
- 其生产 updater 公钥与发布域名

## 发布边界

用户界面不展示 Preview 或平台签名说明。发布与开发文档只记录实际构建、更新验证和兼容性边界，不把 updater minisign 描述成平台代码签名。
