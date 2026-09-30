# 任务看板

**全局状态的唯一真相源。只在 `main` 分支上更新，由集成负责人在合并时维护。不要在 worktree 里改这个文件。**

任务详情见 `docs/tg/roadmap.md`。每个进行中任务的细节见 `docs/devlog/<TASK-ID>.md`。

- 盘点提交：`a16f422`
- **绿色基线提交：`82b256a`** —— TG-012 合并后：cargo 72 个测试二进制 322 过 0 挂（真实 exit 0，PG+Redis 执行）、bun 全仓 259 过 0 挂、验收探针 8/8 由集成负责人对现场服务器独立复验（注册→建群→WS 发送→REST 持久→重连重放→第二账号实时收到→草稿帧回自己连接）
- 归档 tag：`archive/master-2026-09-30`（见 D-008）
- 盘点日期：2026-09-30
- 当前里程碑：**M1**（M0 合并完成；用户 2026-09-30 指示继续全部剩余里程碑，运行规则见 D-009）

### 已知抖动测试

- `tests/auth_security_test.rs::redis_adapter_shares_limits_between_instances_when_configured`：2 秒限流窗口，机器高负载时间歇失败。需要一个不依赖墙钟的版本（候选跟进项）。

### 服务依赖测试的可见性规则（TG-013 已修复静默跳过）

冻结契约（`docs/devlog/TG-013.md`）：**环境变量缺失 = 显式跳过并在原始 stderr 打 marker；设了但连不上 = 大声 panic；设了且可达 = 真实执行。** 不再有任何探测-回退 URL。统计一次运行漏掉的覆盖：

```sh
export TEST_POSTGRES_ADMIN_URL="postgresql://chatroom:chatroom@127.0.0.1:52735/postgres"
export TEST_REDIS_URL="redis://127.0.0.1:6379/"
cargo test --all-targets --all-features 2>&1 | grep -c 'SKIPPED: PostgreSQL not verified'
```

注意事项与跟进项：

- 主仓库 `.env` 通过 dotenvy 父目录搜索给所有 worktree 供 `CHAT_ROOM_REDIS_URL`，所以本机 Redis 测试总是自动配置；要 worktree 级隔离，`.env` 是那个开关。
- CI 已设 `TEST_REDIS_URL`（`68dcd41`）：Redis service 容器坏掉现在会让 CI 变红而不是静默丢覆盖。
- **跟进候选**：`tests/ai_threads_test.rs::ai_run_continues_without_a_browser_stream` 仍会在 Redis 不可达时静默降级到无 Redis 路径（经 `src/state_build.rs` 的生产回退），不消费门控变量——改它要动生产回退语义，应单独立卡。
- **跟进候选**：仓库没有任何 Qdrant 测试（不是静默跳过，是零覆盖），归将来向量检索测试的任务。

## 当前在飞的任务

波次 1（基线 `2903b89`，2026-09-30 开工）：TG-101、TG-102、TG-103、TG-107、TG-208。缝合约定：TG-101 的 `renderMessage(message, MessageRenderContext)` ← TG-103 的 `MessageBubble`；TG-102 会话行的 `isOnline`/`typingText` 与聊天头部 ← TG-107 的 hooks，均由集成负责人在合并后接线。

提前开工（后端为主、与波次 1 零路径交集，基线 `8ba40e5`）：TG-302 贴纸服务端、TG-506 两步验证。

**M0 完成标志的状态**：探针级端到端已由集成负责人独立复验通过（见绿色基线行），但**真实浏览器的两窗口视觉走查还没有任何人做过**——这是刻意如实的区分，M1 开工前请用户完成：`cargo run --bin server` 后浏览器走一遍 注册 → 建群 → 发消息 → 刷新保留 → 第二浏览器实时收到。CI 从未运行（所有提交在本地未推送）。

**M1 前的跟进项**（不阻塞，按优先级）：

