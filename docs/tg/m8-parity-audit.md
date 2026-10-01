# M8 Telegram 对标走查报告（TG-804）

- 日期：2026-10-01；走查人：集成负责人（主线程）
- 基线：`main` 合并 TG-802/803/805/806 之后（`04b71bc`）
- 环境：本 worktree 的服务器二进制（嵌入当时的 web 构建），SQLite；Playwright Chromium 1243
- 数据：3 个账号（Alice、Bob 为好友，Carol 向 Alice 发了好友申请）、私聊、群「产品讨论组」
  （回复、反应、置顶、长消息、图片、PDF、投票、两条链接）、频道「公告频道」、已归档群「旧项目」
- 视口：1440×900 浅色、1440×900 深色、390×844 浅色
- 截图：`/tmp/tg-shots/TG-804/`（d = 桌面，m = 手机），种子与走查脚本在负责人会话的 job 目录，
  步骤可由下文复现
- 对照：Telegram Web A（web.telegram.org/a）的行为与外观（凭对该产品的了解，未同屏对照）
- 范围外：路线图「不在范围内」表与全部 AI 功能

走查中所有流程都能走通，**没有发现 P0（功能损坏）**：23 个界面步骤全部成功，无页面异常、无失败的
业务请求。唯一的错误日志是下面 P2-2 的 403。

## P1 —— 明显不像 Telegram

| # | 问题 | 复现 | 截图 | 可能的文件 |
| --- | --- | --- | --- | --- |
| P1-1 | **消息没有右键菜单**。右键只出现悬浮工具条（表情/回复/⋮），完整菜单要再点 ⋮；Telegram 右键在指针处直接弹出「反应条 + 操作列表」。手机长按同理需核对。 | 群聊中右键任一气泡 | `d03-message-menu.png` | `features/chat/ChatMessage.tsx`、`features/message/messageMenu.tsx`、`features/messageList/MessageListOverlays.tsx` |
| P1-2 | **没有置顶消息条**。服务端有置顶（`GET /api/chats/:id/pins` 返回该消息），菜单也有「置顶」，但聊天头部下方没有 Telegram 的置顶条（点击跳转、多条时循环、可取消置顶）。 | 置顶一条消息后打开该群 | `d02-group-chat.png` | 新建 `features/chat/pinned/*`；挂在 `ChatPane.tsx` 头部下方 |
| P1-3 | **设置面板几何错误**。桌面端覆盖层宽 343 px 而侧栏列 360 px，右侧露出会话列表的蓝色选中条；列表行无左内边距，图标贴边 x=0。手机端宽约 370/390，右侧露出底层。 | 打开 设置（Ctrl+,） | `d12-settings.png`、`m05-settings.png` | `features/settings/shell/settings.css`、`SettingsHost` |
| P1-4 | **手机端聊天的浮层越界与遮挡**。390 px 下右缘露出半个圆形按钮（回到底部按钮/悬浮工具条定位越界）；粘性日期「今天」压在首条可见气泡的文字上。 | 手机宽度打开群聊 | `m02-chat.png` | `features/messageList/messageList.css`、`MessageListOverlays.tsx` |
| P1-5 | **联系人页不像 Telegram**。桌面端是居中卡片 + 一排文字按钮（发消息/备注/删除好友/拉黑），无在线状态/最后上线、无列表内过滤；Telegram 的联系人是左栏列表，按最后上线排序，操作在资料页。（TG-801 遗留的「在线状态、列表内筛选」也在此。） | 主菜单 → 联系人 | `d13-contacts.png`、`m06-contacts.png` | `features/contacts/**` |
| P1-6 | **频道帖子按「自己发出的消息」渲染**：右对齐、蓝色气泡、对勾。Telegram 的频道帖对所有人（含管理员）都左对齐，显示频道头像与名称、浏览数，无已读对勾。 | 打开频道 | `d10-channel.png` | `features/message/MessageBubble.tsx`、`features/channel/*` |
| P1-7 | **全局搜索默认页只搜会话名**。输入「Telegram」在「聊天」页显示「没有找到匹配的会话」，消息命中要切到「消息」页；Telegram 的默认结果同时列出会话与消息。 | 侧栏搜索框输入 Telegram | `d14-search.png` | `features/search/**`、`features/chatList/ChatListPane.tsx`（只读挂载点） |

## P2 —— 打磨

