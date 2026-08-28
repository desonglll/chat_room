# Echo Gate 产品盘点与多 Agent 开发路线图

> 版本：2026-08-26  
> 适用范围：Rust/Axum 后端、SQLite/PostgreSQL、Vue Web、PySide6 Desktop、Docker 运维  
> 用途：产品决策、任务拆分、Agent 分派、集成与验收的唯一计划来源  
> 状态：建议稿；任何功能进入开发前，由产品负责人确认对应任务卡
> 执行更新（2026-08-27）：FND-001 至 FND-004 已全绿并归档；FND-005
> 已进入交付。第 1.1 节保留为执行前基线，不代表当前门禁状态。参见
> `docs/archive/foundation-fnd-001-004.md`。

## 0. 结论先行

当前项目已经不是一个聊天 Demo。它具备账号、群聊、好友与双人私聊、可靠附件、消息协作、
个人收藏、AI 对话、房间级 RAG、后台运维和多客户端基础。最合适的产品定位是：

**面向小团队和私有社群的可自托管 AI 知识通信空间。**

近期不应该平均投入到更多“聊天软件标配”。最值得做的主线是：

1. 先把当前未提交的大批 AI、搜索、置顶、收藏附件改动恢复到全绿。
2. 补齐高频会话管理：草稿、会话置顶、归档、静音、全局关键词搜索。
3. 建立通知中心和后台推送，让好友请求、入群申请、@提及、回复和 AI 完成可追踪。
4. 把现有 AI 能力产品化为“未读总结、决策与待办提取”，同时保持 Room 隔离和可验证来源。
5. 补齐注册策略、持久管理员身份、设备会话、审计日志和自动备份，达到可信自托管标准。

语音/视频通话、端到端加密、公开动态广场、机器人市场和原生移动端放在远期评估，不进入最近
两个版本。它们会显著扩大协议、基础设施或隐私模型，且会分散当前产品差异化。

## 1. 本次盘点依据

本计划基于当前工作树，而不是只看 `main`：

- 后端：Rust 2021、Axum、Tokio、SQLx，支持 SQLite/PostgreSQL。
- Web：Vue 3、TypeScript、Vite、PrimeVue、Tailwind、Bun tests。
- Desktop：PySide6，复用同一套 HTTP/WebSocket 契约。
- 基础设施：Redis、Qdrant、阿里云 OSS、本地存储、Docker Compose。
- 代码证据：路由、迁移、模型、Web 页面、桌面 README、测试和配置文档。
- 运行证据：服务可在 `127.0.0.1:3000` 启动，公开配置报告 AI 为 ready。
- 浏览器限制：当前执行环境没有可用浏览器实例，因此没有把新的视觉走查结果作为结论依据。

### 1.1 当前验证状态

| 检查 | 结果 | 结论 |
| --- | --- | --- |
| Web 单元测试 | 116 passed | 现有前端逻辑总体稳定 |
| Web 生产构建 | passed | 主包约 530 KB，触发 500 KB 分包警告 |
| Desktop pytest | 8 passed | 桌面基础 API/模型/时间线测试通过 |
| Desktop Ruff | passed | 静态检查通过 |
| Rust library tests | 38 passed, 2 failed | TOON 上下文列契约、RAG 引用提示测试与实现不一致 |
| Rust all-targets | compile failed | `admin_index_sync_test` 调用了已变私有的 `store_message` |
| Rustfmt | failed | 当前 AI/附件/收藏等改动未格式化 |
| Prettier check | failed | `App.vue`、`FavoritesPage.vue`、`mobileLayout.test.ts` 未格式化 |

在上述 P0 问题解决前，不把当前工作树称为可发布状态，也不启动会改相同热点文件的新 Agent。

## 2. 当前产品能力

状态定义：

- **已闭环**：后端接口、主要 UI/客户端路径和自动化测试均存在。
- **工作树开发中**：代码已出现，但尚未通过完整 CI 或仍为未提交文件。
- **规划中**：只有设计/研究文档或部分基础设施，没有完整用户流程。