- 用户浏览器走查（上面那条）；TG-012 devlog 的 Residual risk 列了已知外观限制（Firefox 输入框固定高度等）。
- `packages/core/tsconfig.json` 的 `types:["bun"]` 把 WHATWG 环境类型授给了 `src/`——应拆分 src/test 两个 tsconfig,恢复类型层第二道网。
- `chatSocket.createSocket` 同步抛出时静默卡 `connecting`;补拉过滤用 RFC3339 字典序比较(同源低风险);`ai_threads` 测试的 Redis 静默降级(见服务可见性一节)。
- 合并会新增 bun 依赖的分支后,主 checkout 必须 `bun install` 再跑 cargo——`build.rs` 只探测 vite 是否存在,依赖缺失时 `tsc` 挂而报「React web build failed」(本次集成实测踩到,CI 因总是 frozen install 不受影响)。

### 已集成

| 任务 | 提交 | 集成负责人的独立验证 |
| --- | --- | --- |
| TG-002 monorepo 骨架 | `296612a` | rebase 到 main 后：`bun install` 无变更，三包 typecheck 全 0，测试 22/0；**自己种入一个含 `react` + `document` + `localStorage` + `navigator` + `window` 的文件，边界检查确实失败并列出全部 5 处违规带行号与证据；删除后恢复 19/0**。没有采信 agent 的自我报告。 |
| TG-001 构建目录 | `4499a7e` | 见下方实测记录 |
| TG-000 绿色基线 | `fb818ee` | rebase 到 main 后重跑：`check_file_sizes.py` 通过（33 个基线告警、无增长）、迁移 parity 49 对、`cargo fmt` 干净、`clippy --all-targets --all-features` 零告警、`cargo test --all-targets --all-features` **exit 0**。另外确认 `src/lib.rs` / `src/models.rs` / `src/routes.rs` / `Cargo.toml` 与 main 零差异，即公开接口未变。**合并后 pre-push 钩子从拒绝转为通过** —— 这是「基线变绿」最直接的闭环证据。 |
| CI 加固 + D-008 | `8b9d178` | 钩子在红树上拒绝、在绿树上通过，两个方向都实测过 |
| TG-003 `build.rs` 切 React 产物 | `3f2822e` | rebase 到 main 后**亲自起服务器 curl 验证**，不采信报告：`GET /` 返回含 `<div id="root">` 的 React `index.html`；**`/assets/app.js`（Vue 入口）→ 404**；React 入口 200、220080 字节与 Vite 自报一致；服务的 bundle 里 `react-dom` 1 次、`createRoot` 2 次、`__REACT_DEVTOOLS_GLOBAL_HOOK__` 8 次，而 `createApp` 0 次、`primevue` 0 次，且含 `App.tsx` 的字面文本。另验 `src/web.rs` 硬嵌的 9 个遗留路径全部 200，**合成的 `sw.js` 确实是自注销版且对 `app.js` 的引用为 0 次** —— 旧 Service Worker 会把回访浏览器钉在 404 上，这是卡上没写、agent 自己发现的。 |
| TG-010 `packages/ui` 基础组件 | `38095c3` + `0d46042` | 集成补丁由负责人实测后落地：avatar 七色 token 补进两主题且集合仍完全一致（77=77）；`--tg-duration-loop` 放在 reduced-motion 折叠块之外的理由成立（循环动效收成 1ms 会频闪）；`.prettierignore` 按 14 个文件逐一测量后保留（出处标注是承重结构）；`bunfig.toml` 把裸 `bun test` 圈进 `packages/`，根 124/0、`web/` 208/0。**该合并当时未更新看板，此行为事后补记（见协议失效记录）。** |
| TG-004+005+006 Chat 重命名 | `9d09c2d` + `d12aae2` | rebase 到 main 零文件交集；基线四行补丁由负责人折进功能提交，保持该提交自绿。独立验证未采信自我报告：fmt/clippy 干净；全量套件在 `TEST_POSTGRES_ADMIN_URL` 下全绿 —— 唯一失败定位为 main 自 TG-003 起的既有红灯（`web_client` 测试两侧逐字节相同），非本分支回归，已修于 `f044c5d`；bun 门禁 124/0 且 lockfile 无变化；四路对抗验证全绿：**SQLite** 64 个迁移用 CLI 3.51.0（默认 `legacy_alter_table=ON`，风险真实）全量重放 —— 触发器体确认重写、FK 探针拒绝孤儿行、`rooms`/`room_%` 对象为零、`foreign_key_check` 零行；**PostgreSQL** 54 个迁移重放 —— `pg_proc.prosrc` 扫描零旧表名、`record_room_join_notification` 确认重建、双触发路径带回滚冒烟通过、95 个外键全解析；**门禁突变**：501 行种子确实被咬、无陈旧基线键；迁移 parity 52 对。遗留（外观）：PostgreSQL 约束名保留旧名，devlog 已记。 |
| TG-008 云端草稿 | `8d62e9d` | 负责人独立验证:合并树 cargo 72 个二进制 322/0、parity 53 对;对抗审计确认——`draft_updated` 唯一发射点走 `AppState::broadcast`(隐私过滤生效,live 测试驱动真实 PUT 断言他人账号收不到)、读写路径都重查活跃成员、幂等 PUT 不广播、迁移与 devlog schema 一致且无重命名隐患、日志零草稿正文。卡片修正:列名 `room_id`(TG-004 冻结规则)。它实测发现的共享构建目录陈旧 `sqlx::migrate!` 嵌入隐患已由集成补丁修入 `build.rs`。 |
| TG-012 登录与最小可用壳 | `7c3fdc6`..`82b256a` | 负责人独立验证:allowed-paths 审计零越界(仅 `packages/web/**`+devlog+lockfile);合并树 cargo 322/0(嵌入的正是本壳,`web_client` 测试对其断言)、bun 259/0;**验收探针由负责人对现场服务器亲自重跑,8/8**。诚实边界:无真实浏览器走查,已列为 M0 收口的用户步骤。发现并修正自己的验证工具缺陷:awk 管道吞掉 cargo 退出码,首轮「全绿」实为构建失败(缺 `bun install`),重跑改用真实 exit 判定。 |
| TG-011 `packages/core` 骨架 | `9c78047`..`6fc80a6` | 负责人独立验证:bun 全仓 230/0、typecheck 零错;对抗审计逐字段核对 18 个帧标签/可选性方向/十个 typing action/七档 user_status/六个骨架帧 vs Rust serde 与字节级快照——两处失真已修(`6be5fa4`):TS `StoredMessage` 漏 `client_message_id`(会让重连补拉永远无法对账乐观发送)、`favorite_id` 错标可选;并揪出边界测试看不见的 WHATWG 全局(`URLSearchParams`/`globalThis.fetch`,RN 上会炸)——换纯 ES 实现、禁用表扩容加元钉。九个 store 均为 zustand vanilla、WS 客户端退避曲线与冻结 Vue 一致。zustand@5.0.8 为唯一新依赖(预批)。 |
| TG-013 服务测试跳过可见性 | `e786895` | 负责人在合并树上实测：env 缺失 → 全量套件 exit 0 且 **16 个 `SKIPPED: PostgreSQL not verified` marker 逐一可见**（raw-fd 写 stderr 绕过 libtest capture——原来的静默机制正是 capture 吞掉 eprintln）；env 设定 → 0 marker、PG 测试真实执行；env 指向死端口 → panic 而非跳过。对抗审计确认 `tests/` 内零探测-回退残留；顺带揪出 `src/cache.rs` 里它没扫到的同类缺陷（负责人已修 `941b0b7`）与 CI 缺 Redis env（已修 `68dcd41`）。 |
| TG-007 WebSocket 帧扩展 | `20913a5` | 负责人独立验证：合并树 68 个二进制 309/0（PG+Redis 全设）；**草稿隐私突变测试**——故意让 `frame_visible_to` 泄漏草稿，`ws_frame_routing_test` 立刻红，还原即绿，隐私过滤真实被钉住；对抗审计逐字段核对旧帧 vs `d12aae2` 线上真值，全部一致，typing 向后兼容机制（`serde(default)` + 未知 action 降级 + 空 content 停止语义）逐行确认；两个有意的旧帧扩展（typing.action、auth_ok.statuses）已记录且冻结客户端实测容忍（CLI 在 tmux 里真跑、Vue 用 FakeWebSocket 实测）。审计指出快照测试只比 Value 不锁字节序 → 负责人已加字节级 pin（`bd22ebf`）。附带落地其 Vue 集成补丁（`bc0e377`，实测消除空系统气泡）。 |
| TG-302 贴纸服务端 | `e3d4089` → 合并提交 | 负责人在 main 上用私有构建目录独立复跑（D-011，不采信共享目录结果）：fmt/clippy 干净，78 个二进制 340 过 2 挂 → 逐个定位：`ws_frame_routing_test::draft_updated…` 是**测试自身竞态**（bob 自己的「joined the room」系统帧抢在 marker 前，`collect_until("system")` 取错帧），负责人修测试为跳过非 marker 系统帧，隐私断言仍覆盖 marker 之前全部帧，4/4 通过；`auth_security_test::redis_adapter_shares_limits…` 是 2 秒窗口的限流测试，负载 ~25 时间歇失败（隔离重跑 2/3），记为已知抖动。迁移 parity 55 对。agent 在卡外修了一处真实缺陷：孤儿附件清理会在引用消息全部撤回后删掉贴纸包文件，已加回归测试。遗留：转发的贴纸退化为普通附件；custom emoji API 归 TG-304。 |
| TG-101 虚拟消息列表 | `1823285` → `a2da0a2` | 负责人合并树复跑：lint/typecheck 0，测试 core 213/0、ui 103/0、web 262/0；两处追加式冲突（`styles/index.css`、core `domain/index.ts`）手工保留双方并确认无重复导出。agent 实测：prepend 锚点位移 0 px（0–200 ms 延迟）、跳 5 万条后返回 0 px、10 万行列表堆 8.5 MB。遗留：`messageStore` 只更新自己的 timeline，REST 加载的消息收不到编辑/撤回/回应 → 交 TG-100 加 `prependHistory`；零位移部分依赖 react-virtuoso 内部行为，升级时须重跑 `runBrowserBench.mjs`。 |
| TG-105 媒体查看器 | `992e442` → `db1afad` | 负责人合并树复跑：web 238/0（core 179、ui 103），lint/typecheck 0；`styles/index.css` 追加行冲突手工保留双方。未挂载：待 TG-101 合并后在应用根挂 `<MediaViewer>` 并绑定气泡 `onOpenMedia`。遗留：文件列表 API 不能向新方向翻页（深跳后看不到更新的媒体）；列表来源媒体无说明文字。 |
| TG-104 输入框 | `f4da8f9` → 合并提交 | 负责人合并树复跑：lint/typecheck 0，测试 core 179/0、ui 103/0、web 193/0。Composer 尚未挂载：需要 `chatSession.sendFrame`，该文件属在飞的 TG-101，负责人在 TG-101 合并后按 devlog 补丁清单接线（含删除旧 `chat/Composer.tsx`、移除会话自带 typing 预览避免重复帧）。遗留：上传不带内容哈希（无去重/直传 OSS）；多文件逐条发送（相册归 TG-403）；无上传占位行；转发失败无 toast。 |
| TG-102 三栏布局与会话侧栏 | `f7c4c4a` → 合并提交 | 负责人合并树复跑：lint/typecheck 0，测试 core 147/0、ui 103/0、web 178/0；`styles/index.css` 双方各加一行自动合并。负责人接线：会话行的正在输入文案改由 TG-107 `useTypingSummary` 提供（`chatRowPresence.ts`）。遗留：已读双勾与未打开会话的草稿标记需要 `/api/conversations` 增加 peer-read 标志与草稿字段（转给 TG-208 之后的集成补丁）；`MobileBackButton` 待放进 TG-101 的聊天头部。**风险**：共享构建目录下 `build.rs` 嵌入的 web 产物来自最后一个构建它的 worktree——任何用 `cargo run` 做 UI 验证的 agent 可能看到别人的前端；UI 截图应以 worktree 自己的 vite 产物为准，集成验证以 main 为准。 |
| TG-103 消息气泡系统 | `588054c` → `6108b90` | 负责人合并树复跑：三包 lint/typecheck 0，测试 core 147/0、ui 103/0、web 126/0（新增 86），文件大小通过。气泡经 `registerMessageContent` 注册表开放给 M3/M4 的新消息类型。已知：附件无宽高元数据，图片加载后行高变化（TG-101 的高度修正要兜住）；气泡与尾巴假定 LTR；相册比对待 TG-403。 |
| TG-107 输入状态与在线状态 UI | `d1d5d2d` → `4181c4f` | 负责人在合并树上复跑：三包 lint/typecheck 全 0，测试 core 147/0、ui 103/0、web 40/0；文件大小（排除本机未跟踪的 `web-v2/` 构建残留）与迁移 parity 53 对通过。组件尚未挂载，待 TG-101/102 合并后由负责人接线。已知限制：服务端尚无 last-seen 持久化，未见过下线的用户显示「离线」，归 TG-505。 |
| TG-009 Design tokens | `785953a` | rebase 到 main 后独立验证：**`day.css` 与 `night.css` 各声明 70 个 token 且集合完全一致**，6 个强调色文件集合亦完全一致 —— 主题切换不可能留下未定义变量（这是 agent 没提、但最容易出问题的不变量）。`preview.html` 对 `--tg-raw-` 原语的引用数为 **0**，且十六进制、`rgb()`/`hsl()`、命名颜色字面量各为 **0** —— 它确实只靠语义 token 上色，所以是证明而非效果图。原语仅被 token 层自身引用。每个值带 `[web]`/`[desktop]`/`[ios]`/`[derived]`/`[ours]` 出处标注，真实值与猜测值可区分。文件大小最大 253 行。 |

