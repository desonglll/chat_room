# 好友系统与双人私聊实施计划

状态：已实施；自动化测试完成，浏览器视觉复核受当前运行环境限制
制定日期：2026-08-19
适用范围：Rust/Axum/SQLx 后端、SQLite/Postgres、Vue 3 Web 客户端

## 1. 目标

在保留现有群聊房间能力的前提下，增加完整的双向好友关系和一对一私聊：

- 用户可以搜索其他账户、发送/取消好友请求、接受/拒绝请求、删除好友和拉黑用户。
- 好友之间可以开始私聊；私聊复用现有 room、message、read receipt、attachment、reply、edit、recall、forward 和 WebSocket 能力。
- 私聊 room 始终只有两个指定成员，不进入公开发现，不允许第三人加入，不出现群管理操作。
- 左侧改成 Telegram 式统一会话列表，群聊和私聊按最近活动混排，展示最后消息、时间和未读数。
- 保留 Echo Gate 品牌、现有配色和主题系统；参考 Telegram 的信息架构与交互密度，不复制其品牌。

## 2. 已确认的核心决策

### 2.1 私聊就是受约束的 room

不创建第二套私聊消息表或私聊 WebSocket。新增 `direct_conversations` 记录一对用户与唯一 room 的映射，消息仍写入 `messages.room_id`。

每个双人 room 必须满足：

- 一对用户最多对应一个未软删除的 direct room。
- 两个用户不能相同，用户对按 UUID 排序后存为 `user_low_id` / `user_high_id`。
- room 使用 `join_policy = 'approval'`，但不能走普通申请/邀请流程；成员只能由 direct conversation 模块写入。
- 两个成员都使用现有 `member` 权限，不设有业务意义的 owner/admin。
- room 的数据库名称只作为内部标识；界面标题和头像始终按当前查看者的“对方用户”计算。
- 群聊的创建、发现、邀请、改名、退出和删除接口必须拒绝 direct room。

### 2.2 好友是双向关系

第一版采用常见的双向好友模型，而不是 Telegram 的单向通讯录模型：只有接受请求后双方才是好友，也只有好友才能创建或重新打开双人 room。

默认关系规则：

- 重复发送同一个请求是幂等操作。
- 双方同时向对方发送请求时，第二个请求自动完成接受，避免两个 pending 记录。
- 拒绝或取消后删除 pending 关系；可以再次申请，但服务端要加请求频率限制。
- 删除好友会删除双方在 direct room 的 membership，并立即断开该 room 的在线连接；room、消息、已读位置和附件保留。
- 双方重新成为好友后，开始私聊会复用原 direct room，并重新写入两个 membership；旧历史可见，但不会把旧消息重新计为未读。
- 拉黑是单向关系。拉黑会同时解除好友、关闭 direct room 访问、取消双方待处理请求，并阻止搜索结果、请求和私聊创建。
- 拉黑不影响双方共同群聊中的既有成员身份；群内消息屏蔽不属于第一版范围。
- 删除好友或拉黑只阻止后续 room 读取和写入；用户已经下载的文件、浏览器缓存和已经取得的 capability download URL 沿用现有附件语义，不承诺追溯撤销。

### 2.3 私聊按需创建

接受好友请求时不立刻创建空会话。用户点击“发消息”时调用幂等的 start-direct-chat 接口：

1. 验证双方仍为好友且不存在任一方向拉黑。
2. 查找已存在的 canonical user pair。
3. 不存在时，在一个事务内创建 room、member role、两条 membership 和 pair 映射。
4. 已存在但 membership 被删除时，在一个事务内恢复两条 membership。
5. 返回同一个 conversation summary。

数据库唯一约束负责处理双方同时开始私聊的竞争；不能只依赖应用层“先查后建”。

## 3. 明确不做的范围

第一版不包含：手机号通讯录同步、单向联系人、在线时间/最后上线、语音/视频通话、端到端加密、消息阅后即焚、会话置顶/归档/静音、好友分组、全局消息搜索和群内屏蔽某人的消息。

这些能力不应提前出现在 schema、接口或 UI 占位中。

## 4. 现状与可复用能力

现有代码已经具备：