| # | 问题 | 复现 | 截图 | 可能的文件 |
| --- | --- | --- | --- | --- |
| P2-1 | 英文复数错误：群头部显示「1 members」，成员计数文案没有走复数表（订阅者等同类计数需一并核对）。 | 语言切到 English，打开任一群 | `/tmp/tg-shots/TG-806/03-english-cold-boot.png` | `i18n/en/*.ts`（成员/订阅者计数键） |
| P2-2 | 每次加载都请求 `GET /api/admin/overview`，非管理员得到 403 并在控制台报错。应从会话用户信息判断是否系统管理员，或换成不报错的探测。 | 普通账号登录，看控制台 | — | `features/admin/adminApi.ts`（`checkAdmin`） |
| P2-3 | 自己发起的投票气泡纵向间距过大（每个选项约 48 px），比 Telegram 松散很多。 | 群里看投票 | `d02-group-chat.png` | `features/poll/poll.css` |
| P2-4 | 图片解码失败时显示浏览器的破图标 + 文件名；Telegram 显示占位块。（本次是种子数据里故意无效的 PNG 触发。） | 发送一个损坏的图片文件 | `d02-group-chat.png` | `features/message/content/*`（图片体） |
| P2-5 | 信息面板「通知」区：声音选择是未加样式的原生 `<select>`，静音时长按钮换行挤在一起。 | 打开信息面板 | `d06-info-links-tab.png` | `features/chatInfo/NotificationRow.tsx`、`chatInfo.css` |
| P2-6 | 会话列表没有右下角的「新建」铅笔浮动按钮（Telegram Web A 用它新建私聊/群/频道），新建入口只在主菜单。 | 看会话列表 | `d01-chatlist-home.png` | `features/chatList/**` |
| P2-7 | 信息面板没有「音乐」分类（TG-803 把音乐归入「文件」）；Telegram 有独立的音乐页。 | 群里发一个 mp3 | — | `src/attachments/file_handlers.rs`（加 `kind=music`）、`features/chatInfo/sharedSources.ts` |

## 已核对、与 Telegram 一致或可接受

会话列表行（头像、标题、时间、对勾、未读徽章、媒体摘要「你: 📊 周会改到周四？」）、归档行、
深色主题配色、回复引用块、反应胶囊、长消息换行与时间戳、文件气泡、附件菜单（图片或视频/文件/
位置/投票/联系人）、信息面板头部与六个分类页（链接页显示域名 + 链接 + 发送者）、手机端单栏
导航与返回、会话切换与列表重排动效（TG-805）。

## 建议拆成的 M9 任务卡（文件归属互不相交）

| 卡 | 内容 | 覆盖 | 归属 |
| --- | --- | --- | --- |
| TG-901 消息右键菜单与置顶条 | 右键/长按在指针处弹出反应条 + 菜单；聊天头部下的置顶条（跳转、循环、取消置顶、实时更新） | P1-1、P1-2 | `features/chat/**`（含新 `pinned/`）、`features/message/messageMenu.tsx` |
| TG-902 面板与手机几何 | 设置面板宽度/内边距对齐侧栏；手机端浮层越界、粘性日期遮挡；会话列表新建浮动按钮 | P1-3、P1-4、P2-6 | `features/settings/shell/**`、`features/messageList/*.css` 与 `MessageListOverlays.tsx`、`features/chatList/**` |
| TG-903 联系人 Telegram 化 | 左栏式联系人列表、在线/最后上线排序、列表内过滤、点行进资料、操作移入资料页 | P1-5 | `features/contacts/**`、`features/profile/**` |
| TG-904 频道帖与搜索默认页 | 频道帖左对齐 + 频道身份；全局搜索默认结果合并会话与消息 | P1-6、P1-7 | `features/message/MessageBubble.tsx` 与 `features/channel/**`、`features/search/**` |
| TG-905 打磨批 | 英文复数、管理员探测不报错、投票间距、图片占位、通知行样式、音乐分类 | P2-1…5、P2-7 | `i18n/**`、`features/admin/adminApi.ts`、`features/poll/*.css`、`features/message/content/*`、`features/chatInfo/**`、`src/attachments/file_handlers.rs` |

另有看板「M1 前的跟进项」里仍未处理的工程卫生项（`packages/core` 的 src/test tsconfig 拆分、
`chatSocket.createSocket` 同步抛错时卡在 connecting、补拉过滤的 RFC3339 字典序比较），建议作为
**TG-906 core 卫生**单独一卡（归属 `packages/core/**`）。AI 相关跟进项（Redis 降级、Qdrant 覆盖）
随 AI 关闭继续搁置。
