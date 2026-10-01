# 任务看板

**全局状态的唯一真相源。只在 `main` 分支上更新，由集成负责人在合并时维护。不要在 worktree 里改这个文件。**

任务详情见 `docs/tg/roadmap.md`。每个进行中任务的细节见 `docs/devlog/<TASK-ID>.md`。

- 盘点提交：`a16f422`
- **绿色基线提交：`82b256a`** —— TG-012 合并后：cargo 72 个测试二进制 322 过 0 挂（真实 exit 0，PG+Redis 执行）、bun 全仓 259 过 0 挂、验收探针 8/8 由集成负责人对现场服务器独立复验（注册→建群→WS 发送→REST 持久→重连重放→第二账号实时收到→草稿帧回自己连接）
- 归档 tag：`archive/master-2026-09-30`（见 D-008）
- 盘点日期：2026-09-30
- 当前里程碑：**M1**（M0 合并完成；用户 2026-09-30 指示继续全部剩余里程碑，运行规则见 D-009）

### 已知抖动测试

- 无。TG-111（`2fb1540`）已从源头修复此前三个（内存 SQLite 被换空库导致的 presence/隐私矩阵间歇失败、Redis 限流 2 秒窗口）。

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

**在飞（M8，基线 `634edad`，2026-10-01 开工）**：TG-801 联系人修复、TG-802 会话摘要媒体类型、TG-803 共享内容分类、TG-804 对标走查。