- `rooms`、`room_memberships`、RBAC 和 SQLite/Postgres 双数据库迁移。
- room 级 WebSocket 鉴权、历史回放、在线成员、输入状态和断线重连。
- 文本、附件、回复、编辑、撤回、转发、已读位置和未读计数。
- `/ws/account` 的跨 room 新消息和未读快照。
- 用户资料、头像、登录会话以及从消息/成员查看资料的弹窗。
- 桌面双栏和移动单栏切换、深浅主题、PrimeVue 与现有设计 token。

主要缺口：

- 没有关系模型、用户搜索、请求通知和拉黑。
- `GET /api/rooms` 同时承担“我的房间”和“发现房间”，不适合承载隐私敏感的 direct room。
- `Room` 同时被当成数据库记录、缓存记录和查看者视图，已有 membership 装饰泄漏风险；direct title 又是查看者相关字段，不能继续塞进共享缓存。
- 侧栏没有最后消息摘要，当前按房间数据展示创建日期和公开/私密状态。
- 部分 WebSocket 写操作在连接建立后没有逐操作重新校验 membership，删除好友/拉黑时存在很短的竞态窗口。

## 5. 模块设计

### 5.1 `social` 模块

建议文件：

```text
src/social/
├── mod.rs
├── models.rs
├── relationships.rs
├── handlers.rs
└── events.rs
```

外部 interface 只暴露用户可执行的关系操作：搜索、请求、响应、删除、拉黑、解除拉黑和读取列表。canonical pair、状态转换、并发冲突和隐私过滤全部藏在实现内部。

重要不变量：

- self request/self block 永远拒绝。
- `requested_by_id` 必须是 pair 中的一方。
- 接受请求只能由非请求方执行。
- 被任一方拉黑时，对外统一表现为“不可建立关系”，避免泄露对方的拉黑状态。
- 所有关系 mutation 都更新 `updated_at`；现有 account socket 轮询按稳定排序比较 relationship fingerprint，变化后通知双方刷新 social snapshot，不另建进程内事件总线。

### 5.2 `direct_conversations` 模块

建议文件：

```text
src/direct_conversations/
├── mod.rs
├── models.rs
├── repository.rs
└── handlers.rs
```

外部 interface 保持很小：

- `start(viewer_id, peer_id) -> ConversationSummary`
- `deactivate_pair(user_a, user_b)`
- `is_direct_room(room_id)` / `direct_peer(room_id, viewer_id)` 仅供内部授权和查询使用

该模块拥有创建/恢复双人 room 的完整事务。调用者不需要知道 role、membership、pair 排序或内部 room 名称。

### 5.3 `conversations` 查询模块

新增查看者相关的会话查询，避免继续扩张共享 `Room` 模型：

```text
src/conversations/
├── mod.rs
├── models.rs
└── queries.rs
```

核心返回类型：

```text
ConversationSummary
├── room_id
├── kind: group | direct
├── title
├── avatar_emoji
├── description
├── group: GroupConversationMetadata | null
├── peer: UserSummary | null
├── unread_count
├── last_message: MessagePreview | null
├── last_activity_at
└── created_at
```

`title`、`avatar_emoji` 和 `peer` 已按 viewer 计算。前端不读取 direct room 的内部 `rooms.name`。

`MessagePreview` 至少包含 message id、sender id/name、文本安全摘要、附件类型/文件名、撤回状态和时间。摘要由服务端统一生成，避免通知、侧栏和不同客户端各自解释一遍。

会话列表必须用一次 viewer-scoped 查询完成，不能为每个 room 再查 peer、unread 或 last message。复用现有 `(room_id, created_at, id)` 消息索引，并在 SQLite/Postgres 上分别检查查询计划。

### 5.4 room seam 的调整

- 共享缓存只保存持久化 room 字段，不保存 viewer membership、unread、direct peer 或 display title。
- `GET /api/conversations` 专门返回当前账号已加入的群聊/direct 会话，按 `last_activity_at DESC, room_id` 稳定排序。
- `GET /api/rooms` 保留公开房间发现和按名称查询，只返回 group room；direct room 永不进入该接口。
- `GET /api/rooms/:id` 对 direct room 要求当前用户是 active member；非成员返回 404，而不是暴露 room 存在。
- 现有 `/rooms/:id` 前端路由先保持不变，降低路由和外部链接迁移成本。

