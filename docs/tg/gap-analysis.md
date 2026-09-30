# 现状与 Telegram 的差距清单

任务卡的事实依据。盘点于 2026-09-30，基线提交 `a16f422`。

图例：**✅ 已有** —— 可用且有服务端契约。**◐ 部分** —— 存在但形态或深度不足。**❌ 缺失**。
「证据」列指向让结论可核查的文件、表或路由。

---

## 1. 消息核心

| 能力 | 状态 | 证据 / 差距 |
| --- | --- | --- |
| 文本消息、历史分页 | ✅ | `messages_v2`、`/api/rooms/:id/messages`、`MessageCursor` |
| 回复 | ✅ | `20260818000005_add_replies_and_read_receipts.sql` |
| 编辑、撤回 | ✅ | `20260818000006`、`edited_at` / `recalled_at` |
| 表情回应 | ✅ | `message_reactions` |
| @提及 | ✅ | `message_mentions` |
| 转发 | ✅ | `/api/messages/forward`、`forwarded_from` |
| 已读状态 | ✅ | `room_reads` |
| 置顶消息 | ✅ | `room_pins`、`/api/rooms/:id/pins` |
| 消息内搜索 / 全局搜索 | ✅ | `/api/rooms/:id/messages/search`、`/api/messages/search` |
| 附件与断点续传 | ✅ | `attachment_uploads`、分片上传、OSS + 本地双存储 |
| 草稿持久化 | ◐ | 仅浏览器本地（`conversationDraftStorage.ts`），**Telegram 草稿是云端同步的** |
| 正在输入 | ◐ | `ChatMessage::Typing` 存在，但只有「输入中」一种；Telegram 有录音中/发送图片中/发送视频中等约 10 种 |
| 相册 / 媒体组 | ❌ | 多张图作为一条消息的马赛克布局，需要 `grouped_id` |
| 语音消息 | ❌ | 录制、波形、变速播放、已听状态 |
| 圆形视频消息 | ❌ | Telegram 的 video note |
| 定时发送 | ❌ | 需要 `scheduled_at` 与投递调度 |
| 静默发送 | ❌ | 发送时不触发对方通知 |
| 自毁计时器 | ❌ | per-chat auto-delete timer（非密聊的那种，普通聊天也有） |
| 投票 / 测验 | ❌ | 匿名、多选、quiz 模式、实时票数 |
| 位置 / 实时位置 | ❌ | 静态位置与限时共享的移动位置 |
| 链接预览 | ❌ | 服务端抓取 OG 数据、缓存、SSRF 防护 |
| 引用片段回复 | ❌ | 引用对方消息的**一段**而非整条（Telegram 2023+） |
| 跨聊天回复 | ❌ | 在 A 聊天里回复 B 聊天的消息 |
| 联系人名片分享 | ❌ | — |
| 消息翻译 | ❌ | — |
| 消息效果与动画 | ❌ | — |

## 2. 聊天类型与社交骨架

**这是最大的结构性缺口。** 当前只有一种 `rooms`，靠 `password_hash` 是否为空区分公开/私密。

| 能力 | 状态 | 证据 / 差距 |
| --- | --- | --- |
| 单聊 | ◐ | `direct_conversations` 是独立表，与 `rooms` 并行，两套读写路径 |
| 普通群 | ✅ | `rooms` + `room_memberships` + RBAC（`room_roles` / `room_permissions`） |
| 超级群 | ❌ | 无 `chat_type`；无 Telegram 的 12 项管理员权限与 10 项默认成员限制 |
| 频道（广播） | ❌ | 无「仅管理员可发言」语义、订阅者数、浏览量、作者签名 |
| 话题（论坛） | ❌ | 无 `is_forum`、无话题列表、消息无 `topic_id` |
| 频道评论区 | ❌ | 需要频道 ↔ 讨论群的 `linked_chat_id` |
| 公开 @username | ◐ | `users.username` 已是 UNIQUE COLLATE NOCASE，可复用为公开句柄；**chat 没有 username** |
| 邀请链接体系 | ◐ | 有 `/api/rooms/:id/invitations` 与 `join-requests`，但缺多链接并存、有效期、人数上限、按链接统计、撤销 |
| 慢速模式 | ❌ | — |
| 成员禁言（带到期） | ◐ | 有 `room_bans`，但没有 per-user 的细粒度限制与到期 |
| 房间发现 | ✅ | `/api/rooms/discover` |
| 好友、拉黑、备注 | ✅ | `friendships`、`user_blocks`、`friend_remarks` |

## 3. 表达力：贴纸 / emoji / GIF

全部缺失，这是 Telegram 辨识度最高的一块。

| 能力 | 状态 |
| --- | --- |
| 静态贴纸（WebP） | ❌ |
| 动态贴纸（TGS = gzip 过的 Lottie） | ❌ |
| 视频贴纸（WebM） | ❌ |
| 贴纸包安装 / 卸载 / 排序 / 归档 | ❌ |
| 最近使用、收藏贴纸 | ❌ |
| 按输入的 emoji 建议贴纸 | ❌ |
| 自定义 emoji（消息内联渲染） | ❌ |
| Emoji 状态（头像旁的动态状态） | ❌ |
| GIF 收藏、搜索、发送、自动播放策略 | ❌ |
| Emoji 选择器 | ✅ `emoji-picker-element` 已在用，可保留 |