历史：波次 1（基线 `2903b89`，2026-09-30 开工）：TG-101、TG-102、TG-103、TG-107、TG-208。缝合约定：TG-101 的 `renderMessage(message, MessageRenderContext)` ← TG-103 的 `MessageBubble`；TG-102 会话行的 `isOnline`/`typingText` 与聊天头部 ← TG-107 的 hooks，均由集成负责人在合并后接线。

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
| TG-508 通知例外与自定义声音 | 负责人实现 → 合并提交 | 既有 `chat_members.notification_level`/`muted_until` 仍是静音开关（1h/8h/2d = muted_until 到期自动恢复；永久 = level none），新增按聊天类型的默认值与按聊天例外（NULL 继承）。**一条纯函数规则** `decide`（带测试矩阵）：定时静音 > 例外 enabled > 旧 level > 类型默认；Web Push 投递经它取得是否推送、预览开关与声音（声音 none → 静默通知）。设置「通知与声音」页、信息面板静音时长与声音。分支 142 个二进制 588/0。 |
| TG-501 聊天文件夹 | 负责人实现 → 合并提交 | 服务器只存规则（`chat_folders` + `chat_folder_chats`，≤10 个、标题 ≤12 字、读时剔除已退出的聊天），归属由 core 纯函数 `inFolder` 在客户端按会话列表求值，所以未读徽标与列表永远一致。显式排除 > 显式包含 > 类型；静音/已读/归档排除标志对显式包含也生效。列表上方标签或左侧栏（设置 › 聊天文件夹，含编辑器与排序）。分支 chat_folders_test 2/2，web 705/0。 |
| TG-108 动效审计与 reduced-motion | 负责人实现 → 合并提交 | 审计结论：组件时长早已全部走 token；缺口是裸 `linear`、模态/底部弹层/弹出菜单/移动端推入用的贝塞尔，以及三处没有动效（桌面侧栏折叠、新消息进入、长按反馈）。每个交互一个 `--tg-motion-*` token，全部映射到 TG-009 弹簧；新消息仅在挂载时 <3 s 才上浮；长按计时期间气泡缩至 0.96（减弱动效下改为变暗）。新增 `motionDiscipline.test.ts` 闸门。web 710/0。 |
| TG-411 消息效果与动画 | 负责人实现 → 合并提交 | 自己新发的消息从输入框方向弹簧上浮；自己的回应落定（未选→已选，且芯片在屏）才迸发（弹跳+光环），历史不迸发；1–3 个纯 emoji 的文本消息以无气泡大号显示（新内容类型 `bigEmoji`），到达时弹出、点击重播“戳一下”。只动 transform/opacity，无布局抖动；减弱动效下改为淡入或去除。web 713/0。 |
| TG-203 频道评论区 | 负责人实现 → 合并提交 | 频道↔讨论组双向 `linked_chat_id`（两侧管理员、仅群/超级群、一群只服务一个频道）。帖子复制进讨论组由**数据库触发器**完成（所有发送路径都自己插入 messages），实时轮询照常投递；撤回帖子同步撤回副本、评论留在群里，频道侧线程 404；硬删除先撤回副本；编辑同步。解绑后旧线程仍可读。评论 = 讨论组回复链（递归 CTE 计数）；读只需频道，写需入群（一键加入）。迁移用 `20270201000010`（预留号会乱序）。分支 145 个二进制 594/0。 |
| TG-504 全局搜索分栏 | 负责人实现 → 合并提交 | 搜索框下七个分页：聊天（客户端过滤+公开结果）/消息/媒体/链接/文件/音乐/语音；每个消息分页是 `/api/messages/search` 的一个 `content_type` 过滤（新增 media/document/link/music/voice），各自游标、各自加载。授权仍是读时按活跃成员关系联表（退群即消失，有测试）。`q` 也匹配附件文件名。发送者/日期筛选；最近搜索（每浏览器、去重、上限 10）与联想。无需迁移。 |
| TG-407 位置与实时位置 | 负责人实现 → 合并提交 | D-010：Leaflet 1.9.4（懒加载）+ 可配置瓦片 `[map] tile_url/attribution`（默认 OSM，可换自托管），经 `/api/config` 下发；不调用任何地理编码服务。位置 = 普通消息 + `message_locations` 一行；实时位置**只存最新一点、无轨迹**，到期或停止后服务端拒绝更新（409）。隐私提示写明谁可见、精度与无轨迹；「模糊位置」客户端先取整到约 1 km。浏览器每 15 s 最多上报一次。合并视图列出聊天内所有实时位置。 |
| TG-408 链接预览 | 负责人实现 + **安全评审通过** → 合并提交 | 服务端在消息入库并投递**之后**后台抓取首个 http(s) 链接的 OG/title（失败不影响发送，有慢站点测试）。SSRF：仅 http/https、无凭据、端口白名单；自解析 DNS 且**所有**应答必须为公网地址；把检查过的 IP 钉死给连接（防 DNS rebinding）；手动跟随重定向（≤3）逐跳复检；不使用环境代理；5 s 截止、256 KiB 上限、仅 HTML；全局并发 4；成功缓存 24h、失败 10min。测试覆盖 127.0.0.1、169.254.169.254、file://、rebinding、重定向到内网。发送者可在输入框取消或事后移除卡片。 |
| TG-507 主题与聊天背景 | 负责人实现 → 合并提交 | 主题/强调色是 `<html>` 上的属性驱动 token 层，组件零改动；补齐 Telegram 的红、灰两色共 8 色；新增「定时」夜间（可跨午夜，每分钟复核）。壁纸按账号存服务器：全局 + 按聊天覆盖（聊天优先），预设/纯色/渐变/上传图片（仅本人可取，按魔数识别格式，≤8 MiB），模糊与 0–80% 暗化。壁纸是聊天面板后独立的合成层（`contain: strict`，不在滚动容器内，禁止 `background-attachment: fixed`，有结构测试）。主题文件导出/导入。 |
| TG-510 多语言 | 负责人实现 → 合并提交 | 不引入依赖：core 内 ~100 行运行时（`{n}` 参数、`Intl.PluralRules` 复数、回退）。用 TypeScript AST codemod 把 web 1149 条 + core 55 条中文文案抽到按功能分文件的 zh-CN/en 目录（模块级表格改 getter，常量改函数）。设置 › 语言即时切换（`LocaleRoot` 按语言重挂载，不刷新页面），`<html lang>` 同步。CI 闸门解析全部源码，组件中出现汉字即失败；两包目录完整性与参数一致性有测试。 |
| TG-605 `packages/core` 抽离验证 | 负责人实现 → 合并提交 | `hosts/node/smoke.ts` 在 core 之外注入 Node 的存储（Map）、WebSocket（Node 24 内置 WHATWG）与时钟，跑通 注册 → 列会话 → 建群 → 连 socket → 发消息 → 收广播 → 读历史。集成测试以**无 DOM lib** 类型检查、Bun 打包、断言包内无 `document.`/`window.`，再用纯 `node` 运行（无 polyfill）。 |
| TG-603 PySide6 桌面端处置 | 用户决策 D-010 → 负责人实现 → 合并提交 | 保留 PySide6 并接新 API：所有 HTTP 调用从 `/api/rooms/*` 改到 `/api/chats/*`，建群发 `title`，读取 `title`（兼容旧 `name`）；新增“客户端不含 `/api/rooms`”的静态测试。desktop pytest 16/16，ruff 干净。为 TG-602 删除别名扫清障碍。 |
| TG-602 删除旧客户端与 alias | 负责人实现 → 合并提交 | 删除 `web/`（Vue 客户端）；`src/web.rs` 内嵌的 8 个文件迁到 `packages/web/public/`，build.rs/Dockerfile/CI/尺寸基线同步。删除 `/api/rooms/*` 别名、方言中间件与 OpenAPI 孪生条目、`ChatCompatView`（`name` 冗余字段；输入仍接受 `name`）。ratatui CLI、压测工具、桌面端全部走 `/api/chats`，并在“聊天目录”里过滤私聊（别名过去的行为）。`grep -rn "/api/rooms" src/ packages/` 无结果；能力矩阵与 README 架构图重写。 |
| TG-601 PWA 强化 | 负责人实现 → 合并提交 | React 客户端自己的 Service Worker（TS，WebWorker lib 类型检查，Vite 第二入口产出无哈希自包含 `/sw.js`）：离线可读应用壳、会话列表、打开过的消息页与看过的媒体（按 TG-509 上限/保留天数淘汰）；草稿本地保留、联网后同步；发送需要联网；退出登录清空缓存。更新提示（新 SW 等待，点「刷新」生效）、安装入口、推送订阅开关。**推送深链**改为 `/chat/:id?message=`（原 Vue 路由），点击聚焦并跳转到该消息。 |
| TG-604 性能压测 | 负责人实现 → 合并提交 | `docs/tg/perf-results.md`：10 万条消息（最新页 13 ms、千页之前 59 ms、跳到最早 94 ms）、500 会话列表（183 ms）、20 万成员（键集分页 ≤ 6 ms）、1000 人同时浏览（1 帧）、20 个动态贴纸（59.8 fps）、混合压测（HTTP 3.7k ops/s、WS、上传全部 0 错误）。压测发现并修复两个缺陷：SQLite 并发上传 `database is locked`（先读后写的事务改为先写）、写竞争下迟提交消息漏推（10 s 尾窗 + 每连接去重）。 |
| TG-606 无障碍与键盘操作审计 | 负责人实现 → 合并提交 | 纯函数键位表：Ctrl/⌘+K 或 / 搜索、Alt+↑/↓ 切会话、Ctrl/⌘+Shift+1…9 文件夹、Ctrl/⌘+, 设置、输入框 Ctrl/⌘+↑/↓ 键盘回复（避开浏览器保留键）。对比度测试从 token 文件算比值：正文类 ≥ 4.5（调整了日间危险文字与夜间链接两个**文字** token），强调色填充与带第二信号的元信息按裁决保留，偏离清单 `docs/tg/contrast-deviations.md` 由测试锁定。 |
| TG-509 数据与存储 | 负责人实现 → 合并提交 | Telegram 自动下载矩阵（Wi-Fi / 移动数据 / 漫游 × 图片 / 视频 / 文件 + 视频文件大小上限），纯函数在 core 带测试，网络类型取 Network Information API（Save-Data 视为漫游，未知视为 Wi-Fi）；图片/视频气泡在规则拒绝时显示「点击下载 · 大小」。「数据与存储」页：浏览器占用（`storage.estimate`）与 Cache Storage 分区、清除缓存（屏上媒体是 DOM 元素，不受影响）、最大缓存与保留天数（交 TG-601 的离线缓存执行）。按聊天清理暂不提供（今天无按聊天的缓存），记入 devlog。 |
| TG-511 多头像与二维码名片 | 负责人实现 → 合并提交 | `user_avatar_files` 保留为「当前头像」指针（冻结客户端不受影响），新表 `user_avatar_history` 保存全部上传并回填现有头像；替换/改用 emoji 不再删文件，只有从历史删除才删；删当前头像时提升次新的一张（或清空），顺序与指针始终一致；历史读取同样经 TG-505 头像隐私判定。二维码编码 `/add/<username>` 绝对链接，强调色 + 两主题都为白色的新 token `--tg-scan-surface` 卡片（深色主题也能扫），颜色运行时取自 token（无字面量）；「设置 › 我的账号」挂「头像」「我的二维码」。双适配器测试且既有头像/隐私测试保持通过；分支 141 个二进制 584/0。 |
| TG-410 联系人名片与消息翻译 | 负责人实现 → 合并提交 | 名片 = 普通消息（`media_kind: contact`）+ `message_contacts` 发送时快照，走 `message.send` 与 TG-204/207 发帖闸门；附件菜单「联系人」打开好友选择器；气泡带「发消息 / 添加好友」。翻译复用既有 AI 供应方（`AiAssistant::translate`），只返回给请求者、不回写原文；AI 关闭（D-009）时 `/api/translation` 为 `available:false`、前端不显示「翻译」、接口 503。整聊天翻译模式暂不做（AI 关闭时不可见），记入 devlog。分支 141 个二进制 584/0。 |
| TG-409 引用片段与跨聊天回复 | 负责人实现 → `0354b4b` | 引用必须是原文真实片段（服务端按 UTF-16 校验，≤1024），不匹配则丢弃引用保留回复。跨聊天回复须能读源聊天，否则降级为普通消息；目标聊天只看到发送时的**快照**（源发送者、源聊天标题、引用或前 200 字），`MESSAGE_SELECT` 的实时回复 join 限定同聊天——任何读路径都拿不到源消息其余内容。原文在引用后被编辑 → 「已修改」。PG 上 `INTEGER` 与 i64 绑定不匹配导致写入失败，测试抓到并改 `BIGINT`。回复列与映射移到 `reply_quotes::ReplyColumns`（`#[sqlx(flatten)]`），`store.rs` 由基线 457 降到 417。前端：「引用」读取选区、「在其他聊天中回复」复用转发选择器、气泡与回复栏显示引用/来源。分支 140 个二进制 582/0。 |
| TG-503 Saved Messages | 负责人实现 → `9987b58` | 按用户决策 D-010 做**投影**：`favorites` 仍是唯一真相，无自聊行、无迁移（预留号 `20270201000002` 未用并记入 devlog），后端零改动故既有收藏测试全部保持有效。`/saved` 以聊天形态显示（笔记输入、转发、删除、可访问时「查看原消息」），会话列表顶部固定「收藏夹」行，所有已送达消息的菜单加「保存到收藏夹」。 |
| TG-206 公开 username | 负责人实现 → 合并提交 | Telegram 句柄规则（5–32、a–z0–9_、字母开头、不以下划线结尾/不连续）+ 保留词；存小写使既有部分唯一索引实现大小写不敏感唯一，竞争写入的唯一冲突映射为 409；与用户登录名共享命名空间。预览对非成员**不返回内部 id**，加入走句柄并直接复用 `request_join`（封禁/锁定/审批/加入策略一处生效）；设进群密码的群不能公开。`/api/chats/discover?q=` 句柄前缀或标题子串。前端：`/public/:username` 预览页、管理面板「公开链接」（防抖可用性检查）、会话列表搜索下的「全局搜索」。双适配器测试；分支 139 个二进制 579/0。 |
| TG-405 自毁计时器 | 负责人实现 → 合并提交 | 每条消息的删除时间由插入触发器盖章（SQLite `strftime(+N seconds)` / PG `make_interval`）——所有插入路径（含定时投递）自动遵守，改/关计时器不影响已发消息（Telegram 语义）。后台清扫每 10 s 分批 500 条、落后时连续清空，幂等可断点续做；硬删除沿用 TG-204 删话题的 `ON DELETE` 级联并重算附件孤儿状态（转发/收藏仍持有的文件不丢）。新增增量帧 `messages_deleted`（旧客户端忽略）；客户端 store 移除行；信息面板「自动删除消息」。双适配器测试（未到期不删、到期只删计时期间的消息、关后新消息不删、幂等、单聊双方可设、群组需 `chat.info`）；合并后 main 138 个二进制 576/0。 |
| TG-207 慢速模式 | 负责人实现 → 合并提交 | 在 `resolve_post_topic`（所有实时发送路径共用的发帖闸门）内强制——一处生效、新发送路径自动继承；定时发送走不限速的 `resolve_scheduled_post_topic`。上次发言取发送者在该聊天的 `MAX(created_at)`（含已撤回，防删了重发），新索引 `(room_id, sender_id, created_at)`；群主/管理员豁免；`members.ban` 才能改间隔（Telegram 七档），群设慢速即升级 supergroup。客户端：`useSlowMode` 倒计时、发送禁用、「慢速模式已开启 · 还需等待 m:ss」条、权限页间隔选择器。双适配器测试 2/2；分支 137 个二进制 574/0。 |
| TG-402 圆形视频消息 | 接管后 → `a79e53a` | 摄像头圆形取景（非正方形居中裁剪）、60 s 上限与进度环、与语音同款手势、录音键 mic↔camera 切换、服务端嗅探容器并要求视频轨、WebM/MP4（含 fragmented）时长解析、可选 JPEG 缩略图、圆形播放气泡、已看状态、`camera=(self)`。原 owner 在写浏览器 E2E 时中断（36 文件未提交）；负责人补话题闸门、修快照字段与一处帧序假设（回复目标自己的广播可能先到）。分支 136 个二进制 571 过（修复后该测试 5/5）。遗留：浏览器 E2E 未完成（真实 Chromium 录制的视频作为测试夹具覆盖了服务端）。 |
| TG-403 相册 / 媒体组 | 接管后 → `4f8c033` | 原 owner 完成后端（两阶段原子发送、整组撤回、`ForwardPlan` 转发保组）后在未提交状态中断。负责人接管：补话题闸门；双适配器测试（原子有序、说明在首项、任一上传未完成则零残留、单项拒绝、转发换新组 id、整组撤回、他人不能撤回）；**前端全部**——Telegram Desktop `LayoutMediaGroup` 马赛克算法移植为纯 core 函数（9 测试 896 断言：无重叠、不越界、每行满宽、四角标记、2/3/4/5–10 形态与极端比例）、列表把连续同组消息折叠为一行（WeakMap 附带成员、首项 key 稳定、按成员跳转）、相册气泡、Composer 2–10 张媒体走原子相册发送（先传完全部分片再一次 `POST /albums`，失败整组标失败可续传）、删除/转发/选择作用于全组。分支 131 个二进制 557/0。已知：附件无宽高元数据，马赛克随图片加载重排一次。 |
| TG-204 话题（论坛模式） | 接管后 → `794ac96` | 原 owner 在 58 个文件未提交时中断；负责人 WIP 提交、修补半截编辑的测试辅助、合并 main（与 TG-202/401/404/305/205 冲突：SQL 列表与占位符 `$10` 手工合并、`forward_favorite` 先 TG-202 完整权限判定再 TG-204 话题落点）。**发现并补上缺口**：语音/GIF/定时三条后写的发送路径都绕过了话题闸门且无法指定话题 → 三处加 `topic_id` 与 `resolve_post_topic`、迁移 `20270101000011` 给 `scheduled_messages` 加话题列（投递时复查关闭）、客户端经 `activeTopicId` 传参、新测试钉住定时路径。分支 130 个二进制 555/0；合并前 main 126 个二进制 **543/0（首次零失败）**。 |
| TG-111 确定性测试基础设施 | 接管后 → `2fb1540` | 源头修复内存 SQLite 被换成空库：具名 shared-cache 内存库 + 独立锚连接（仅测试/开发路径）；Redis 限流测试窗口 2 s → 600 s 并直接断言两实例共享的 Redis 计数器 = 3（证明力不减）；TG-505 的文件库绕行已撤回。原 owner 因 agent 故障中断，负责人接管验证：回归测试 4/4、Redis 测试 4/4、此前抖动的三套件循环 10 次零失败。**已知抖动测试清单由此清空。** |
| TG-202 频道广播 | 接管后 → `9d8eea6` | 频道内 `message.send` 判定为 `message.post`（`ChatType::effective_permission`），所有发送路径一处生效——负责人补测试钉住后来合入的定时发送路径同样被拦（语音/GIF 经同一 `has_chat_permission`）。浏览量批量帧、签名、订阅者计数事务维护。原 owner 两个提交后中断，负责人合并 main、修 `voice_frame_snapshot_test` 字段、写 handoff；main 合并后 125 个二进制 539/0。 |
| TG-305 GIF | `d61abd3` → 合并提交 | 无第三方 GIF 源（按指示）；无转码器（仓库与镜像都不带 ffmpeg）→ 接受真 .gif 与无音轨短 MP4/WebM，决策见 devlog。收藏 GIF 计入孤儿文件存活引用。新增 `registerMessageMenuItem` 消息菜单扩展点。 |
| TG-205 邀请链接 | `9de6faf` → `9278340` | 43 字符随机 token；40 人并发抢 5 个名额恰好放行 5 人（双适配器）；审批链接复用既有加入请求队列；旧接口不变。负责人已挂载：信息面板群组显示 `ChatAdminEntry`（TG-201 补丁）、成员页改用服务端 keyset 分页、管理面板挂 `InviteLinksEntry variant="overlay"`（非 overlay 会双 sheet 抢焦点崩溃）；贴纸管理页经 `registerSettingsPage` 进「外观」。遗留：匿名访问 `/joinchat/…` 被导去登录后 token 丢失。 |
| TG-404 定时与静默发送 | `3bd485f` → 合并提交 | 定时消息存独立 `scheduled_messages` 表、投递时才以同一 id 写入 `messages`——历史/搜索/未读/预览/通知/TG-502 弹出触发器都天然看不到它；重启与双实例下恰好投递一次（双适配器测试）。静默发送跳过提及/回复通知与 Web Push，仍计未读。暂不支持定时发送媒体/贴纸/投票/转发。合并冲突为三方追加（sticker/voice/silent 字段、restriction sweeper 与 scheduled dispatcher 两个后台任务、Composer 的 `ScheduledEntry`+带 `chatId` 的 `AttachMenu`）。 |
| TG-401 语音消息 | `35ea6f4` → 合并提交 | 真实 Chromium 假麦克风 11 步 E2E 通过，并揪出两个真缺陷：`Permissions-Policy` 整体禁用了麦克风；MP4 时长解析错误。波形由录音端计算、服务端校验（100 点 0–31）并从容器读时长——纯 Rust 无法解码 Opus/AAC，决策见 devlog。合并冲突要点：`message.ts` 两个新接口块直接拼接会丢右花括号、`media_kind` 两侧各声明一次 → 负责人手工合并。**集成待办**：会话列表按扩展名把语音猜成「视频/音频」，需在最后消息摘要带 `media_kind`；转发语音进单聊不校验接收方语音隐私。 |
| TG-201 超级群与权限体系 | `b764c43` → 合并提交 | 14 个新权限键（共 23）、按成员限制与管理员自有权限的判定读写双向生效、限制到期即失效且 30 s 清理、keyset 成员分页（20 万成员每页 SQLite 1–7 ms / PG 2–6 ms，OFFSET 同深度 73–121 ms）、四种触发的单向 supergroup 升级；**`member_count` 由触发器维护**（修复 TG-100/106 报告的「1 位成员」）。已注册未强制的键（post、编辑/删除他人、投票、链接预览）交 TG-202 等。合并时 TG-110 新增的 `poll_edit_test` 仍用旧 `edit_message` 签名（TG-304 加了 entities 参数）→ 负责人修正。 |
| TG-505 合并后修复 ×2 | `73bac51`、`1eedac5` | ① presence 测试假设了服务端不保证的帧序 → 改为等待 owner 帧后再发 marker（仍证明 hidden 不可见）；② 间歇 500 = **内存 SQLite 在 WebSocket 中途取消查询时被连接池换成一个全新空库**（生产的文件 SQLite 与 PG 不受影响）→ 该套件改用文件 SQLite；源头修复交 TG-111。 |
| TG-110 集成批次 2 + 设置外壳 | `925cc67` → 合并提交 | 投票实时更新与附件菜单入口、服务端拒绝编辑投票消息（双适配器测试）、信息面板开合、`@tg/ui` Radio 无 label 点击修复、**设置面板外壳**（`registerSettingsPage` 注册 API，M5 各任务自挂页面；隐私与 2FA 已挂；设备会话管理）。E2E 扩展到 22/22。合并冲突：`actions.rs` 仅注释冲突——负责人确认 SQL 同时保留 TG-304 的 entities 替换与 TG-110 的「投票不可编辑」条件；快照测试取 main 的拆分版。遗留：设置打开时侧栏仍可被 Tab 聚焦。 |
| TG-303 贴纸面板 | `9acc8d2` → 合并提交 | 表情/贴纸/GIF 三合一面板（GIF 由 TG-305 经 `registerMediaPanelTab` 填入）、emoji 建议 62–70 ms、300 贴纸基准只拉取 ~37 个、暖开零长任务。**触发负责人修订包体门禁**：总量达 464 KB（其中 234 KB 为懒加载块），单一总量预算已无法区分「首屏变重」与「功能多了懒块」→ 拆为首屏 ≤300 KB（当前 230 KB）+ 全量 ≤1.5 MB，理由写在 `scripts/check_web_bundle.py` 文档串。 |
| TG-502 归档区 | `94d028a` → 合并提交 | 「新消息弹出归档」由数据库触发器实现（迁移 `20270201000009`，双适配器），覆盖全部 9 条插入路径；静音者与发送者本人不弹出。负责人合并树 bun：web 461/0。**给 TG-404 的约束**：定时消息若在投递前就插入 `messages`，会提前触发弹出——TG-404 必须让触发器只对已投递消息生效。M6 压测要看该触发器在大群上的 `EXPLAIN`。 |
| TG-406 投票与测验 | `bcd94c0` → `0b0c562` | 1000 并发投票仅 7 个广播帧（250 ms 聚合）；匿名投票在 REST/历史/WS 均不泄露投票人；非成员 404。**合并冲突要点**：TG-406 把 `forward_message` 从 `store.rs` 移到新 `forward_store.rs`，而 main 上 TG-505 已给它加了转发署名隐私覆盖（`$8`）——机械取任一侧都会丢掉隐私规则；负责人手工移植到 `forward_store.rs`。补丁 A/B（`poll_updated` 订阅、附件菜单投票入口）与「禁止编辑投票消息」交 TG-110。 |
| TG-304 自定义 emoji | `e72c108` → `6d126a1` | 消息 `entities`（UTF-16 偏移，预留粗体/链接等类型）贯通 REST/WS/编辑；复制得到回退 emoji（真实 Chromium 验证）。与 TG-302/406 在消息结构体上三方追加冲突，负责人保留全部字段；`ws_frame_snapshot_legacy_test.rs` 因此到 351 行 → 按方向拆出 `ws_frame_snapshot_legacy_inbound_test.rs`（未调基线）。未应用的 UI 挂载点见其 devlog，交后续集成。已知：emoji 状态可见性按共同聊天而非隐私矩阵；状态变更无实时推送。 |
| TG-306 静态与视频贴纸 | `b4ace09` → 合并提交 | `<Sticker>` 统一分派 TGS/WebP/WebM，WebM 与 TGS 共用视口/隐藏/reduced-motion/并发上限；Safari 兜底按 UA 保守判定。零卡外编辑。 |
| TG-100 M1 集成接线 | `1be8aea` → `bf7880d` | 气泡进虚拟列表、全部动作绑定、选择栏与转发、Composer/MediaViewer/头部状态挂载、`prependHistory` 让 REST 加载的消息接收编辑/撤回/回应、补发 `read` 帧（此前从未发送，双勾与未读永不更新）。agent 自带 16 步双账号 E2E（`packages/web/test/e2e/m1-chat.e2e.mjs`）对真实服务器通过。负责人合并树 bun：core 248/0、ui 103/0、web 358/0。遗留：已读判定按「可见即读」；置顶消息无展示；加入/离开系统消息不入时间线（既有行为）。 |
| TG-106 右侧信息面板 | `74ab520` → `4e61ffa` | 开合面板时消息列表逐帧 0 px 位移（`chatInfo/panelAnchor.ts`，依赖 TG-101 的 DOM 属性名）；三种头部、六个独立游标分页。负责人待办：`WorkspaceShell` 常挂 `<InfoPane/>`、头部按钮切换、删 `shell.css` 旧面板样式。后端缺口：`/files` 无语音/GIF/纯媒体过滤；无链接索引；成员列表不分页 → 交 M2/M4。 |
| TG-208 单聊路径统一 | `0a19c23` → `75ae908` | 审计结论：消息读写本就同路径，重复在建聊天 SQL 与 9 处「是否单聊」反查。`direct_conversations` 只剩查找；`/api/chats` 以 `chat_type:"private"` 列出单聊；`/api/rooms` 仍排除单聊保护冻结客户端。 |
| TG-301 TGS 渲染 | `698d50d` → `2ce9f3b` | canvas「light」构建（无 eval，符合 CSP）+ worker 池；20 个不同贴纸页面 59.8 fps、零长任务、离屏 CPU 0；贴纸自身 20–30 fps 在负载 23–30 的机器上测得，需安静机器复测。lottie 懒加载。 |
| TG-505 隐私矩阵 | `6291637` → `ecd8e78` | 合并树上 `privacy_rules_test::hidden_viewers_receive_no_live_presence_of_the_owner` **确定性失败**（3/3，分支上通过）→ 已退回原 agent 对 main 诊断修复。last-seen 持久化与模糊化已落地；冻结 Vue 客户端免登录拉头像，对受限者不再显示。 |
| TG-506 两步验证 | `362486b` → `a2c73a5` | 无 2FA 账号登录响应不变；有 2FA 返回 428 + 一次性 pending token；开启终止其他会话（D-010）。服务器无邮件传输，恢复邮箱接口返回 503 直到部署注入 mailer。与 TG-505 在 `routes.rs`/`api_doc.rs`/core `api/index.ts`/`styles/index.css` 的纯追加冲突由负责人保留双方。 |
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
| TG-100 M1 集成接线（负责人新增） | M | — | **merged** `bf7880d` | agent:TG-100 | TG-101..105,107 |
| TG-110 集成批次 2 + 设置面板外壳（负责人新增） | M | — | **merged** | agent:TG-110 | TG-106,406,505,506 |
| TG-111 确定性测试基础设施（负责人新增） | S | — | **merged** `2fb1540` | agent:TG-111 | — |
| TG-101 虚拟消息列表 ← 最高风险 | XL | A | **merged** `a2da0a2` | — | TG-012 |
| TG-102 三栏布局与会话侧栏 | L | B | **merged** | — | TG-012 |
| TG-103 消息气泡系统 | L | C | **merged** `6108b90` | — | TG-012 |
| TG-104 输入框 | L | B | **merged** | — | TG-102, TG-008 |
| TG-105 媒体查看器 | M | C | **merged** `db1afad` | — | TG-103 |
| TG-106 右侧信息面板 | M | A | **merged** `4e61ffa` | — | TG-101 |
| TG-107 粒度输入状态与在线状态 UI | S | B | **merged** `4181c4f` | — | TG-007 |
| TG-108 动效审计与 reduced-motion | M | C | **merged** | — | M1 其余项 |

