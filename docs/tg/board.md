# 任务看板

**全局状态的唯一真相源。只在 `main` 分支上更新，由集成负责人在合并时维护。不要在 worktree 里改这个文件。**

任务详情见 `docs/tg/roadmap.md`。每个进行中任务的细节见 `docs/devlog/<TASK-ID>.md`。

- 盘点提交：`a16f422`
- **绿色基线提交：尚无 —— `a16f422` 的 `check_file_sizes.py` 是红的，见 TG-000**
- 盘点日期：2026-09-30
- 当前里程碑：**M0（串行，禁止并行）**

## 现在可以开工的任务

**先做 TG-000。** 基线不绿，后续任务无法区分「我弄坏的」和「本来就坏的」。

之后 M0 按下表顺序串行推进，**TG-004 合并进 `main` 之前不要创建第二个 worktree。**

`TG-000` → `TG-001` → `TG-002` → `TG-003` → `TG-004` → `TG-005` → `TG-006` → `TG-007` → `TG-008`

`TG-009`（design tokens）不碰 Rust 也不碰 `packages/`，是 M0 里唯一可以真正并行的任务，TG-000 之后即可与其余任务同时进行。

## M0 地基

| 任务 | 规模 | 状态 | Owner | Worktree | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-000 修复红色基线 ← **先做这个** | S | not-started | — | — | — |
| TG-001 回收构建目录并验证共享配置 | S | not-started | — | — | TG-000 |
| TG-002 monorepo 骨架 | M | not-started | — | — | TG-001 |
| TG-003 `build.rs` 切换嵌入目标 | S | not-started | — | — | TG-002 |
| TG-004 Chat 数据模型迁移 | L | not-started | — | — | TG-001 |
| TG-005 Rust 模块与类型重命名 | L | not-started | — | — | TG-004 |
| TG-006 API 路径重命名与 alias | M | not-started | — | — | TG-005 |
| TG-007 WebSocket 帧扩展 | M | not-started | — | — | TG-005 |
| TG-008 云端草稿 | M | not-started | — | — | TG-006, TG-007 |
| TG-009 Design tokens 提取 | M | not-started | — | — | — |
| TG-010 `packages/ui` 基础组件 | L | not-started | — | — | TG-002, TG-009 |
| TG-011 `packages/core` 骨架与逻辑迁移 | L | not-started | — | — | TG-002, TG-006, TG-007 |
| TG-012 登录与最小可用壳 | M | not-started | — | — | TG-003, TG-010, TG-011 |

**M0 完成标志** 新 React 客户端能完成 注册 → 建群 → 发消息 → 刷新保留 → 第二浏览器实时收到；CI 全绿；共享构建目录体积记录在案。

## M1 会话骨架（3 路并行，M0 完成后开放）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-101 虚拟消息列表 ← 最高风险 | XL | A | blocked | — | TG-012 |
| TG-102 三栏布局与会话侧栏 | L | B | blocked | — | TG-012 |
| TG-103 消息气泡系统 | L | C | blocked | — | TG-012 |
| TG-104 输入框 | L | B | blocked | — | TG-102, TG-008 |
| TG-105 媒体查看器 | M | C | blocked | — | TG-103 |
| TG-106 右侧信息面板 | M | A | blocked | — | TG-101 |
| TG-107 粒度输入状态与在线状态 UI | S | B | blocked | — | TG-007 |
| TG-108 动效审计与 reduced-motion | M | C | blocked | — | M1 其余项 |

## M2 社交骨架（3 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-201 超级群与权限体系 | XL | A | blocked | — | M1 |
| TG-202 频道广播语义 | L | B | blocked | — | TG-201 |
| TG-203 频道评论区 | M | B | blocked | — | TG-202 |
| TG-204 话题（论坛模式） | L | C | blocked | — | TG-201 |
| TG-205 邀请链接体系 | M | A | blocked | — | TG-201 |
| TG-206 公开 username 与聊天发现 | M | C | blocked | — | TG-201 |
| TG-207 慢速模式与成员限制 UI | S | A | blocked | — | TG-201 |
| TG-208 单聊路径统一 | M | B | blocked | — | TG-005 |