## 顺序约束

M0 全部 merged。用户 2026-09-30 指示开放 M1 并完成全部剩余里程碑（D-009）。CI 修复 `759721c`+`0e538c8` 后 `main` 首次 CI 全绿。

## M0 地基

| 任务 | 规模 | 状态 | Owner | Worktree | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-000 修复红色基线 | S | **merged** `fb818ee` | agent:TG-000 | — | — |
| TG-001 回收构建目录并验证共享配置 | S | **merged** `4499a7e` | 集成负责人 | — | — |
| TG-002 monorepo 骨架 | M | **merged** `296612a` | agent:TG-002 | — | — |
| TG-003 `build.rs` 切换嵌入目标 | S | **merged** `3f2822e` | agent:TG-003 | — | — |
| TG-004 Chat 数据模型迁移 | L | **merged** `9d09c2d` | agent:TG-004 | — | TG-000 ✓ |
| TG-005 Rust 模块与类型重命名 | L | **merged** `9d09c2d`（同一提交） | agent:TG-004 | — | TG-004 ✓ |
| TG-006 API 路径重命名与 alias | M | **merged** `9d09c2d`（同一提交） | agent:TG-004 | — | TG-005 ✓ |
| TG-007 WebSocket 帧扩展 | M | **merged** `20913a5` | agent:TG-007 | — | TG-005 ✓ |
| TG-008 云端草稿 | M | **merged** `8d62e9d` | agent:TG-008 | — | TG-006 ✓, TG-007 ✓ |
| TG-009 Design tokens 提取 | M | **merged** `785953a` | agent:TG-009 | — | — |
| TG-013 修正 PostgreSQL 测试静默跳过 | S | **merged** `e786895` | agent:TG-013 | — | — |
| TG-010 `packages/ui` 基础组件 | L | **merged** `38095c3`+`0d46042` | agent:TG-010 | — | TG-002 ✓, TG-009 ✓ |
| TG-011 `packages/core` 骨架与逻辑迁移 | L | **merged** `6fc80a6` | agent:TG-011 | — | TG-002 ✓, TG-006 ✓, TG-007 ✓ |
| TG-012 登录与最小可用壳 | M | **merged** `82b256a` | agent:TG-012 | — | TG-003 ✓, TG-010 ✓, TG-011 ✓ |