## M2 社交骨架（3 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-201 超级群与权限体系 | XL | A | **merged** | — | M1 |
| TG-202 频道广播语义 | L | B | **merged** `9d8eea6` | — | TG-201 |
| TG-203 频道评论区 | M | B | **merged** | — | TG-202 |
| TG-204 话题（论坛模式） | L | C | **merged** `794ac96` | — | TG-201 |
| TG-205 邀请链接体系 | M | A | **merged** `9278340` | — | TG-201 |
| TG-206 公开 username 与聊天发现 | M | C | **merged** | — | TG-201 |
| TG-207 慢速模式与成员限制 UI | S | A | **merged** | — | TG-201 |
| TG-208 单聊路径统一 | M | B | **merged** `75ae908` | — | TG-005 |

## M3 表达力（2 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-301 TGS 解码与 Lottie 渲染 ← 性能风险 | L | A | **merged** `2ce9f3b` | — | M1 |
| TG-302 贴纸数据模型与服务端 | L | B | **merged** | — | M1 |
| TG-303 贴纸面板 | L | A | **merged** | — | TG-301, TG-302 |
| TG-304 自定义 emoji | M | B | **merged** `6d126a1` | — | TG-302 |
| TG-305 GIF | M | A | **merged** | — | TG-303 |
| TG-306 静态与视频贴纸 | S | B | **merged** | — | TG-301, TG-302 |

