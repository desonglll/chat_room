# 目标架构

本文件描述 M0 结束时仓库应有的形状，以及 Chat 数据模型的设计。任务卡不重复这里的内容，只引用。

---

## 1. 仓库结构

```
chat_room/
├── Cargo.toml                单 crate，不改为 workspace
├── build.rs                  M0 起嵌入 packages/web/dist（原先是 web/dist）
├── src/                      Rust 服务端
├── migrations/               SQLite
├── migrations-postgres/      PostgreSQL
├── package.json              bun workspaces 根，声明 packages/*
├── packages/
│   ├── core/                 @tg/core   平台无关业务层
│   ├── ui/                   @tg/ui     React 基础组件与 design tokens
│   └── web/                  @tg/web    React 应用
├── web/                      旧 Vue 客户端 —— M0 起冻结，M6 删除
├── desktop/                  PySide6 —— M6 决定去留
├── docs/tg/                  本计划
└── docs/devlog/              每个任务一个开发日志
```

Rust 侧保持单 crate。拆 workspace 会让共享构建目录的锁竞争变复杂，而当前没有第二个消费者需要 library crate。

## 2. `packages/core` 的边界（为移动端留门的关键）

`@tg/core` 承载**所有不涉及渲染的东西**：类型、HTTP 客户端、WebSocket 客户端、Zustand store、领域逻辑。

```
packages/core/src/
├── types/          与 Rust 契约一一对应的类型，按域分文件
├── api/            HTTP 客户端，按域分文件（chats.ts, messages.ts, stickers.ts …）
├── realtime/       WS 连接管理、重连、协议帧编解码
├── stores/         Zustand vanilla store，一个域一个文件
└── domain/         纯函数：乐观发送、游标推进、滚动锚点策略、计算器、markdown 清洗…
```

**两条硬约束，由 CI 检查：**

1. `packages/core` 不得 `import` `react`、`react-dom`、`@tg/ui`、`@tg/web`。
2. `packages/core` 不得引用 `window`、`document`、`localStorage`、`navigator`。需要平台能力时，定义接口由宿主注入（`createStorage()`、`createWebSocket()`、`createFileReader()`）。

违反这两条，将来接 React Native 就要重做一遍。这是本架构唯一真正不可妥协的地方。

Store 用 Zustand 的 **vanilla** 形态（`zustand/vanilla`）创建于 `core`，`packages/web` 通过 `zustand/react` 的 `useStore` 订阅。这样 store 不依赖 React。

**Store 按域切分，一个文件一个 store**，这既是设计需要也是并行开发需要 —— 多个 worktree 不会抢同一个文件：

```
stores/
├── authStore.ts          会话、当前用户
├── chatListStore.ts      会话列表、置顶/归档/静音、文件夹
├── messageStore.ts       按 chatId 分片的消息缓存、游标、乐观消息
├── composerStore.ts      草稿、回复态、编辑态、待发附件
├── presenceStore.ts      在线状态、输入状态
├── stickerStore.ts       贴纸包、最近、收藏
├── mediaStore.ts         上传进度、下载缓存
├── settingsStore.ts      主题、隐私、通知、自动下载
└── uiStore.ts            侧栏宽度、当前打开的面板、模态栈
```

## 3. `packages/ui` 的边界

只有基础组件与 design tokens，**不含任何业务概念**（不认识 Chat、Message、User）。约 15 个原子，Telegram 的界面就这么点东西：

`Button` `IconButton` `Ripple` `TextField` `Toggle` `Checkbox` `Radio` `Menu` `ContextMenu` `Popover` `Modal` `Sheet` `Tooltip` `Tabs` `Avatar` `Badge` `Spinner` `Skeleton` `ScrollArea`

tokens 分三层，这个分层决定了主题能不能在运行时切换：

```
tokens/
├── primitive.css     原始值：色阶、间距刻度、圆角、字号、动效曲线与时长
├── semantic.css      语义映射：--tg-bg, --tg-text-primary, --tg-accent, --tg-bubble-out …
└── themes/           day.css, night.css, 以及强调色覆盖层
```

组件只消费 `semantic` 层，永不直接用 `primitive` 值。主题切换 = 换 `semantic` 层的赋值，组件零改动。

## 4. Chat 数据模型

### 4.1 一个关键的既有事实

`direct_conversations.room_id` **已经是 `rooms(id)` 的外键**，所有消息都在同一张 `messages` 表里按 `room_id` 索引。也就是说单聊本来就是一个 room，`direct_conversations` 只是记录两个参与者的旁表。

