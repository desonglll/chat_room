# Telegram Parity 计划

把 Echo Gate 做成一个功能与视觉都对标 Telegram 的即时通讯产品。这是该工程的唯一入口文档。

## 新接手的 agent：按这个顺序读

1. 本文件（范围、技术栈、纪律）
2. `docs/tg/board.md` —— 谁在做什么，现在的真实状态
3. `docs/tg/agent-protocol.md` —— worktree、共享构建目录、开发日志的强制规则
4. `docs/tg/roadmap.md` —— 你那张任务卡
5. `docs/devlog/<你的TASK-ID>.md` —— 前一位 owner 的「Handoff snapshot」一节
6. 仓库既有规则：`AGENTS.md`、`CONTEXT.md`

不要跳过第 5 步。如果那一节写着 `<fill in>`，说明上一位 owner 违反了流程，在 board 上标记它，不要靠读 diff 猜意图。

## 已锁定的决策

这些决策已经定了，不要在任务里重新讨论。要改必须走 `docs/tg/decisions.md` 的流程并通知集成负责人。

| 决策 | 结论 | 影响 |
| --- | --- | --- |
| Web 框架 | **React 19 + TypeScript**，从零重写 | 现有 92 个 `.vue` 与 59 个 composable 全部废弃；约 55 个框架无关的业务 `.ts` 及其测试迁移进 `packages/core` |
| UI 组件库 | **自建**。删除 PrimeVue / `@primeuix/themes` / `tailwindcss-primeui` | Telegram 的气泡尾巴、侧栏动效、贴纸面板无法建立在通用组件库上 |
| 移动端 | 以后要做，现在不开工 | 从第一天起业务逻辑放 `packages/core`，不依赖 DOM，为 React Native 留门 |
| 领域词汇 | **Room → Chat 立刻重命名**，引入 `chat_type` | 工作树干净、前端反正重写，现在是成本最低的时刻；铺开并行开发后就回不来了 |
| 频道 / 超级群 / 话题 | **在范围内**，且排在最早 | 侧栏与消息列表的形状取决于 `chat_type`，后补会迫使 UI 重做 |
| 贴纸 / 动画 emoji / GIF | **在范围内** | TGS 即 gzip 过的 Lottie，用 `lottie-web`；同屏多贴纸的性能是真风险 |
| 音视频通话 | **不在范围内** | 需要独立 SFU 媒体服务，属于另一个工程 |
| 端到端密聊 / Bot API | **不在范围内** | 数据库里保留 `@noble/hashes` 与预留字段，不实现 |
| 构建目录 | 所有 worktree **共享**，见 `.cargo/config.toml` | `target/` 曾达 194 GB，卷上只有 456 GB 可用，独立 target 装不下 |

## 技术栈

**服务端**（不变）Rust 1.97 / Axum 0.7 / sqlx（SQLite + PostgreSQL 双适配）/ tokio。

**Web 客户端**（新建）

| 用途 | 选择 | 为什么是它 |
| --- | --- | --- |
| 框架 | React 19 + TypeScript 5.9 | 双向虚拟列表生态最成熟；训练数据最多，多 agent 接手最稳；为 RN 留门 |
| 构建 | Vite + bun（沿用 `build.rs` 嵌入链路） | 服务端把产物编进二进制的链路一行不用改 |
| 状态 | Zustand，**按域切多个 store** | 一个 store 一个文件 = 多 worktree 不抢同一个文件 |
| 消息列表 | `react-virtuoso` | 反向列表 + 动态高度 + prepend 保锚点，业界唯一成熟实现 |
| 动效 | `motion`（framer-motion） | Telegram 的手感是 spring，不是 ease |
| 贴纸 | `lottie-web` + `pako`（TGS 解 gzip） | Telegram 动态贴纸格式 |
| 样式 | Tailwind 4 + 自建 CSS 变量层 | 变量层承载主题、强调色、聊天背景的运行时切换 |
| 保留自旧客户端 | `emoji-picker-element`、`dompurify`、`marked`、`plyr`、`viewerjs`、`jszip`、`@noble/hashes` | 都是框架无关的 web 组件或纯库 |
| 删除 | `primevue`、`@primeuix/themes`、`tailwindcss-primeui`、`cytoscape`、`vue*` | — |

## 里程碑

| 里程碑 | 内容 | 并行度 | 完成标志 |
| --- | --- | --- | --- |
| **M0 地基** | 共享构建目录、monorepo、Chat 模型重命名、design tokens、基础组件、`packages/core` | **串行**，不并行 | 新 React 壳能登录、拉会话列表、发一条消息；CI 全绿 |
| **M1 会话骨架** | 虚拟消息列表、三栏布局、气泡系统、输入框、媒体查看器、信息面板 | 3 路 | 单聊与普通群的日常收发体验达到 Telegram 水准 |
| **M2 社交骨架** | 超级群权限、频道广播、评论区、话题、邀请链接、慢速模式 | 3 路 | 四种 `chat_type` 全部可用 |
| **M3 表达力** | TGS 贴纸、贴纸包、自定义 emoji、GIF | 2 路 | 贴纸面板与内联渲染完成 |
| **M4 消息能力** | 语音消息、圆形视频、相册、定时发送、自毁、投票、位置、链接预览、引用片段 | 4 路 | 消息类型覆盖 Telegram 主线 |
| **M5 组织与设置** | 文件夹、归档、Saved Messages、搜索分栏、隐私矩阵、2FA、主题背景、通知例外 | 4 路 | 设置面板与 Telegram 对齐 |
| **M6 收口** | PWA 强化、切换嵌入产物、删除旧客户端、性能压测、`packages/core` 抽离验证 | 串行 | 旧 Vue 客户端删除，压测达标 |

**M0 必须一个人串行做完。** 它会动 `Cargo.toml`、61 个迁移的表名、`src/rooms/`、`routes.rs`、`build.rs` —— 任何并行分支都会被它撕碎。在 M0 合并进 `main` 之前不要创建第二个 worktree。

## 纪律（违反即回退）

- 一个 worktree 一个任务 ID，一个分支，一个 devlog 文件。
- 只写任务卡 `Allowed paths` 列出的路径。要动别人的路径，找集成负责人，不要自己解决。
- 每次会话结束前重写 devlog 的「Handoff snapshot」，哪怕只做了十分钟。
- devlog 和它描述的代码放进同一个 commit。
- 手写源文件 350 行告警、500 行阻断（`scripts/check_file_sizes.py`）。
- SQLite 与 PostgreSQL 迁移必须同一任务内成对创建，版本号与语义名一致。
- 不新建 `utils` / `helpers` / `common` 倾倒场。

## 相关文档

- `docs/tg/decisions.md` —— 上表每条决策的理由，以及推翻它的流程
- `docs/tg/gap-analysis.md` —— 现状与 Telegram 的逐项差距，任务卡的事实依据
- `docs/tg/architecture.md` —— monorepo 结构、Chat 数据模型、API 契约方向
- `docs/tg/roadmap.md` —— 全部任务卡
- `docs/tg/agent-protocol.md` —— 协作与交接的强制流程
- `docs/tg/board.md` —— 实时状态看板
- `docs/product-roadmap-and-agent-plan.md` —— 本计划之前的产品路线图，仍然有效的部分已并入 gap-analysis