## M4 消息能力（4 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-401 语音消息 | L | A | **merged** | — | M1 |
| TG-402 圆形视频消息 | M | A | **merged** `a79e53a` | — | TG-401 |
| TG-403 相册 / 媒体组 | M | B | **merged** `4f8c033` | — | M1 |
| TG-404 定时发送与静默发送 | M | B | **merged** | — | M1 |
| TG-405 自毁计时器 | M | C | **merged** | — | M1 |
| TG-406 投票与测验 | L | C | **merged** `0b0c562` | — | M1 |
| TG-407 位置与实时位置 ← 需选型确认 | M | D | **merged** | — | M1 |
| TG-408 链接预览 ← 需安全评审 | M | D | **merged** | — | M1 |
| TG-409 引用片段与跨聊天回复 | M | B | **merged** `0354b4b` | — | TG-103 |
| TG-410 联系人名片与消息翻译 | S | D | **merged** | — | M1 |
| TG-411 消息效果与动画 | S | C | **merged** | — | TG-108 |

## M5 组织与设置（4 路并行）

| 任务 | 规模 | 组 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-501 聊天文件夹 | L | A | **merged** | — | TG-102 |
| TG-502 归档区 | S | A | **merged** | — | TG-102 |
| TG-503 Saved Messages ← 需决策确认 | M | B | **merged** `9987b58` | — | TG-208 |
| TG-504 全局搜索分栏 | M | B | **merged** | — | M3, M4 |
| TG-505 隐私设置矩阵 | L | C | **merged**（含两轮合并后修复） | — | TG-107 |
| TG-506 两步验证云密码 | M | C | **merged** `a2c73a5` | — | M0 |
| TG-507 主题与聊天背景 | L | D | **merged** | — | TG-009 |
| TG-508 通知例外与自定义声音 | M | D | **merged** | — | M1 |
| TG-509 数据与存储 | M | A | **merged** | — | M4 |
| TG-510 多语言 | M | B | **merged** | — | M1 |
| TG-511 多头像与二维码名片 | S | C | **merged** | — | M1 |