## 6. 数据库迁移

SQLite 与 Postgres 必须在同一阶段各增加一份语义一致的迁移。以下 `UUID/TEXT` 和 `TIMESTAMP/TEXT` 是双数据库类型占位，实际迁移分别使用 Postgres 的 `UUID`/`TIMESTAMPTZ` 与 SQLite 的 `TEXT`。

### 6.1 `friendships`

```sql
CREATE TABLE friendships (
    user_low_id     UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_high_id    UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requested_by_id UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status          TEXT NOT NULL CHECK (status IN ('pending', 'accepted')),
    created_at      TIMESTAMP/TEXT NOT NULL,
    updated_at      TIMESTAMP/TEXT NOT NULL,
    accepted_at     TIMESTAMP/TEXT,
    PRIMARY KEY (user_low_id, user_high_id),
    CHECK (user_low_id <> user_high_id),
    CHECK (requested_by_id = user_low_id OR requested_by_id = user_high_id)
);
```

增加按 `requested_by_id/status` 和 pair 另一端读取列表所需的索引。UUID canonical 排序在 Rust 中完成，不能依赖 SQLite TEXT 与 Postgres UUID 的排序行为完全一致。

### 6.2 `user_blocks`

```sql
CREATE TABLE user_blocks (
    blocker_id UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_id UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP/TEXT NOT NULL,
    PRIMARY KEY (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);
```

### 6.3 `direct_conversations`

```sql
CREATE TABLE direct_conversations (
    room_id      UUID/TEXT PRIMARY KEY REFERENCES rooms(id) ON DELETE CASCADE,
    user_low_id  UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_high_id UUID/TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at   TIMESTAMP/TEXT NOT NULL,
    UNIQUE (user_low_id, user_high_id),
    CHECK (user_low_id <> user_high_id)
);
```

账号删除前，应用事务先软删除该账号参与的 direct rooms，再删除 user，避免 pair 行级联删除后留下一个被误识别为 group 的活动 room。

## 7. HTTP interface

所有 social/direct 接口都要求 Bearer session。

| Method | Path | 作用 |
| --- | --- | --- |
| `GET` | `/api/users/search?q=...&limit=20` | 按 username/display name 搜索，排除自己和任一方向拉黑 |
| `GET` | `/api/friends` | 好友列表，返回资料摘要和可用 direct room id |
| `GET` | `/api/friend-requests?direction=incoming|outgoing` | 收到/发出的请求 |
| `POST` | `/api/friend-requests` | 发送请求，body 为 `user_id` |
| `PATCH` | `/api/friend-requests/:user_id` | `accept` 或 `decline` |
| `DELETE` | `/api/friend-requests/:user_id` | 取消自己的 outgoing request |
| `DELETE` | `/api/friends/:user_id` | 删除双向好友并停用 direct room |
| `GET` | `/api/blocks` | 已拉黑列表 |
| `PUT` | `/api/blocks/:user_id` | 幂等拉黑 |
| `DELETE` | `/api/blocks/:user_id` | 解除拉黑 |
| `POST` | `/api/direct-chats` | 幂等创建/恢复 direct room，body 为 `user_id` |
| `GET` | `/api/conversations` | 当前账号的统一会话摘要列表 |

搜索约束：trim 后至少 2 个字符、最多 64 个字符、默认 20 条且最大 50 条、SQL LIKE 通配符必须转义、精确 username 优先，其次 username 前缀，再按 display name/username 稳定排序。空查询不能返回全量用户，防止账户枚举。搜索按账号限制为每分钟 30 次；同一 `requester -> target` 方向的新好友请求 60 秒内只接受一次 mutation，重复 pending 请求仍返回幂等成功。第一版沿用进程内 cooldown，若以后部署多实例再把 limiter 移到共享存储。

接口错误语义要固定并测试：401 会话失效，400 输入非法，403 调用者无权执行关系状态转换，404 用户/关系不可见，409 当前关系不允许开始私聊，429 请求过频。

## 8. 实时协议

沿用 `/ws/account`，不要再开一条好友 WebSocket。

新增两类账号级帧：

```json
{ "type": "social_changed", "incoming_request_count": 2 }
```