**M0 完成标志** 新 React 客户端能完成 注册 → 建群 → 发消息 → 刷新保留 → 第二浏览器实时收到；CI 全绿；共享构建目录体积记录在案。

## M1 会话骨架（3 路并行，M0 完成后开放）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-100 M1 集成接线（负责人新增） | M | — | in-progress | agent:TG-100 | TG-101..105,107 |
| TG-101 虚拟消息列表 ← 最高风险 | XL | A | **merged** `a2da0a2` | — | TG-012 |
| TG-102 三栏布局与会话侧栏 | L | B | **merged** | — | TG-012 |
| TG-103 消息气泡系统 | L | C | **merged** `6108b90` | — | TG-012 |
| TG-104 输入框 | L | B | **merged** | — | TG-102, TG-008 |
| TG-105 媒体查看器 | M | C | **merged** `db1afad` | — | TG-103 |
| TG-106 右侧信息面板 | M | A | in-progress | — | TG-101 |
| TG-107 粒度输入状态与在线状态 UI | S | B | **merged** `4181c4f` | — | TG-007 |
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
| TG-208 单聊路径统一 | M | B | in-progress | — | TG-005 |

## M3 表达力（2 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-301 TGS 解码与 Lottie 渲染 ← 性能风险 | L | A | in-progress | — | M1 |
| TG-302 贴纸数据模型与服务端 | L | B | **merged** | — | M1 |
| TG-303 贴纸面板 | L | A | blocked | — | TG-301, TG-302 |
| TG-304 自定义 emoji | M | B | in-progress | — | TG-302 |
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
| TG-406 投票与测验 | L | C | in-progress | — | M1 |
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
| TG-505 隐私设置矩阵 | L | C | in-progress | — | TG-107 |
| TG-506 两步验证云密码 | M | C | in-progress | — | M0 |
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