## M6 收口（串行）

| 任务 | 规模 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- |
| TG-601 PWA 强化 | M | **merged** | — | M5 |
| TG-602 删除旧客户端与 alias | M | **merged** | — | M5 |
| TG-603 PySide6 桌面端处置 ← 需用户决策 | S | **merged** | — | TG-602 |
| TG-604 性能压测 | L | **merged** | — | M5 |
| TG-605 `packages/core` 抽离验证 | M | **merged** | — | M5 |
| TG-606 无障碍与键盘操作审计 | M | **merged** | — | M5 |
| TG-701 聊天资料与生命周期 | M | **merged** | — | M6 |
| TG-702 联系人 | M | **merged** | — | M6 |
| TG-703 通知中心 | S | **merged** | — | M6 |
| TG-704 账号安全补齐 | S | **merged** | — | M6 |
| TG-705 管理后台 | M | **merged** | — | M6 |
| TG-706 聊天任务与审计日志 | S | **merged** | — | M6 |

- **2026-10-01 TG-801 合并**：用户报告「加了好友不可以聊天、显示慢速模式已开启」是真缺陷——`useSlowMode` 的陈旧 `now` 让无慢速模式的聊天出现永不倒数的假等待、发送键被锁；撤回修复时 E2E 第 6 步复现失败，加回后通过。另修联系人页实时刷新、搜索 400/429 静默、备注无法取消等 6 项。**既有问题**：`main` 首屏包 325131 B gzip 超出 300000 预算（非 TG-801 引入，+156 B），需单独懒加载任务。

