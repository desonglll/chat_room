# 决策记录

已锁定决策的理由与推翻流程。**任务里不要重新讨论这些**，除非走了 §0 的流程。

## 0. 推翻一个已锁定决策的流程

1. 在本文件对应条目下追加「重新评估」小节，写明：什么新事实出现了、它如何改变原有权衡、受影响的任务 ID。
2. 提给用户确认。技术栈与范围决策不由 agent 决定。
3. 确认后更新 `docs/tg/README.md` 的「已锁定的决策」表，并在 `docs/tg/board.md` 的失效记录里留一行说明哪些任务需要重做。

只写「我觉得 X 更好」不构成重新评估。必须有原先未知的事实。

---

## D-001 Web 框架用 React，不沿用 Vue

**日期** 2026-09-30 · **确认人** 用户

**结论** React 19 + TypeScript，`packages/web` 从零重写。删除 PrimeVue 全家。

**权衡时掌握的事实**

- 旧 `web/src` 有 92 个 `.vue`、59 个 Vue composable、约 55 个框架无关的 `.ts`、71 个测试文件（其中仅 2 个 import `.vue`）。
- 92 个组件无论换不换框架都要重写（视觉是像素级复刻）。真实的迁移成本在 59 个 composable —— 那里是客户端领域规则的落地处（`useChatSocket`、`useMessageViewport`、`useChunkedUpload` 等），附带约 20 个测试。
- Telegram 官方两个 web 客户端都没用主流框架：Web K 是手写 DOM，Web A 用作者自制的 Teact（因为 React 的 reconciliation 扛不住消息列表）。**框架不是这类应用的成败杠杆，消息列表架构和状态模型才是。**
- Vue 的优势本会是：proxy 响应式对 WS 事件洪流改写深层状态更友好，agent 少一类「只在规模上来后才暴露成卡顿」的 memoization 错误。
- React 的优势：`react-virtuoso` 是双向虚拟列表（反向 + 动态高度 + prepend 保锚点）唯一成熟的实现，而这是全项目风险最高的单点；训练数据最多，多 agent 长期接手最稳；为将来 React Native 留门。

**选 React 的理由** 把最高风险的单点交给生态里最成熟的实现。代价是接受 React 的 memoization 纪律风险，缓解办法是 TG-101 设定并测量明确的性能预算，而不是靠 code review 发现掉帧。

## D-002 移动端「以后要做，现在不开工」

**日期** 2026-09-30 · **确认人** 用户

**结论** 本期只做 Web + PWA，但从第一天起把业务逻辑放进平台无关的 `packages/core`。

**这个决策的唯一硬约束**（`architecture.md` §2，CI 检查）

- `packages/core` 不得 import `react` / `react-dom` / `@tg/ui` / `@tg/web`
- `packages/core` 不得引用 `window` / `document` / `localStorage` / `navigator`；平台能力靠注入接口

TG-605 会写一个 Node 宿主真实验证这两条，而不是止于文档承诺。**破坏这两条，将来接 RN 就要重做一遍。**

## D-003 Room → Chat 立刻重命名

**日期** 2026-09-30 · **确认人** 用户

**结论** M0 的 TG-004/005/006 完成重命名，引入 `chat_type(private/group/supergroup/channel)`。

**为什么是现在** 工作树干净、只有一个 worktree、前端反正重写 —— 这是成本的历史最低点。一旦铺开并行开发，一次全局重命名会撕碎所有在飞的分支，就再也做不了了。

**一个有意保留的不一致** `messages.room_id` 列名**不改**。改它要动 61 个迁移涉及的索引和 30 个模块的查询，收益只是命名整齐。这条记在 `CONTEXT.md` 里，免得后来者以为是漏改。

**一个让这件事变便宜的发现** `direct_conversations.room_id` 本来就是 `rooms(id)` 的外键，所有消息都在同一张 `messages` 表里。单聊本来就是一个 room，`direct_conversations` 只是参与者旁表。所以四种 `chat_type` 的统一几乎是免费的 —— 只需回填标记。

## D-004 频道 / 超级群 / 话题在范围内，且排最早

**日期** 2026-09-30 · **确认人** 用户

**结论** M2，紧随 M1 之后。`chat_type` 字段在 M0 就落地。

**为什么不能后补** 侧栏会话项、消息列表、输入框权限门、成员面板的形状全都取决于 `chat_type`。如果 M1 按「只有群聊」做完 UI，M2 要把它们全部重做一遍。字段先落地、行为后补，是为了让 M1 的 UI 从一开始就按四种类型设计。

**权限的实现方式** 扩展既有 `chat_permissions` 注册表（key → 描述）+ `chat_roles` + `chat_role_permissions`，**不引入并行的 bitflags 体系**。Telegram 的 12 项管理员权限与 10 项成员限制映射为新的 `permission_key` 行。per-user 的时限禁言需要新表 `chat_member_restrictions`，因为它不是角色而是对个体的时限覆盖。

## D-005 贴纸 / 动画 emoji / GIF 在范围内

**日期** 2026-09-30 · **确认人** 用户

**结论** M3。TGS（gzip 过的 Lottie）用 `pako` + `lottie-web`。

**已识别的风险** 同屏多个动态贴纸的性能。TG-301 必须实测三种 lottie 渲染器（svg / canvas / html）并把结论写进 devlog，同时实现视口外暂停、首帧静态占位、超阈值降级。这是 M3 的技术核心，不是顺手做的事。

## D-006 音视频通话、端到端密聊、Bot API 不在范围内

**日期** 2026-09-30 · **确认人** 用户

- **通话** 需要独立 SFU 媒体服务（LiveKit / mediasoup），是另一个工程。`chat.call` 权限 key 在 M2 预留但不实现。
- **密聊** 需要客户端密钥存储与协议层改造。`@noble/hashes` 依赖保留（旧客户端已在用），但不实现密聊。
- **Bot API** 需要新增账号类型与 Mini App 沙箱容器。

这三项不实现，但**不要在数据模型里堵死它们**：`chats` 表的 `chat_type` CHECK 约束将来可以加值，权限注册表可以加 key。

## D-007 构建目录所有 worktree 共享

**日期** 2026-09-30 · **依据** 实测

**结论** `.cargo/config.toml` 指向 `/Volumes/Tuo-APFS/workspace/.cargo-target/chat_room`。

**这不是偏好，是硬约束** 实测 `target/` 已达 **194 GB**，卷上仅剩 456 GB 可用。三个独立 target 装不下。

**接受的代价** Cargo 对构建目录加排他锁，并行 worktree 的构建会串行。逃生口是在单个 worktree 里 `export CARGO_TARGET_DIR=...`，见 `agent-protocol.md` §1。

**附带发现** 194 GB 几乎全是沉积。实测：把构建目录指向全新的共享路径后，一次完整的 `cargo clippy --all-targets` 冷编译只产生 **1.5 GB**，耗时 1m44s。也就是说旧 `target/` 里 99% 以上是历次构建与旧工具链的残留。TG-001 回收它并建立 `cargo sweep` 的月度清理。

**已验证** 共享配置生效：新路径被创建并填充，clippy 通过。CI 与 Docker 的 `CARGO_TARGET_DIR` 覆盖已就位（`.github/workflows/ci-cd.yml` env 块、`Dockerfile` builder 阶段）。

**sccache 未采用** sccache 的价值在于「各 worktree 独立 target 但共享依赖编译缓存」。既然磁盘不允许独立 target，sccache 解决不了锁竞争，装了也没用。若将来磁盘宽裕并改为独立 target，再重新评估。