## TG-001 实测记录（集成负责人执行）

| 项 | 结果 |
| --- | --- |
| 旧 `target/` 删除 | 卷可用空间 450Gi → **642Gi**，回收 192GB |
| 共享目录冷编译 | `cargo clippy --all-targets` 通过，1m44s，产生 1.4–1.5GB |
| 共享配置生效 | 确认新路径被创建并填充 |
| CI 覆盖 | `.github/workflows/ci-cd.yml` env 块设 `CARGO_TARGET_DIR` |
| Docker 覆盖 | `Dockerfile` builder 阶段设 `CARGO_TARGET_DIR=/app/target`，保住 cache mount |
| 清理工具 | `scripts/tg-sweep-build-cache.sh`，默认 dry-run，检测 `rustc` 在跑时拒绝清理 |
| `cargo-sweep` | **未安装**。脚本会提示安装命令并降级为只报告。装它要编译，需等无人构建时再做 |
| `docker build` 验证 | **取消，不在本地做**（用户指示 2026-09-30：镜像工作在 GitHub Action 完成）。CI 的 `image` job 无 `if:` 条件，`agent/**` 分支推送时也会构建，覆盖已足够。`Dockerfile` 的 `ENV CARGO_TARGET_DIR=/app/target` 靠阅读确认。 |