- **2026-10-01 TG-802 合并**（负责人主线程串行，agent 仍不可用）：服务端为会话摘要与账号 `new_message` 帧推导 `media_kind`（13 种），转发语音/圆形视频进单聊时校验对方 `voice_messages` 规则（`skipped_reason: voice_messages_restricted`，撤掉守卫后新测试在第 99 行失败、加回通过）。门禁：`cargo nextest run` 616/616、clippy 净、bun core 348 / web 783 / ui 119。顺手修了 TG-801 留在 `main` 上的 Prettier 漂移（lint 当时是红的）。首屏包 326158 B 仍超预算（既有问题），下一个任务处理。**自此 Rust 测试一律用 `cargo nextest run`（用户 2026-10-01 指示）。**

## M8 缺陷收口与 Telegram 对标（4 路并行）

| 任务 | 规模 | 状态 | Owner | 依赖 |
| --- | --- | --- | --- | --- |
| TG-801 联系人修复与 Telegram 化 | M | **merged** `1de7f35` | lead | — |
| TG-802 会话列表媒体摘要与转发隐私 | S | **merged** `bd31980` | lead | — |
| TG-803 共享内容分类与成员分页 | M | in-progress | agent | — |
| TG-804 Telegram 对标走查（报告） | M | in-progress | agent | — |

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