| 板块 | 状态 | 已有能力 | 主要缺口 |
| --- | --- | --- | --- |
| 账号与资料 | 已闭环 | 注册、登录、退出、改密、注销、头像、昵称、签名、主页、会话过期 | 无注册开关、设备会话列表、全端退出、2FA/SSO |
| 群聊 Room | 已闭环 | 创建、公开/密码、发现、申请加入、邀请、成员角色、昵称、退出、软删除 | 无禁言/封禁、慢速模式、审核日志、会话通知偏好 |
| 好友与私聊 | 已闭环 | 搜索用户、好友请求、接受/拒绝/取消、备注、删除、拉黑、双人私聊 | 无好友分组、在线时间；这些不是近期核心 |
| 实时消息 | 已闭环 | WebSocket、历史分页、断线恢复、乐观发送、回复、编辑、撤回、已读、输入状态、表情反应、@提及、批量选择、转发 | 无跨会话搜索入口、定时消息、结构化消息类型 |
| 会话列表 | 已闭环 | 群聊/私聊混排、最后消息、未读数、会话备注、本地筛选 | 无置顶、归档、静音、通知级别、持久草稿 |
| 附件 | 已闭环 | 图片/视频/文件、分块续传、SHA-256、内容复用、Range、敏感遮罩、文件浏览、OSS 直传与本地回退 | 在线副本、自动故障修复、定时灾备仍在规划 |
| 收藏 | 已闭环/开发中 | 消息收藏、手工笔记、多人协作、乐观版本、转发 | 收藏附件、旧转发关联正在工作树开发中 |
| 消息搜索与置顶 | 工作树开发中 | 房间级关键词查询和上下文接口已存在；搜索弹窗、房间消息置顶正在接线 | 尚未全绿；无跨 Room 关键词搜索 |
| AI 助手 | 已闭环/开发中 | 个人 AI Thread、模型选择、思考开关、持久 Run、流式结果、房间上下文、RAG、引用来源 | 规划器、阶段进度、Trace、来源详情正在工作树开发中；无用户化预设任务 |
| 知识检索 | 已闭环 | 房间级向量、事务 outbox、Qdrant、授权复核、rerank 回退、引用深链、授权图片按需视觉理解 | PDF/Office 等附件正文未抽取；缺少质量评测和成本治理 |
| 管理后台 | 已闭环 | 运行概览、依赖状态、AI 模型、向量探针/重建、全局/单房间锁、保留期清理、Postgres 备份恢复 | 管理员按用户名配置，缺审计日志、定时备份、应用健康端点 |
| 隐私与体验 | 已闭环 | 深浅主题、响应式布局、隐私锁、自动伪装、浏览器通知、房间密码记忆、无障碍基础 | 无 PWA、后台 Push、通知中心、统一产品命名 |
| 客户端 | 已闭环但不等齐 | Web、Qt Desktop、CLI、压力测试工具 | Desktop 尚未覆盖收藏、AI、搜索/置顶、管理等最新 Web 能力 |
| 部署 | 已闭环 | Docker 镜像、Compose、SQLite/Postgres、Redis、Qdrant、OSS 配置 | CI 不检查 Desktop、Clippy、Web build、E2E、文件体量、安全扫描 |

## 3. 产品方向与衡量指标

### 3.1 目标用户

优先服务 5-100 人的小团队、兴趣社群和私有知识小组。他们重视自托管、数据可控、历史可查、
部署成本低，并希望 AI 能基于原始对话给出可追溯回答。

### 3.2 核心用户循环

1. 用户加入 Room，与成员持续交流并分享附件。
2. 未读、提及和请求通过通知中心回到用户。
3. 用户用搜索、置顶、收藏找到原始内容。
4. 用户让 AI 总结未读、提取决策或定位旧信息，并从引用跳回原消息。
5. 管理员通过备份、健康状态、审计和保留策略维持可信运行。

### 3.3 建议的产品指标

只采集聚合运营指标，不采集消息正文、搜索词、AI 提示词或附件名。

| 指标 | 定义 | 用途 |
| --- | --- | --- |
| 首次激活 | 注册后 24 小时内加入/创建 Room 并发送首条消息 | 判断新手流程是否成立 |
| 周活跃 Room | 一周内有 3 名以上成员发言的 Room 数 | 判断协作而非单人试用 |
| 消息成功率 | 已持久化并被客户端确认的发送占比 | 可靠性核心指标 |
| 搜索跳转率 | 搜索后打开原消息的会话占比 | 判断检索是否有效 |
| 通知处理率 | 通知被打开或标为已读的占比 | 判断通知噪音和价值 |
| AI 引用打开率 | 带来源回答中用户打开来源的占比 | 判断“可验证 AI”的价值 |
| 未读总结复用率 | 总结后被收藏/转为待办/再次打开的占比 | 判断 AI 是否进入工作流 |
| 备份新鲜度 | 最近成功备份距当前时间和恢复演练状态 | 自托管可信度 |

## 4. 领域与架构约束

这些约束比具体页面更稳定，所有 Agent 必须遵守：

1. **Room 是授权与知识隔离边界。** 语义向量和 AI 证据不得跨 Room 合并。跨 Room 只先做经授权的关键词搜索；如未来需要跨 Room 语义检索，先修改 `CONTEXT.md` 并写 ADR。
2. **Message 是事实来源。** 搜索索引、通知、收藏、摘要、AI 引用和任务来源都是投影，必须能追溯原消息并处理编辑、撤回和权限变化。
3. **Direct Conversation 是受约束的 Room。** 不建立第二套消息、附件或 WebSocket 栈。
4. **用户偏好属于 Membership/Conversation 视图。** 会话置顶、归档、静音、别名和草稿不能写入共享 Room 元数据。
5. **AI 输出不是事实。** 引用证据仍是不可信输入；回答必须与来源分开保存，来源在读取时重新授权。
6. **模块接口即测试面。** 协议 handler 只做认证、解析、错误映射；复杂行为集中在深模块内。
7. **两种数据库同等支持。** 每个 schema 与查询变化同时覆盖 SQLite/PostgreSQL。
8. **Web 先稳定契约，Desktop 再跟进。** 避免两端同时猜测仍在变化的接口。