结论：194GB 里 99% 以上是沉积。一次 `clippy --all-targets` 的真实足迹是 1.5GB。

## 集成负责人自己引入的缺陷

诚实记录，和 agent 的缺陷同等对待。

- **2026-09-30** `f044c5d`（重写 web_client 测试）只跑了那个测试本身就提交，没过 `cargo fmt --check` —— main 的 fmt 门禁红了一轮，被 TG-007 与 TG-013 两个 agent 各自独立撞见（它们的分支在红基线上无法自绿）。已修（`b88c4af`）。教训：单文件测试修复也要走完整门禁清单；另外本机 rustfmt 无 toolchain pin，fmt 结论跨机器不可复现，TG-013 的 agent 建议加 `rust-toolchain.toml`，值得单独考虑。
- **2026-09-30** 为了让任务分支获得 CI 覆盖，我把 `on.push.branches` 从 `[main]` 放宽到 `[main, "agent/**"]`。但 `publish` job 当时只排除 `pull_request`，而 metadata 用 `type=ref,event=branch` 打标签 —— **每个 worktree 分支推送都会往 ghcr.io 发布一个以分支命名的镜像**。已把 `publish` 的 `if:` 限定为 `main` 与 `v*` tag。`image` job 保持无条件，那才是想要的覆盖。教训：放宽触发条件前要把该 workflow 里所有下游 job 的条件过一遍。

## 协议失效记录

发现 `docs/tg/agent-protocol.md` §8 列出的任一失效信号，记在这里。空着是好事，但只有真的看过才能空着。