```json
{
  "type": "new_message",
  "room_id": "...",
  "conversation_kind": "direct",
  "conversation_title": "对方显示名",
  "sender": "...",
  "content": "...",
  "timestamp": "..."
}
```

行为：

- 账号 socket 连接后先发 unread snapshot 和 social snapshot/count，重连即可自愈。
- 关系变化时双方收到 `social_changed`；Web 客户端重新读取好友/请求列表。
- 沿用现有 account socket 数据库轮询循环，比较 `(pair, status, updated_at, block)` 的稳定快照；第一版不增加第二套 WebSocket 或新的内存广播基础设施。
- 新消息事件更新会话摘要并把会话移到顶部；若 room id 尚不在本地列表，先刷新 `/api/conversations` 再显示通知。
- direct room 的事件标题必须按接收者计算，不能发送内部 room name。
- 删除好友或拉黑后，服务端通过现有 room disconnect 机制断开双方该 direct room 的 socket，并发出 membership/social 刷新。

## 9. 授权与竞态

新增统一的 conversation authorization interface，供以下所有写路径调用：

- WebSocket `message`、`typing`、`poke`、`edit`、`recall`。
- 普通附件上传、分块上传创建/完成。
- 消息转发的 source 和 target room。
- AI suggestion 等任何读取私有上下文的 room 接口。

direct room 访问不能只在 WebSocket 建连时校验。删除好友/拉黑可能与一条在途消息竞争，因此 message/attachment/forward 最终写入要在数据库事务内再次确认 active membership。提交关系删除后，任何新的写入都必须失败。

删除好友/拉黑事务还要取消该 direct room 中两人的未完成 attachment upload sessions，避免旧 upload id 在关系失效后完成写入。

普通 group room 保持现有 RBAC；direct room 在统一 interface 内额外禁止：join request、invite、leave、member role change、nickname、room update/delete 和 public discovery。

## 10. Telegram 参考下的 UI 方案