## M3 表达力（2 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-301 TGS 解码与 Lottie 渲染 ← 性能风险 | L | A | blocked | — | M1 |
| TG-302 贴纸数据模型与服务端 | L | B | blocked | — | M1 |
| TG-303 贴纸面板 | L | A | blocked | — | TG-301, TG-302 |
| TG-304 自定义 emoji | M | B | blocked | — | TG-302 |
| TG-305 GIF | M | A | blocked | — | TG-303 |
| TG-306 静态与视频贴纸 | S | B | blocked | — | TG-301, TG-302 |

## M4 消息能力（4 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-401 语音消息 | L | A | blocked | — | M1 |
| TG-402 圆形视频消息 | M | A | blocked | — | TG-401 |
| TG-403 相册 / 媒体组 | M | B | blocked | — | M1 |
| TG-404 定时发送与静默发送 | M | B | blocked | — | M1 |
| TG-405 自毁计时器 | M | C | blocked | — | M1 |
| TG-406 投票与测验 | L | C | blocked | — | M1 |
| TG-407 位置与实时位置 ← 需选型确认 | M | D | blocked | — | M1 |
| TG-408 链接预览 ← 需安全评审 | M | D | blocked | — | M1 |
| TG-409 引用片段与跨聊天回复 | M | B | blocked | — | TG-103 |
| TG-410 联系人名片与消息翻译 | S | D | blocked | — | M1 |
| TG-411 消息效果与动画 | S | C | blocked | — | TG-108 |

## M5 组织与设置（4 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-501 聊天文件夹 | L | A | blocked | — | TG-102 |
| TG-502 归档区 | S | A | blocked | — | TG-102 |
| TG-503 Saved Messages ← 需决策确认 | M | B | blocked | — | TG-208 |
| TG-504 全局搜索分栏 | M | B | blocked | — | M3, M4 |
| TG-505 隐私设置矩阵 | L | C | blocked | — | TG-107 |
| TG-506 两步验证云密码 | M | C | blocked | — | M0 |
| TG-507 主题与聊天背景 | L | D | blocked | — | TG-009 |
| TG-508 通知例外与自定义声音 | M | D | blocked | — | M1 |
| TG-509 数据与存储 | M | A | blocked | — | M4 |
| TG-510 多语言 | M | B | blocked | — | M1 |
| TG-511 多头像与二维码名片 | S | C | blocked | — | M1 |

## M6 收口（串行）

| 任务 | 规模 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- |
| TG-601 PWA 强化 | M | blocked | — | M5 |
| TG-602 删除旧客户端与 alias | M | blocked | — | M5 |
| TG-603 PySide6 桌面端处置 ← 需用户决策 | S | blocked | — | TG-602 |
| TG-604 性能压测 | L | blocked | — | M5 |
| TG-605 `packages/core` 抽离验证 | M | blocked | — | M5 |
| TG-606 无障碍与键盘操作审计 | M | blocked | — | M5 |

---

## 状态取值

`not-started` · `in-progress` · `blocked` · `review` · `merged` · `abandoned`

`blocked` 用于「依赖未满足」和「被外部问题卡住」两种情况，后者必须在对应 devlog 的 Blockers 一节写明卡在什么上。

## 协议失效记录

发现 `docs/tg/agent-protocol.md` §8 列出的任一失效信号，记在这里。空着是好事，但只有真的看过才能空着。

- **2026-09-30** 盘点发现 `a16f422` 的 `scripts/check_file_sizes.py` 有 5 个错误，即基线本身是红的。旧路线图的 `FND-001 修复当前 CI 阻断` 是同一类问题，说明这道门禁会反复变红而没人盯。TG-000 负责修复，并确认 CI 是否真的在 PR 上执行这个脚本。

## 待用户决策

| 决策 | 涉及任务 | 状态 |
| --- | --- | --- |
| 地图供应商选型 | TG-407 | 未提出 |
| `favorites` 与 Saved Messages 的关系 | TG-503 | 未提出 |
| 开启 2FA 是否终止其他设备会话 | TG-506 | 未提出 |
| PySide6 桌面端去留 | TG-603 | 未提出 |