建议补充到领域词汇表的术语：User、Session、Room、Membership、Conversation、Direct
Conversation、Message、Attachment、Favorite、Notification、AI Thread、AI Run。`CONTEXT.md`
只记录业务含义和不变量，不记录表名或 Rust 类型。

## 5. 优先级总览

| 优先级 | 目标 | 建议周期 | 发布条件 |
| --- | --- | --- | --- |
| P0 | 恢复绿色基线、收敛当前工作树、拆热点、补 CI | 3-5 天 | 全套静态检查与测试通过 |
| P1 / v0.2 | 会话控制、全局关键词搜索、通知中心、安全基线 | 2-4 周 | Web 用户流程闭环，双数据库通过 |
| P2 / v0.3 | AI 未读总结、轻量待办、PWA Push、审计与自动备份 | 4-8 周 | 来源可追溯、权限复核、恢复演练通过 |
| P3 | 附件正文检索、存储副本、SSO、投票等候选 | 按验证结果 | 单独立项与 ADR |

## 6. P0：绿色基线与当前工作树收口

P0 先串行完成。新的并行 Wave 必须从 P0 的绿色提交创建 worktree。

### FND-001 修复当前 CI 阻断

- **目标**：当前工作树所有既有检查恢复绿色。
- **工作**：
  - 重写 `admin_index_sync_test`，通过公开消息接口或测试支持 adapter 触发写入；不要把 `store_message` 改成公共接口只为测试。
  - 明确 AI 上下文是否已增加附件列，更新实现与 TOON 契约测试为同一事实。
  - 明确 RAG 引用提示的稳定措辞/结构，避免测试只绑定脆弱文案。
  - 运行 Rustfmt 和 Prettier，只格式化本批次已修改文件。
- **验收**：`cargo test --all-targets`、`cargo fmt --all -- --check`、`bun test`、`bun run build`、`bun run format:check` 全通过。
- **所有权**：当前工作树原作者；在其完成前其他 Agent 不改 AI、收藏、搜索、Room 访问相关文件。

### FND-002 收口当前开发中功能

- **目标**：将下列工作树能力拆成可审查的逻辑提交：AI planner/pipeline/progress/trace/source details、房间消息置顶、房间搜索 UI、收藏附件、Room 发现查询、owner leave 行为。
- **工作**：每个功能列出 migration、接口、UI、测试；删除半接线入口；OpenAPI 与双数据库迁移同步。
- **验收**：每项都有端到端行为测试或明确延期，不存在“路由已暴露但 UI/权限/迁移缺一段”。
- **输出**：绿色基线 commit SHA，后续所有 Agent 以它为 base。

### FND-003 拆分阻断与高风险大文件

体量审计当前发现 `ChatPanel.vue` 500 行；`App.vue` 499、`src/lib.rs` 498、
`FavoritesPage.vue` 498、`realtime/ws.rs` 491、`useChatSocket.ts` 490 等已接近阻断线。

- **前端 seam**：
  - `ChatPanel.vue` 保留页面编排；消息选择、拖放和图片预览状态移入具名 composable/子模块。
  - `FavoritesPage.vue` 抽出创建、编辑、协作和转发对话框；页面保留筛选与编排。
  - `MessageComposer.vue` 将草稿/提及与 AI 建议分开，避免继续把所有输入行为塞进一个文件。
  - `useChatSocket.ts` 把连接生命周期与出站命令分开；外部保持一个小 interface。
  - `web/src/api.ts` 逐域迁移到独立 client，新功能不得继续扩张它。
- **后端 seam**：
  - `src/lib.rs` 保留应用装配；路由按 domain 提供 `routes()`，OpenAPI 由集成层汇总。
  - `realtime/ws.rs` 分离会话生命周期、数据库轮询和出站事件映射。
  - `state.rs` 分离构建/依赖装配与运行时 Room channel 状态。
- **验收**：新增或改动的手写源码小于 500 行；被本任务触及的阻断文件降至 350 行附近；接口测试在拆分前后不变。
- **注意**：按职责拆，不创建 `utils`/`helpers` 文件，也不做全仓一次性重构。

### FND-004 增强 CI 与发布门禁