## 基础设施事故

- **2026-10-01** 运行中的 5 个 agent（TG-111/202/204/402/403）在同一时刻因 API 400「A maximum of 4 blocks with cache_control may be provided. Found 5」终止；随后任何带工具调用的新 agent（含 Sonnet/Opus、不同类型）第二次请求即失败，无工具调用的 agent 正常。属本会话 agent 基础设施问题，非任务代码问题。五个 worktree 的未提交工作均保留在磁盘上（TG-204 58 个文件、TG-402 36、TG-403 15、TG-111 5、TG-202 2 提交 + 2 文件），`.claude/tg-agent-brief.md` 已加「接管中断任务」规程。恢复前由负责人在主线程直接推进。

- **2026-10-01（第二次）** 新会话里派出的 7 个后台 agent（TG-801/802/803/804/805 与一个只跑两条 `ls` 的 Haiku 探针）全部在第二次请求时报同一个 400（cache_control 块 5 > 4），与模型和任务无关。TG-802/803/804/805 未创建 worktree，无残留。TG-801 由负责人在主线程完成。

## 集成负责人自己引入的缺陷

- **2026-10-01** 合并 TG-204 前在 worktree 里用**绝对路径**调用 `scripts/check_file_sizes.py`——该脚本按自身位置解析仓库根，所以审计的是主 checkout 而非 worktree，「通过」毫无意义。合并后 `main` 有三个文件越过 350 行（`api_doc.rs`、`favorites/store.rs`、`chatSession.ts`），TG-403 的门禁才暴露。已按职责拆分：TG 阶段 OpenAPI 条目移入 `api_doc_tg.rs`（在 `compat.rs` 唯一生成点 merge）、收藏转发移入 `favorites/forward_store.rs`、会话类型移入 `chatSessionTypes.ts`。教训：在 worktree 里用 worktree 自己的 `scripts/` 相对路径跑审计。

- **2026-10-01** 合并 TG-305 后只跑了 bun 门禁就合并 TG-205 并挂载 UI，cargo 门禁到最后才跑——才发现 `main` 自 TG-305 合并起**无法编译**：TG-305 基线早于 TG-201，其 GIF 发送调用 `authorize_upload` 少了 TG-201 新增的权限参数。已修（GIF 用 `message.send_sticker`，与 Telegram「贴纸与 GIF」同权）。教训：含 Rust 的分支合并后先跑 `cargo clippy` 再合下一个。

- **2026-10-01** 在主 checkout 解决 TG-506 合并冲突时用了 `git add -A`，把本机未跟踪的 `web-v2/`（6136 个构建残留文件，含 node_modules）提交进了合并提交 `a2c73a5`。TG-306 的 agent 发现（文件大小审计 341 个错误）。已在 `36dfcae` 取消跟踪并加入 `.git/info/exclude`。**推送 `dev` 前必须用 index-filter 从 `a2c73a5` 起的历史中彻底清除 `web-v2/`**，否则约 185 MB 垃圾进入远端。教训：冲突解决只 `git add` 冲突文件本身。

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