## 4. 聊天列表与导航

| 能力 | 状态 | 证据 / 差距 |
| --- | --- | --- |
| 会话列表、置顶、归档、静音 | ✅ | `conversation_preferences`、`ConversationRow.vue` |
| 会话别名 | ✅ | `/api/conversations/:room_id/alias` |
| 聊天文件夹 | ❌ | Telegram 的 folder 带 include/exclude 规则（按类型、按聊天、排除已读/静音） |
| 归档区作为独立伪文件夹 | ◐ | 有归档标记，但没有 Telegram 的归档区入口与未读徽章折叠行为 |
| Saved Messages 作为真实聊天 | ◐ | `favorites` 功能上覆盖，但**不是聊天形态**，没有消息流界面 |
| 全局搜索分栏 | ◐ | 有全局搜索，但缺 Telegram 的 聊天/消息/媒体/链接/文件/音乐/语音 分栏 |
| 单聊内共享媒体侧栏 | ◐ | 有 `/api/rooms/:id/files` 与 `ChatFilesDialog.vue`，但不是 Telegram 的分页侧栏形态 |
| 消息跳转与上下文定位 | ✅ | `/api/rooms/:id/messages/:message_id/context`、`messageDeepLink.ts` |

## 5. 个人资料与设置

| 能力 | 状态 | 证据 / 差距 |
| --- | --- | --- |
| 头像、昵称、简介 | ✅ | `users.display_name` / `signature` / `homepage`、`user_avatar_files` |
| 多头像与头像历史 | ❌ | Telegram 保留头像历史可翻阅 |
| 登录用户名 | ✅ | `users.username` |
| 改密码 | ✅ | `/api/users/me/password` |
| 设备会话管理 | ✅ | `device_sessions`、`DeviceSessionsPanel.vue` |
| 在线状态 / 最后上线 | ◐ | 只有房间在线人数（`online_counts()`），无 per-user last seen，无隐私分级 |
| 隐私设置矩阵 | ❌ | Telegram 对「谁能看到手机号/最后上线/头像/转发署名/邀请我入群」各有 所有人/联系人/无人 三档 **加例外名单** |
| 两步验证云密码 | ❌ | 带提示、恢复邮箱 |
| 主题与聊天背景 | ◐ | 有明暗主题（`useTheme.ts`）与 tokens，缺强调色切换、per-chat 壁纸、自定义主题 |
| 通知例外与自定义声音 | ◐ | 有通知中心与 Web Push，缺 per-chat 例外与声音 |
| 数据与存储 / 自动下载规则 | ❌ | 按网络类型的自动下载策略、缓存管理 |
| 多语言 | ❌ | 界面硬编码中文与英文混排 |
| 二维码名片 | ❌ | — |

## 6. 平台

| 能力 | 状态 | 证据 / 差距 |
| --- | --- | --- |
| Web 客户端 | ✅ | Vue 3 + PrimeVue，92 组件 —— **本计划将其整体废弃重写** |
| PWA | ✅ | `pwa.ts`、`pwaBuild.ts` |
| Web Push | ✅ | `web-push`、`push_notifications` 模块 |
| 桌面客户端 | ◐ | PySide6，能力矩阵见 `docs/client-capability-matrix.md`，M6 决定去留 |
| CLI 客户端 | ◐ | ratatui，仅登录/房间/收发/传文件 |
| 移动端原生 | ❌ | 本期不做，`packages/core` 为其留门 |

## 7. 本项目独有、Telegram 没有的能力

这些**不要在重写中丢掉**，它们是产品差异化，不是技术债。

| 能力 | 位置 |
| --- | --- |
| Room 级 AI 工作区（独立 AI 线程、durable run、引用溯源、房间授权检索） | `src/ai_threads/`、`src/knowledge/` |
| 未读总结 catch-up | `src/ai_threads/catch_up.rs` |
| 决策与待办提取（Extraction Run + 候选确认） | `src/ai_extractions/` |
| Room 级 AI 治理与用量记账 | `src/ai_governance/` |
| 收藏与文件夹协作（含协作者） | `src/favorites/` |
| 房间轻量待办 | `src/tasks/`、`room_tasks` |
| 系统管理员、审计事件、备份与恢复验证 | `src/admin/`、`src/audit/`、`src/backup/` |
| 输入框快捷计算 | `web/src/calculator.ts` → 迁入 `packages/core` |
| 隐私锁屏与伪装模式 | `privacyLock.ts`、`idleDisguise.ts`、`vscodeDisguise*` |

## 8. 差距的量级判断

按实现成本从大到小，这决定了里程碑顺序：

1. **聊天类型骨架**（M2）—— 动核心表，且决定所有 UI 的形状，必须最早。
2. **消息列表渲染**（M1）—— 单点技术风险最高：双向虚拟化 + 动态高度 + 保锚点 + 10 万条消息 60fps。
3. **贴纸系统**（M3）—— 新增媒体管线 + 同屏多 Lottie 的性能问题。
4. **消息类型补齐**（M4）—— 项数多但彼此独立，最适合并行。
5. **设置与组织**（M5）—— 项数多、风险低，适合并行与低成本 agent。
6. **云端草稿、粒度输入状态、last seen 隐私分级** —— 分散在各里程碑的小项，容易被漏掉，已单独立卡。
