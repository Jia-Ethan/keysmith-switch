# 公告

App 会读取公开仓库 `Jia-Ethan/keysmith-switch-releases` 的 `announcements` 分支上的 `announcements.json`：

```
https://raw.githubusercontent.com/Jia-Ethan/keysmith-switch-releases/announcements/announcements.json
```

改这个文件并推送到该分支，用户下次打开 App（或运行中每 6 小时、窗口回到前台且距上次超过 1 小时）就会看到。不需要发版。GitHub 的 raw 缓存约 5 分钟。

## 格式

```json
{
  "schema": 1,
  "announcements": [
    {
      "id": "2026-10-zcode-login",
      "kind": "news",
      "publishedAt": "2026-10-02T09:00:00Z",
      "title": { "zh-CN": "ZCode 请用 API 登录", "en": "Sign in to ZCode with an API key", "zh-TW": "ZCode 請用 API 登入" },
      "body": { "zh-CN": "ZCode 只支持 API 登录。其他 Agent 用 API 或账号登录都一样，建议用 API。" },
      "pinned": true,
      "tools": ["zcode"],
      "minAppVersion": "0.3.1",
      "link": "https://github.com/Jia-Ethan/keysmith-switch-releases"
    }
  ]
}
```

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `id` | 是 | 唯一、不再改；字母、数字、`-`、`_`、`.`，最长 64。已读和已关闭按 id 记录。 |
| `kind` | 是 | `news`（公告）、`release`（本版内容）、`preview`（下版预告）。 |
| `publishedAt` | 是 | RFC 3339 时间，列表按它倒序。 |
| `title` / `body` | 是 | `zh-CN` 必填，`en`、`zh-TW` 选填，缺的语言显示中文。纯文本，换行保留；标题最长 120 字，正文最长 4000 字。 |
| `pinned` | 否 | `true` 时在助手页顶部显示卡片，直到用户关闭。 |
| `tools` | 否 | `claude`、`codex`、`grok`、`zcode`。只在这些助手页显示置顶卡片；不填就是全部。 |
| `minAppVersion` / `maxAppVersion` | 否 | 只给这个版本范围（含两端）的 App 显示。 |
| `link` | 否 | 只允许 HTTPS，且域名为 `github.com`、`anthropic.com`、`openai.com`、`x.ai`、`z.ai`（含子域名）。 |

## 规则

- 不合法的单条公告会被丢掉，其他照常显示；整个文件读不出来时，App 保留上一次的内容。
- 文件最大 256 KB，最多显示 50 条。
- 删掉一条公告，它的已读、已关闭记录也会一起清掉。
- `schema` 只能是 `1`。以后格式升级会用新数字，旧版 App 会提示"需要更新版本"。
- 发版时，在这里加一条 `kind: "release"` 的公告，就是"本版内容"。