- 增加 `cargo clippy --all-targets -- -D warnings`。
- CI 执行 `bun run build`，保留 chunk size 警告并将主入口拆到 500 KB 以下。
- CI 执行 Desktop pytest、Ruff check、Ruff format check。
- 增加文件体量审计：350 行 warning、500 行 fail；现有警告先建 baseline，新增增长不得越线。
- 增加最小 E2E：注册/登录、建 Room、双用户发消息、上传、搜索、AI disabled/ready 两状态。
- 增加 SQLite 与 PostgreSQL migration parity 检查。
- **验收**：PR 上所有门禁可重复运行；失败给出具体文件/命令，而不是只构建 Docker。

### FND-005 文档与产品命名统一

- 新增根 README：定位、截图、快速启动、架构、配置指针、测试命令。
- 统一 `Echo Gate`、`Chat Room`、`Echo Chat Desktop` 和 Web scaffold 包名的展示规则。
- 将已完成计划归档，配置事实继续以 `docs/configuration.md` 为准，避免复制易过期配置。
- **验收**：新开发者在空机器上能按 README 启动 SQLite 最小环境；文档不包含真实密钥。

## 7. P1：v0.2 核心功能任务卡

P1 先并行构建隔离模块，再由 Integration Agent 接线共享外壳。推荐同时最多 4 个 Agent。

### CONV-101 持久草稿（低成本高频）

- **用户结果**：切换 Room、刷新页面或误关闭后，未发送文字和待发送状态可恢复。
- **第一版范围**：Web 本地按 `user_id + room_id` 保存文字、回复目标和更新时间；不保存文件字节和房间密码；退出账号清除或隔离。
- **实现**：新增 `conversationDraftStorage.ts` 与测试；输入组件通过小 interface 读写，不直接操作全局存储。
- **不做**：第一版不做跨设备同步。验证确有需求后再增加 `conversation_drafts` 表。
- **验收**：账号/Room 严格隔离；撤回/删除的 reply target 恢复时安全降级；无痕/存储失败时仍可正常输入。
- **允许路径**：新 storage/composable/test；输入组件接线由 Integration Agent 完成。

### CONV-102 会话偏好后端

- **用户结果**：每个用户可独立设置会话置顶、归档、通知级别和静音截止时间。
- **模型**：偏好属于 viewer 的 Conversation/Membership，不属于共享 Room。建议字段：
  `is_pinned`、`is_archived`、`notification_level(all|mentions|none)`、`muted_until`、`updated_at`。
- **interface**：`get_preferences(user_id, room_id)`、`update_preferences(user_id, room_id, patch)`；验证 active membership。
- **HTTP**：`GET/PATCH /api/conversations/:room_id/preferences`；更新后的 `ConversationSummary` 携带 viewer 偏好。
- **排序**：未归档会话按 `is_pinned DESC, last_activity_at DESC, room_id`；归档单独查询/展示。
- **验收**：群聊/私聊一致；退出 Room 后不能读写；静音不改变未读数；SQLite/Postgres 语义一致。
- **迁移起点**：`20260901010000`，同名双迁移。

### CONV-103 会话置顶/归档/静音 UI

- **用户结果**：会话行菜单可置顶、归档、设置“全部/仅提及/不通知”和静音时长；侧栏有置顶/最近/归档区。
- **交互**：乐观更新失败回滚；静音和归档有图标而非仅颜色；移动端操作可达。
- **验收**：刷新后状态保留；归档会话收到新消息时遵从既定策略，第一版建议保持归档但更新未读；偏好不泄露给其他成员。
- **依赖**：CONV-102；共享 `RoomSidebar.vue` 由 Integration Agent 接线。

### SRCH-101 全局关键词搜索后端

- **用户结果**：在自己仍有权限的所有活跃 Conversation 中搜索文本消息。
- **范围**：关键词、Room、发送者、日期区间、内容类型筛选；稳定 cursor 分页；结果含最小上下文和 viewer-scoped Conversation 标题。
- **授权**：查询必须先限制 active memberships；Direct Room 对非成员不可见；结果读取时排除撤回内容。
- **interface**：`search_visible_messages(viewer_id, query) -> SearchPage`，隐藏 SQLite/Postgres 查询差异。
- **HTTP**：`GET /api/messages/search`；限制 query 长度、页大小和超时。
- **不做**：不跨 Room 做向量检索，不把多个 Room 的向量证据交给一个 AI Run。
- **验收**：特殊字符与 SQL wildcard 正确；没有 N+1；大结果分页稳定；双数据库查询计划有索引。
- **迁移起点**：`20260901020000`，只在确需新索引时创建。

### SRCH-102 搜索工作区

- **用户结果**：主导航进入独立搜索页，支持筛选、结果摘要、来源 Conversation 和跳回原消息。
- **实现**：新 `GlobalSearchPage.vue`、`globalSearchApi.ts`、查询状态 composable；不扩张 `App.vue` 和 `api.ts`。
- **验收**：URL 可恢复查询/筛选但不把敏感结果正文写入 URL；键盘导航、空状态、加载和错误状态完整；深链沿用现有 message navigation。
- **依赖**：SRCH-101；路由和 Workspace 接线由 Integration Agent 完成。