**这让四种 chat_type 的统一几乎是免费的**：迁移只需给 `direct_conversations` 里出现过的 chat 打上 `chat_type='private'`，其余打 `'group'`。`direct_conversations` 保留为「按两个 user 反查私聊」的索引表，不废弃。

### 4.2 `chats` 表（由 `rooms` 重命名并扩展）

```sql
chats (
  id                              TEXT PRIMARY KEY,
  chat_type                       TEXT NOT NULL
      CHECK (chat_type IN ('private','group','supergroup','channel')),
  title                           TEXT NOT NULL,          -- 原 rooms.name
  username                        TEXT,                   -- 公开句柄，活跃行内唯一
  access_hash                     TEXT NOT NULL,          -- 公开解析时不泄露 id
  password_hash                   TEXT NOT NULL DEFAULT '',
  creator_user_id                 TEXT REFERENCES users(id) ON DELETE SET NULL,
  join_policy                     TEXT NOT NULL DEFAULT 'open'
      CHECK (join_policy IN ('open','approval')),
  avatar_emoji                    TEXT NOT NULL DEFAULT '',
  description                     TEXT NOT NULL DEFAULT '',
  is_forum                        INTEGER NOT NULL DEFAULT 0,
  linked_chat_id                  TEXT REFERENCES chats(id) ON DELETE SET NULL,
  slow_mode_seconds               INTEGER NOT NULL DEFAULT 0,
  auto_delete_seconds             INTEGER NOT NULL DEFAULT 0,
  signatures_enabled              INTEGER NOT NULL DEFAULT 0,
  history_visible_to_new_members  INTEGER NOT NULL DEFAULT 1,
  member_count                    INTEGER NOT NULL DEFAULT 0,   -- 投影，非真相
  deleted_at                      TEXT,
  created_at                      TEXT NOT NULL
)
```

`member_count` 是投影，权威值仍是 `chat_members` 的计数。超级群到 20 万成员时实时 count 不可行，所以落一个投影列，并由成员变更的同一事务维护。

`username` 沿用 `rooms_name_active_idx` 的模式：`CREATE UNIQUE INDEX chats_username_active_idx ON chats (username) WHERE deleted_at IS NULL AND username IS NOT NULL`。

### 4.3 四种类型的语义差异

| | `private` | `group` | `supergroup` | `channel` |
| --- | --- | --- | --- | --- |
| 成员上限 | 2 | 200 | 200000 | 无限订阅者 |
| 谁能发言 | 双方 | 全体成员 | 受 `default_permissions` 与 per-user 限制约束 | 仅有 `post_messages` 权限者 |
| 新成员可见历史 | 全部 | 加入后 | 由 `history_visible_to_new_members` 决定 | 全部 |
| 公开 username | 不可 | 不可 | 可 | 可 |
| 话题 | 不可 | 不可 | `is_forum` 时可 | 不可 |
| 浏览量 | 无 | 无 | 无 | 有 |
| 评论区 | 无 | 无 | 无 | `linked_chat_id` 指向讨论群 |
| 慢速模式 | 无 | 无 | 有 | 无 |

`group → supergroup` 的升级是单向的、不可逆的，Telegram 亦如此。升级时机：超过 200 成员、启用公开 username、启用话题、设置慢速模式。

### 4.4 权限：扩展既有 RBAC，不引入位标志

仓库已有 `room_permissions`（key → 描述）+ `room_roles` + `room_role_permissions` 的注册表式 RBAC，现有 9 个 key。**扩展这张注册表，不要新建一套 bitflags。**

M2 需要新增的 `permission_key`：

```
message.post            频道发布（区别于 message.send）
message.edit_any        编辑他人消息（频道管理员）
message.delete_any      删除他人消息
message.pin             置顶
message.send_media      发送媒体
message.send_sticker    发送贴纸与 GIF
message.send_poll       发起投票
message.embed_link      链接预览
members.ban             封禁与限制成员
members.promote         任命管理员
chat.info               修改标题、头像、简介
chat.topics             管理话题
chat.anonymous          匿名发言
chat.call               管理语音房（预留，本期不实现）
```

per-user 的临时限制（禁言到某时刻）需要新表，因为它不是角色，是对个体的时限覆盖：

```sql
chat_member_restrictions (
  chat_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  denied_permission_key TEXT NOT NULL,
  until TEXT,                       -- NULL = 永久
  restricted_by TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (chat_id, user_id, denied_permission_key)
)
```