- **2026-09-30** 盘点发现 `a16f422` 的 `scripts/check_file_sizes.py` 有 5 个错误，即基线本身是红的。旧路线图的 `FND-001 修复当前 CI 阻断` 是同一类问题，说明这道门禁会反复变红而没人盯。TG-000 负责修复，并确认 CI 是否真的在 PR 上执行这个脚本。
- **2026-09-30** TG-002 的 agent 报告 `scripts/tg-worktree.sh check` 从 worktree 内部无法运行：`REPO_ROOT` 用了 `git rev-parse --show-toplevel`，在 worktree 里返回 worktree 自己的根，于是去找 `<worktree>/.claude/worktrees/<task>`。**这是脚本的真实缺陷，协议 §2.4 对每个 agent 都会失效。** 已改用 `--git-common-dir` 解析主仓库，并加了「worktree 里的旧副本自动委托给主仓库当前版本」的自愈。自愈只对此修复之后创建的 worktree 生效，基线为 `5ee18f4` 或更早的 worktree 需按 §2.4 用绝对路径调用。该 agent 手工复现了全部检查项，没有静默跳过 —— 这是正确的处理方式。
- **2026-09-30** TG-002 的 agent 提交了 `bun.lock`，虽然它不在该任务的 allowed paths 里。理由成立（创建 bun workspace 必然产生它，CI 的 `--frozen-lockfile` 没有它无法工作），且已在 devlog 中显式标注而非静默提交。集成负责人追认。**这说明 allowed paths 应该预见到锁文件** —— 后续涉及包管理的任务卡要把锁文件写进 allowed paths。
- **2026-09-30** TG-010 已合并（`38095c3` + `0d46042`）但当时没有更新看板：在飞表仍列 `tg-010`、M0 表仍标 in-progress、已集成表无该行、worktree 已删而看板不知道。本次 TG-004 集成时发现并补记。「合并时更新看板」是集成负责人自己的职责 —— 看板是唯一真相源，它失真比任何单个 devlog 失真都贵。
- **2026-09-30** 对抗审计发现 **TG-004 的机械重命名改掉了十个 WebSocket 线上数据字符串**（System 帧内容、断连原因、auth_fail 原因：`room renamed to` / `room password changed` / `room deleted` / `room locked` / `{} joined the room` 等），而冻结 Vue 客户端在 `roomSystemEvents.ts` / `chatProtocol.ts` 里**按字符串匹配这些内容做功能处理**——改名刷新、密码清理、删除清理静默失效。TG-004 的冻结清单覆盖了 API 与数据值却漏了 WS 帧内容字符串，且没有任何测试钉住它们，所以全量套件抓不到。已恢复旧拼写并加钉死测试 `tests/ws_frozen_wire_strings_test.rs`（`7cb1a80`），每条断言注明它保护的前端匹配行；`CONTEXT.md` 的 Room 条目同步补全。教训：**冻结契约必须包含所有客户端字符串匹配的线上值，且每个值要有测试**——没被测试钉住的契约在机械重命名面前等于不存在。
- **2026-09-30** TG-004 devlog 声称「18 个 PostgreSQL 测试真实执行」，实为 **16 PG + 2 Redis**（TG-007 的报告照抄了同一数字）。对抗普查逐一点名后更正。数字要可复算，不要转抄。
- **2026-09-30** 集成 TG-004 跑全量套件时发现 `web_client::web_client_is_only_served_when_enabled` 在 main 上红：它仍断言旧 Vue 壳（`<div id="app">`、`/assets/app.js`、PrimeVue 变量），而 TG-003 在 `3f2822e` 已把嵌入产物切成 React。TG-003 集成时只做了手工 curl 验证、没有重跑全量 cargo 套件；且所有 TG 提交仍未推送，CI 一次都没跑过。已修于 `f044c5d`，把当时的手工验证固化成断言（含「`/assets/app.js` 必须保持 404」与「`sw.js` 必须自注销且不引用任何资产」）。教训：**集成清单必须包含全量 cargo 套件** —— 手工验证只能补充、不能替代；未推送则 CI 等于不存在。

## 待用户决策

| 决策 | 涉及任务 | 状态 |
| --- | --- | --- |
| 地图供应商选型 | TG-407 | **已决** D-010：Leaflet + 可配置瓦片（默认 OSM） |
| `favorites` 与 Saved Messages 的关系 | TG-503 | **已决** D-010：投影，favorites 为真相 |
| 开启 2FA 是否终止其他设备会话 | TG-506 | **已决** D-010：终止其他会话 |
| PySide6 桌面端去留 | TG-603 | **已决** D-010：保留并接新 API |