### NTF-101 持久通知模块

- **用户结果**：好友请求、入群申请、@提及、回复、AI Run 完成不再只靠瞬时弹窗。
- **模型**：`Notification` 包含 recipient、kind、actor、room/message/run 引用、created_at、read_at、dedupe_key；正文只保存最小安全摘要。
- **interface**：`record(event)`、`list(recipient, cursor)`、`mark_read`、`mark_all_read`、`unread_count`。
- **一致性**：事件创建尽量与源 mutation 同事务；不能同事务时使用现有数据库 outbox 模式，不能用进程内内存队列当事实来源。
- **隐私**：读取时重新授权；用户失去 Room 权限后隐藏消息摘要但可保留通用事件记录。
- **验收**：幂等重试无重复；撤回/删 Room/拉黑后的展示符合规则；账号 socket 只推失效信号或新通知 ID。
- **迁移起点**：`20260901021000`。

### NTF-102 通知中心 UI

- **用户结果**：导航 badge、通知列表、按类型筛选、单条/全部已读、跳转来源。
- **实现**：新 `NotificationsPage.vue`、`notificationsApi.ts`、`useNotifications.ts`；Integration Agent 接主导航和 account socket。
- **验收**：跨标签页刷新一致；不可访问来源显示安全文案；badge 与服务端未读数最终一致；不把浏览器通知权限与站内通知开关混为一项。

### SEC-101 注册模式与持久管理员身份

- **问题**：当前管理员权限按配置中的用户名匹配。开放注册环境中，未预先创建的管理员用户名可能被抢注。
- **决策**：此项先写 ADR。推荐增加 `open | invite_only | disabled` 注册模式，并将系统管理员持久化为 `user_id` 角色；配置用户名只作为迁移兼容，不再作为长期授权事实。
- **引导**：设计一次性 bootstrap token 或本地 CLI 授权第一个管理员；已有管理员可授予/撤销，但不能删除最后一名管理员。
- **验收**：大小写/改名不改变授权；抢注配置名不能获得权限；bootstrap 可审计且完成后失效。
- **迁移起点**：`20260901040000`。

### SEC-102 认证限流与安全响应头

- 登录、注册、密码验证按 IP 与账号双维度限流，失败响应避免用户名枚举。
- 为生产环境配置精确 CORS allowlist；补 CSP、frame-ancestors、nosniff、referrer policy。
- 保留 Bearer 模型，记录限流指标但不记录凭据。
- **验收**：正常突发不误伤；多实例部署使用 Redis adapter，单实例有本地 adapter；对应安全测试通过。

## 8. P2：v0.3 AI 工作流、平台与治理

### AI-201 “总结未读”可信工作流

- **用户结果**：在 Room 顶部一键生成从个人已读游标到当前的摘要，包含主题、决定、待确认问题和来源链接。
- **执行**：复用 durable AI Run，但增加受信任的 `purpose=catch_up` 入口，由服务端决定消息边界；客户端不能伪造任意他人已读位置。
- **输出**：先保存到个人 AI Thread，可选择收藏；不自动向 Room 发消息。
- **授权**：Run 开始和来源读取时都验证 active membership；撤回内容不进入新 Run。
- **验收**：0 条未读不调用模型；长历史有有界上下文；引用只显示回答实际使用的来源；失败/刷新后可恢复。
- **依赖**：FND-002 的 AI pipeline 全绿。

### TASK-201 Room 轻量待办

- **用户结果**：从消息手工创建待办，包含标题、负责人、截止时间、状态和原消息链接。
- **模型**：Room-scoped Task；状态 `open | in_progress | done | cancelled`；来源 Message 可为空，删除来源不删除 Task，只移除可见摘要。
- **权限**：成员可创建；负责人/创建者可更新；管理员可管理。具体规则在开发前确认并写领域测试。
- **UI**：Room 内独立待办面板，不把页面堆成卡片仪表盘。
- **验收**：成员离开、消息撤回、Room 删除、负责人被移除等边缘场景有明确结果。
- **迁移起点**：`20260901060000`。

### AI-202 决策与待办提取

- **用户结果**：AI 从选定时间范围提出“候选决定/候选待办”，用户逐项确认后才写入收藏或 Task。
- **实现**：结构化输出 schema + 服务端验证；AI 永不直接改变 Task 状态或分配负责人。
- **验收**：每个候选项带来源；无来源项标明“模型推断”；无权限来源不能落库；重复执行可去重。
- **依赖**：AI-201、TASK-201。

### AI-203 Room 级 AI 治理

- Room owner 可设置 AI 禁用/成员可用/仅管理员可用；部署管理员可设置模型 allowlist、并发和用量上限。
- 记录 token/耗时/模型/状态等聚合用量，不记录提示词或私密证据正文。
- 提供超额、模型不可用、RAG 降级的明确 UI 状态。
- **验收**：禁用后新 Run 被服务端拒绝；运行中的策略变化有明确规则；后台能按 Room/模型看聚合成本。