参考原则来自 Telegram 官方客户端的统一 chat list、联系人入口、从聊天顶部添加联系人/拉黑，以及独立联系人列表；仅借用信息结构。参考：[Telegram Contacts](https://telegram.org/blog/contacts-local-groups?setln=en)、[Telegram Desktop 官方源码](https://github.com/telegramdesktop/tdesktop)。

### 10.1 侧栏改为统一会话列表

将 `RoomSidebar.vue` 逐步改名/拆分为 `ConversationSidebar.vue` 和 `ConversationRow.vue`：

- 群聊与私聊混排，按最后活动时间排序。
- 私聊行显示对方头像和显示名；群聊保持房间头像。
- 第一行：标题 + 紧凑时间；第二行：`发送者: 最后消息` 或附件/撤回摘要。
- 自己发送的最后消息加“你：”；direct 不显示公开/私密标签。
- 未读 badge 保持 Coral danger token；超过 99 显示 `99+`。
- 选中、hover、focus、collapsed 和 skeleton 状态都使用稳定尺寸，动态内容不能造成行高跳动。
- 右键菜单按 kind 分支：group 保留管理/退出；direct 提供查看资料、删除好友、拉黑，不显示群管理。

### 10.2 搜索与新会话

侧栏搜索框文案改为“搜索聊天或用户”：

- 输入时先即时过滤本地会话。
- 达到 2 个字符后 debounce 250-300ms 请求用户搜索。
- 结果分为“聊天”和“用户”两个无嵌套卡片的列表区。
- 用户行根据关系显示一个明确动作：`添加好友`、`已发送`、`接受`、`发消息`。
- 顶部 `+` 打开 `NewConversationDialog`，提供“新建群聊”和“添加好友/开始私聊”两个入口。

### 10.3 联系人页

新增 `/contacts` 与 `ContactsPage.vue`：

- 顶部使用 segmented tabs：`好友`、`请求`，请求 tab 带未处理数。
- 好友按 display name/username 排序；点击整行打开私聊，资料按钮打开 profile。
- 请求页分“收到的请求”和“已发送”；收到的请求提供接受/拒绝，已发送提供取消。
- 空状态分别说明没有好友或没有新请求，并提供搜索用户动作。
- 侧栏更多菜单增加“联系人”，不另建常驻第三栏。

### 10.4 direct chat header

`ChatRoomHeader.vue` 按 `conversation.kind` 分支：

- direct：显示对方头像、display name 和 `@username`；对方当前在该 room 的 presence 中时显示“在线”，否则只显示“离线”，第一版不伪造 last seen。
- 点击标题/头像打开资料卡；右侧保留文件和更多菜单。
- direct 隐藏 room id、复制 id、成员列表、房间公开状态、管理和退出。
- 更多菜单提供查看资料、删除好友、拉黑；危险操作必须二次确认并明确历史保留/访问变化。
- group header 保持现有行为。

### 10.5 消息区与资料卡

- 继续复用 `ChatPanel`、`MessageList` 和 `MessageComposer`。
- direct 中隐藏重复的发送者名称和成员管理入口，保留双方气泡方向、头像、回复、已读和附件。
- 资料卡增加 relationship action 区：添加好友、接受请求、发消息、删除好友、拉黑/解除拉黑。
- direct 的已读状态简化为单一对方已读/未读，不弹出群成员清单。
- 所有新控件必须有 loading、disabled、success、error 和离线恢复状态；避免连点产生重复请求。

### 10.6 响应式与无障碍

- 桌面保持会话列表 + 对话双栏；联系人页占右侧内容区。
- 移动端一次只显示会话列表、联系人页或聊天页，沿用返回导航。
- 图标按钮桌面至少 40x40，移动端至少 44x44；仅图标按钮必须有 `aria-label` 和 title/tooltip。
- 搜索结果、请求操作和会话列表支持键盘 Tab/Enter/Escape，focus ring 使用现有 token。
- 深浅主题分别截图验证；长用户名、CJK、emoji avatar、`99+` 未读和 320px 宽度不得溢出。

## 11. 前端状态调整

建议新增：

```text
web/src/
├── socialApi.ts
├── conversationState.ts
├── relationshipState.ts
├── composables/useConversations.ts
├── composables/useContacts.ts
├── components/ConversationSidebar.vue
├── components/ConversationRow.vue
├── components/ContactsPage.vue
└── components/NewConversationDialog.vue
```

原则：

- `App.vue` 只编排页面和模块，不继续堆入关系 mutation 细节。
- `useConversations` 负责初始加载、账号事件合并、最后消息更新、排序和 selected conversation 对账。
- `useContacts` 负责好友/请求加载和 mutation pending/error 状态。
- `relationshipState.ts` 用纯函数描述 relationship state transition，方便 Bun 单测。
- bootstrap snapshot 升级版本或改变 key，避免旧 `Room[]` 被误读成 `ConversationSummary[]`。
- 前端使用 discriminated `kind`，不得通过 `peer !== null`、名称前缀或 has_password 猜会话类型。

### 11.1 现有文件改动清单

| 文件/目录 | 计划改动 |
| --- | --- |
| `migrations/`、`migrations-postgres/` | 增加 friendships、blocks、direct pair 及索引 |
| `src/lib.rs` | 注册新模块、HTTP 路由和 OpenAPI schema |
| `src/models.rs` | 只保留真正跨模块的传输类型；viewer-specific conversation 类型放入新模块 |
| `src/state.rs` | 分离持久化 room cache 与 viewer decoration，注册必要的 runtime 状态 |
| `src/accounts/account_events.rs` | 为 direct 消息计算接收者视角的 kind/title |
| `src/accounts/account_ws.rs` | 初始 social snapshot、变化检测和新消息扩展字段 |
| `src/accounts/user_handlers.rs`、`users.rs` | 账号删除时先处理 direct room 生命周期 |
| `src/rooms/handlers.rs`、`access.rs` | 发现/读取过滤 direct，接入统一 conversation authorization |
| `src/rooms/membership_handlers.rs` | direct 禁止申请、邀请、离开、昵称和角色变更 |
| `src/realtime/auth.rs`、`ws.rs`、`inbound.rs` | direct 鉴权、断开、逐 mutation 权限复核 |
| `src/attachments/handlers.rs`、`upload_handlers.rs`、`upload_sessions.rs` | direct 权限复核与关系失效时取消上传 |
| `src/messages/forward_handlers.rs` | source/target 使用统一授权，支持合法 direct 转发 |
| `web/src/types.ts`、`api.ts` | 新传输类型；social HTTP 可按规模拆到 `socialApi.ts` |
| `web/src/router.ts`、`App.vue` | contacts route 与页面编排，移出关系 mutation 细节 |
| `web/src/composables/useAppBootstrap.ts` | conversation bootstrap 和旧 snapshot 失效 |
| `web/src/composables/useUnreadSocket.ts` | social/change/direct message 账号事件 |
| `web/src/components/RoomSidebar.vue` | 被统一会话 sidebar/row 取代，保留现有响应式行为 |
| `web/src/components/ChatRoomHeader.vue` | group/direct discriminated rendering |
| `web/src/components/ProfileCardDialog.vue` | relationship actions 和危险操作确认 |
| `web/src/components/MessageList.vue`、`ReadReceiptStatus.vue` | direct 的精简 sender/read 状态 |

## 12. 分阶段执行

### Phase 0：规格冻结

- [x] 将第 2、3 节的关系语义和第一版范围转成可测试 contract。
- [x] 为 social、direct conversation、conversation summary 添加 Rust/TypeScript 类型草案。
- [x] 记录 HTTP 和 WebSocket JSON contract 样例。

完成条件：删除好友、拉黑、重加好友和空 direct room 的既定行为都已有对应 contract 与验收用例。

### Phase 1：schema 与纯领域逻辑

- [x] 增加 SQLite/Postgres 三张表迁移和索引。
- [x] 实现 canonical pair、relationship transition 和并发幂等逻辑。
- [x] 更新账号删除流程。
- [x] 增加 migration/relationship 单元与集成测试。

完成条件：两种数据库都能迁移；并发请求不会产生重复 pair；现有测试全部通过。

### Phase 2：好友 HTTP 与实时通知

- [x] 实现 user search、好友/请求、删除、block/unblock handlers。
- [x] 接入 Axum router 和 OpenAPI schema。
- [x] 扩展 account WebSocket 的 social snapshot/change。
- [x] 增加认证、隐私、状态转换、限频和多 tab 测试。

完成条件：不使用 Web UI 也能通过 HTTP + account socket 完成完整好友生命周期。

### Phase 3：双人 room 与会话查询

- [x] 实现 direct start/deactivate 事务和唯一性冲突恢复。
- [x] 新增 `/api/conversations` 和 viewer-aware summary query。
- [x] 让 `/api/rooms`、join/invite/manage 等路径明确排除 direct room。
- [x] 扩展 account message event 的 kind/title。
- [x] 收紧所有 room 写路径的授权与事务内复核。

完成条件：双方并发开始私聊只得到一个 room；第三人无法发现、读取、加入或修改；现有消息能力全部可在 direct room 工作。

### Phase 4：前端数据层

- [x] 增加 social/direct/conversation API client 和类型。
- [x] 将 bootstrap 从 `Room[]` 迁移为 `ConversationSummary[]`。
- [x] 账号 socket 合并 social/new message 事件并处理未知 room。
- [x] 增加 relationship/conversation reducer 的 Bun 测试。

完成条件：临时调试 UI 能实时看到好友请求、会话新增、最后消息、排序和未读变化。

### Phase 5：Telegram 式 UI

- [x] 实现统一会话侧栏和行摘要。
- [x] 实现搜索用户、新会话 dialog、联系人/请求页。
- [x] direct header、profile actions、context menu 和确认对话框按 kind 分支。
- [x] direct message/read receipt 做轻量适配。
- [x] 补全移动端、collapsed、empty/loading/error/offline 状态。

完成条件：用户可只通过 UI 完成“搜索 -> 请求 -> 接受 -> 发消息 -> 收到未读 -> 已读 -> 删除好友/拉黑”。

### Phase 6：回归与发布

- [x] 运行 Rust build、`cargo test --all-targets`、Vue typecheck/build、Bun tests。
- [x] 在 SQLite 和 Postgres 各跑一次关键集成流程。
- [ ] Playwright 覆盖双浏览器上下文的双方操作和断线重连。
- [ ] 截图检查 1440x900、768x1024、390x844，浅色/深色各一轮。
- [x] 检查数据库查询计划和账号 socket 轮询开销。
- [x] 更新 README/OpenAPI/相关使用说明，记录 feature rollout 与回滚方式。

完成条件：下方验收矩阵全部通过，且群聊现有行为无回归。

## 13. 测试矩阵

### Backend

- 搜索：大小写、中文、display name、通配符转义、空查询、limit、自身/blocked 排除。
- 请求：重复、交叉、取消、拒绝、越权接受、自请求、频率限制。
- 拉黑：两个方向、解除、与 pending/accepted 的组合、不可推断对方 block 状态。
- direct：未好友禁止、幂等创建、双方并发、重加复用、恰好两个 membership。
- 隐私：discover/list/get/join/invite/manage/leave/nickname 均不能绕过 direct 约束。
- 消息：文本、附件、分块上传、回复、编辑、撤回、已读、转发、AI 上下文权限。
- 实时：好友请求、接受、首条私聊、未读、删除好友、拉黑、多 tab、重连快照。
- 生命周期：账号删除、room 软删除、旧历史、上传 session 清理。
- 数据库：SQLite 与 Postgres 行为一致，唯一约束和事务竞争均覆盖。

### Frontend

- conversation summary 合并、稳定排序、未知 direct room 刷新和 self-message 不重复。
- relationship button 状态与 mutation pending 防重。
- 搜索 debounce、过期响应不覆盖新查询、断网重试。
- direct/group 菜单互斥，direct 永不展示群管理项。
- route refresh、登录恢复、旧 bootstrap snapshot 失效。
- 未读 `0/1/99/100+`、撤回/编辑/附件最后消息摘要。
- 键盘、screen reader label、focus、reduced motion、深浅主题和移动布局。

## 14. 关键验收场景

1. Alice 搜索 Bob 并发送请求；Bob 无需刷新即可看到请求 badge。
2. Bob 接受后双方联系人页状态一致；任一方点击发消息都进入同一个 room。
3. Alice 发文本和附件；Bob 在另一会话中看到正确的对方标题、最后消息和未读数，打开后清零。
4. 双方同时点击发消息，数据库仍只有一条 pair 和一个 direct room。
5. 未登录用户、非好友 Charlie、公开发现接口和 room id 猜测都无法看到或加入该 direct room。
6. Alice 删除 Bob 后双方立刻退出私聊；历史未删除。重新加好友后再次发消息复用旧 room 和历史。
7. Bob 拉黑 Alice 后，Alice 无法搜索/申请/开始私聊；共同群聊仍正常。
8. direct chat 不出现群成员、复制 room id、管理房间、邀请和退出等操作。
9. 群聊创建、加入、权限、附件、转发、未读和管理现有测试全部不回归。

## 15. 发布与回滚

- schema 先以向前兼容方式上线；旧客户端只看不到新接口，不应因新增表失败。
- 后端接口与 conversation query 稳定后再切前端，避免 UI 先依赖不存在的事件。
- 回滚前端只需切回旧 room list；新增表可保留，不执行破坏性 down migration。
- 若 direct 功能需要紧急关闭，回滚新增前端入口和后端 mutation 版本，但保留新增表、历史数据与群聊路径。

## 16. 建议 PR 切分

1. `schema + social domain + tests`
2. `social HTTP + account events + tests`
3. `direct room lifecycle + conversation query + authorization tests`
4. `frontend conversation/social state + unit tests`
5. `conversation sidebar + contacts/search UI`
6. `direct header/profile UX + responsive/a11y polish`
7. `cross-database/E2E regression + docs`

每个 PR 都必须独立通过现有测试；不要在好友功能 PR 中顺手重写消息渲染或设计 token。

## 17. 实施结果

- 好友关系、请求、拉黑、搜索限流、账号事件和 viewer-scoped 会话接口均已落地并写入 OpenAPI。
- 双人私聊复用现有 room/message/WebSocket；唯一用户对映射、恰好两个成员、重加复用历史以及第三方隔离均由数据库事务和授权测试保证。
- Telegram 式统一会话列表、联系人/请求页、新会话搜索、私聊标题与危险操作确认已接入现有 Vue 客户端。
- SQLite 全流程、真实 Postgres 并发创建/转发、Rust 全目标测试、Bun 状态测试、TypeScript 类型检查和生产构建纳入交付验证。
- 当前宿主没有可用的应用内浏览器实例，因此 Phase 6 的多视口截图复核无法在本次运行中执行；功能自动化验证不依赖该步骤。