授权判定顺序：**系统管理员 → 创建者 → 角色权限 → per-user 限制（能否被覆盖为拒绝）→ chat_type 固有约束**。读路径与写路径都要判定，这是 `AGENTS.md` 的既有要求。

### 4.5 `messages` 表的扩展

按里程碑分批加列，不要一次性全加（迁移必须成对，且每个列要有使用者）：

| 列 | 里程碑 | 用途 |
| --- | --- | --- |
| `topic_id` | M2 | 话题内消息 |
| `views_count` | M2 | 频道浏览量 |
| `post_author` | M2 | 频道作者签名 |
| `media_kind` | M3 | `text` / `photo` / `video` / `voice` / `video_note` / `sticker` / `gif` / `poll` / `location` / `contact` |
| `grouped_id` | M4 | 相册：同组消息渲染为一条 |
| `scheduled_at` | M4 | 定时发送 |
| `silent` | M4 | 静默发送 |
| `auto_delete_at` | M4 | 自毁计时 |
| `reply_quote_text` / `reply_quote_offset` | M4 | 引用片段 |
| `reply_to_chat_id` | M4 | 跨聊天回复 |

`media_kind` 放在 M3 而不是 M4，因为贴纸是第一个非文本非附件的消息类型，它迫使这个字段出现。

### 4.6 新表清单（按里程碑）

| 里程碑 | 新表 |
| --- | --- |
| M0 | 无新表，只有重命名与列扩展 |
| M2 | `forum_topics`、`chat_member_restrictions`、`chat_invite_links`、`chat_admin_log`（复用 `audit_events` 若够用则不建） |
| M3 | `sticker_sets`、`stickers`、`user_sticker_sets`、`user_recent_stickers`、`user_favorite_stickers`、`custom_emoji`、`user_saved_gifs` |
| M4 | `polls`、`poll_options`、`poll_votes`、`message_locations`、`link_previews` |
| M5 | `chat_folders`、`chat_folder_rules`、`user_privacy_rules`、`user_privacy_exceptions`、`two_factor_credentials`、`chat_wallpapers`、`notification_exceptions` |

## 5. API 契约方向

### 5.1 重命名与兼容

`/api/rooms/*` → `/api/chats/*`。旧路径保留为 **deprecated alias**，指向同一 handler，M6 删除。理由：ratatui CLI 与 PySide6 桌面端在 M0–M5 期间是有用的手工验证工具，不值得为词汇统一而废掉它们。

alias 只做路由层转发，**不复制任何领域逻辑**。

### 5.2 WebSocket 帧

现有 `ChatMessage` 枚举（`src/models.rs`）已有 `Broadcast` / `Typing` / `Presence` 等变体。扩展而非替换。M2 起需要的新变体：

```
ChatUpdated          标题/头像/权限/慢速模式变更
MemberUpdated        加入/离开/角色/限制变更
TopicUpdated         话题创建/关闭/置顶
MessageViewsUpdated  频道浏览量批量更新（必须批量，逐条会打爆连接）
PollUpdated          投票结果
DraftUpdated         云端草稿跨设备同步
```

`Typing` 需要从「一种状态」扩展为带 `action` 字段的粒度状态（`typing` / `recording_voice` / `uploading_photo` / `uploading_video` / `recording_video_note` / `choosing_sticker` …）。这是一个破坏性的帧变更，**必须在 M0 与重命名一起做完**，不要留到 M4，否则要改两次客户端。

### 5.3 分页契约

现有 `MessageCursor { created_at, id }` 的游标设计是对的，保留。频道浏览量、投票结果这类高频变更的字段**不进入游标计算**，否则会导致重复拉取。

## 6. 旧客户端的处置时间线

| 客户端 | M0 | M1–M5 | M6 |
| --- | --- | --- | --- |
| `web/`（Vue） | 冻结，仅接受编译修复 | 通过 `/api/rooms/*` alias 保持可运行，作为行为对照 | 删除 |
| `desktop/`（PySide6） | 不动 | 同上 | 决策：迁 Tauri、保留、或删除 |
| `src/bin/client.rs`（ratatui） | 不动 | 同上，主要用于服务端手工验证 | 保留 |

`build.rs` 在 M0 就切到 `packages/web`。切换期间旧 Vue 客户端仍可用 `cd web && bun dev` 独立跑起来对照行为，只是不再被编进二进制。