### PLAT-201 安全 PWA 壳

- 增加 manifest、安装入口和 service worker，仅缓存版本化静态资源。
- 第一版离线状态只显示连接状态和已加载页面，不把消息历史、附件或 token 放入 Cache Storage。
- **验收**：新版本可更新；登出后无敏感缓存；离线/重连不产生重复消息。

### PLAT-202 Web Push

- 在 NTF-101 之上增加 push subscriptions 与 VAPID 配置。
- 通知 payload 默认不含正文；用户明确开启“显示详情”才包含最小摘要。
- 发送失败清理过期订阅；会话静音/通知级别在服务端决定是否发送。
- **验收**：浏览器关闭时可收到；点击深链；撤销权限后停止；多设备订阅独立。
- **依赖**：NTF-101、CONV-102、PLAT-201。

### SEC-201 设备会话管理

- Session 增加设备名、创建/最后使用时间、可选粗粒度 IP 信息；提供列表、单个撤销、撤销其他设备。
- 密码修改默认撤销其他 session，当前 session 是否保留需产品确认。
- **验收**：撤销后 Redis 与数据库同时失效，WebSocket 尽快断开；不暴露完整 IP/UA 给日志。

### SEC-202 审计与房间治理

- 管理审计：管理员授权、系统/Room 锁、备份恢复、模型配置、索引重建、清理操作。
- Room 审计：邀请、审批、角色变更、移除、封禁、解除封禁。
- 审计事件 append-only，敏感 payload 最小化；UI 支持 actor、时间、类型筛选。
- **验收**：审计失败时关键管理 mutation 的策略明确；普通成员不可读取系统审计。

### OPS-201 应用健康与可观测性

- 增加 `/health/live` 和 `/health/ready`；ready 检查数据库和必要依赖，Redis/Qdrant/AI 按配置区分 required/optional。
- Docker 对应用容器使用 readiness healthcheck。
- 增加 request ID、JSON 日志选项和 Prometheus/OpenTelemetry adapter；消息正文不进入 label/log。
- **验收**：依赖降级状态与实际服务策略一致；高基数数据不作为 metric label。

### OPS-202 自动备份与恢复演练

- 为 PostgreSQL 和 SQLite 提供定时备份、保留策略、目标后端、最近结果和校验状态。
- SQLite 使用在线一致性方案，不直接复制正在写的数据库文件。
- 备份清单包含数据库、附件范围和 SHA-256；恢复默认只做验证，执行恢复需二次确认并锁系统。
- **验收**：自动测试从备份恢复到临时实例并校验关键计数；明确 RPO/RTO；失败告警可见。
- **迁移/配置起点**：`20260901050000`。

### DESK-201 客户端能力矩阵与增量对齐

- 先建立 Web/Desktop/CLI 契约矩阵，再按用户价值补：全局搜索、通知中心、会话偏好、收藏、AI。
- Desktop 不复制业务规则，只消费已冻结接口；桌面 UI 任务在对应服务端任务发布后开始。
- CI 强制 Desktop 测试，关键网络行为用 fake HTTP/WebSocket adapter。

## 9. P3 候选与暂缓项

| 候选 | 前置验证 | 为什么不现在做 |
| --- | --- | --- |
| 附件正文检索 | 确认 PDF/Office 搜索需求、文件类型与安全沙箱 | 需要解析器隔离、恶意文件防护、增量索引和引用定位 |
| 在线附件副本/自动修复 | 完成现有 storage plan 的状态机与恢复演练 | 当前本地+OSS fallback 已满足基本可用，副本会放大一致性复杂度 |
| 投票消息 | 确认群决策频率 | 结构化消息会影响协议、收藏、转发、搜索和多客户端 |
| OIDC/SSO | 有组织部署客户 | 需要账号绑定、退出、管理员映射和部署文档 |
| 语音/视频 | 有明确实时会议用户群 | WebRTC、TURN、设备权限、录制和移动网络成本高 |
| 端到端加密 | 先选择“E2EE”还是“服务端 AI/RAG”优先 | 当前服务端搜索、AI、审核和备份无法直接读取密文 |
| 原生移动端 | PWA 数据证明移动留存不足 | 会新增第三套 UI 和发布体系 |
| 机器人/插件市场 | 稳定权限、审计和事件订阅模型 | 未解决权限前开放自动化风险过高 |

## 10. 多 Agent 并行执行方案

### 10.1 角色

| 角色 | 唯一职责 | 不承担 |
| --- | --- | --- |
| Integration Lead | 选绿色 base、分配路径、冻结契约、修改共享热点、集成与发布 | 不与 Feature Agent 同时改其实现文件 |
| Feature Agent | 完成一个任务卡的深模块、迁移、契约与测试 | 不接线共享 App/router/types |
| Web Integration Agent | 在后端契约冻结后接页面、导航、全局状态 | 不改数据库规则 |
| Verification Agent | 在集成分支做黑盒、迁移、权限、E2E 与回归 | 不顺手重构实现 |
| Documentation Agent | 更新 README、配置、领域词汇和发布说明 | 不复制配置文件中可直接读取的事实 |

同一时间建议最多 4 个活跃 Agent：3 个隔离 Feature Agent + 1 个 Integration Lead。更多并发会让
共享模型、路由、App 外壳和迁移排序成为瓶颈，吞吐反而下降。

### 10.2 推荐 Wave

```text
Wave 0（串行）
FND-001 -> FND-002 -> 绿色基线 -> FND-003/FND-004

Wave 1（并行构建独立模块）
CONV-101   CONV-102   SRCH-101   SEC-101
                        |
                     SRCH-102（独立页面，暂不接主导航）

Wave 2（并行）
NTF-101   SEC-102   AI-201   OPS-201

Integration Gate
共享 router/App/types/OpenAPI/account socket 统一接线 + 全套回归

Wave 3（并行）
TASK-201   AI-203   PLAT-201   OPS-202
   |                    |
AI-202              PLAT-202

Wave 4
DESK-201 + P3 需求验证
```

### 10.3 共享热点唯一负责人

并行 Wave 中，以下文件只由 Integration Agent 修改：

| 热点 | 原因 | Feature Agent 的替代做法 |
| --- | --- | --- |
| `src/lib.rs` | 路由和 OpenAPI 总装配 | 在 domain 暴露 `routes()`/handler，提交接线清单 |
| `src/models.rs` | 多领域共享类型 | 类型放在自己的 `models.rs` |
| `web/src/App.vue` | 全局编排与对话框 | 新建 page/composable，通过 props/interface 暴露 |
| `web/src/router.ts` | 所有页面共用 | 提交 route record 建议 |
| `web/src/types.ts` | 容易形成类型垃圾场 | 新类型放 domain API 文件 |
| `web/src/api.ts` | 已接近 500 行 | 新建 `<domain>Api.ts` |
| `Cargo.toml`/lockfiles | 依赖冲突 | 先在任务卡声明依赖，由 Integration Lead 统一添加 |
| `.env.example`/`chat-room.toml` | 配置顺序和文档耦合 | Feature Agent 提交配置字段说明 |
| `CONTEXT.md`/ADR | 领域决策唯一事实 | Domain/Integration Lead 记录已确认决策 |
| `.github/workflows/*` | 门禁顺序与缓存 | FND-004 唯一 owner |

### 10.4 迁移编号分配

每个领域同时创建 SQLite/PostgreSQL 同版本、同语义名称的迁移。现有迁移禁止修改。

| 领域 | 起始版本 | 示例 |
| --- | --- | --- |
| Conversation | `20260901010000` | `..._add_conversation_preferences.sql` |
| Search/Notification | `20260901020000` | `..._add_notifications.sql` |
| AI/Knowledge | `20260901030000` | `..._add_room_ai_policy.sql` |
| Security/Governance | `20260901040000` | `..._add_system_roles.sql` |
| Operations/Storage | `20260901050000` | `..._add_backup_runs.sql` |
| Tasks/Experiments | `20260901060000` | `..._add_room_tasks.sql` |

同一领域的后续迁移从起始版本递增。Agent 在任务开始时声明具体版本；Integration Lead 检查唯一性。

### 10.5 契约先行但不提前造层

需要后端/前端并行时，先冻结一份最小契约：请求、响应、授权、错误码、分页、幂等语义。契约冻结后：

- Backend Agent 实现 domain + handler + OpenAPI annotation。
- Web Agent 用 typed fake 完成独立页面和状态测试。
- Integration Agent 最后替换 fake、接 route 与 account socket。

只有生产 adapter 和测试 adapter 都存在时才引入 port。单一实现不创建 factory/repository wrapper 只为“以后可能”。

### 10.6 分支、提交和交接

1. Integration Lead 公布 `BASE_SHA` 和 Wave 所有权表。
2. 每个任务使用独立 worktree/branch：`agent/<task-id>-<slug>`。
3. 一个提交只包含一个可解释变化；迁移与对应代码/测试不拆散到不可运行状态。
4. Feature Agent 不合并主分支、不推送共享分支、不格式化无关文件。
5. 交接报告必须列：base、改动文件、迁移、契约、验证、剩余风险、共享文件接线清单。
6. Integration Lead 按依赖顺序集成；每次集成后跑 focused tests，Wave 结束跑全量门禁。

## 11. 通用开发准则

### 11.1 正确性与权限

- 所有 Room 派生读取都先验证 active membership；缓存命中也不能绕过授权。
- 用户失去权限、消息撤回或账号拉黑后，搜索、通知、AI 来源和深链同时降级。
- 写操作定义幂等键；WebSocket 重连、HTTP retry 和后台 job 不产生重复事实。
- 不在日志、指标、错误详情中输出 token、密码、capability URL、消息正文、AI 证据或密钥。

### 11.2 数据与迁移

- SQLite/PostgreSQL 查询分别验证，不依赖未声明的方言行为。
- 大表新增非空字段先给安全默认/回填，再收紧约束。
- 后台投影采用数据库 outbox；数据库记录是恢复事实，不把内存 channel 当持久队列。
- 删除采用现有软删除/保留期语义；新增数据必须进入备份、注销和 purge 设计。

### 11.3 前端

- 页面区块保持全宽、工作型和可扫描，不把所有内容做成嵌套卡片。
- 图标按钮使用现有 Lucide/PrimeVue，提供 `aria-label` 和 tooltip/title。
- 每个异步操作有 loading、empty、error、success/rollback 状态。
- 键盘、移动端、深浅主题、reduced motion 与长文本必须在验收中覆盖。
- 新页面优先异步加载；主入口 chunk 保持低于 CI 阈值。

### 11.4 测试

- domain 规则通过模块 interface 测试，不绑定私有 SQL 方法。
- 每个权限功能至少覆盖 owner/admin/member/non-member/removed member。
- 每个投影功能覆盖源消息编辑、撤回、Room 删除、权限丢失和重试幂等。
- UI 测试断言用户可观察行为，不只搜索 CSS 字符串；关键流程补浏览器 E2E。
- 性能任务使用现有 stress 工具给出基线、目标和回归阈值。

## 12. Definition Of Done

单个任务完成必须满足：

- 任务卡的用户结果完整，不留不可用入口或假按钮。
- 授权、验证、错误映射、幂等、并发和兼容行为明确。
- SQLite/PostgreSQL migration 和测试同步。
- OpenAPI、Web typed client、Desktop 契约（如受影响）一致。
- 相关格式化、lint、typecheck、测试和 build 实际运行并记录结果。
- 修改后没有新增 500 行手写源码；触及的超大文件已按职责处理。
- 用户界面在桌面/移动、浅色/深色、键盘/reduced motion 下验证。
- 配置、部署、领域词汇或 ADR 只在确有新事实时更新。
- 交接报告能让 Integration Lead 不猜测地合并。

Wave/版本发布还必须满足：

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cd web && bun run format:check && bun test && bun run typecheck && bun run build
cd desktop && uv run pytest && uv run ruff check src tests && uv run ruff format --check src tests main.py
```

并额外完成：双数据库升级测试、E2E smoke、文件体量审计、依赖/镜像安全扫描、备份恢复演练（涉及
数据或运维的版本）、最终 diff 审查。

## 13. 可直接发给 Agent 的任务模板

```md
# Task <ID>: <名称>

Base commit: <SHA>
Branch: agent/<id>-<slug>
Owner: <agent>
Depends on: <task IDs or none>
Migration version: <reserved version or none>

## Outcome
<一句用户可观察结果>

## Read First
- AGENTS.md
- CONTEXT.md
- docs/product-roadmap-and-agent-plan.md#<task-card>
- <relevant source/tests only>

## Allowed Paths
- <exclusive existing paths>
- <new module directory>

## Frozen Interface
<request/response/interface, authorization, errors, pagination, idempotency>

## Work
1. <implementation step + completion criterion>
2. <test step + completion criterion>
3. <handoff step + completion criterion>

## Out Of Scope
- <explicit non-goals>

## Acceptance
- <behavioral checks>
- <permission/edge cases>
- <commands that must pass>

## Handoff
Report changed files, migrations, interface changes, exact test results,
unresolved risks, and the shared-hotspot integration patch list.
```

## 14. 产品负责人需要确认的决策

这些问题会改变模型或用户行为，开发前应由产品负责人一次性确认：

1. 产品正式名称是否统一为 Echo Gate，Chat Room 是否只作为技术仓库名。
2. 目标部署默认是开放注册、邀请码注册还是关闭注册。
3. 归档会话收到新消息时保持归档，还是自动回到最近列表。本计划建议保持归档。
4. “仅提及通知”是否包含回复、好友/入群请求和 AI 完成。本计划建议系统请求始终独立通知。
5. AI 未读总结是否允许一键发回 Room。本计划第一版只保存到个人 AI Thread。
6. Room Task 的更新权限：负责人/创建者，还是所有成员。本计划建议负责人和创建者可更新。
7. 密码修改后是否保留当前设备 session。本计划建议保留当前、撤销其他设备。
8. 管理员 bootstrap 采用一次性环境 token 还是本地 CLI。本计划偏向本地 CLI，攻击面更小。

在这些决策未确认时，Agent 可以完成隔离的研究、接口备选和测试矩阵，但不应擅自提交难以逆转的
schema 或权限语义。
