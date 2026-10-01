# Telegram Parity 任务卡

事实依据见 `docs/tg/gap-analysis.md`，架构设计见 `docs/tg/architecture.md`，协作规则见 `docs/tg/agent-protocol.md`。

**规模标记** S ≈ 半天 · M ≈ 1–2 天 · L ≈ 3–5 天 · XL ≈ 一周以上或风险不可预估。
**并行组** 同一里程碑内标记相同字母的任务可以同时开工；不同字母之间有依赖。

**迁移号分配**（`AGENTS.md` 要求每个任务族有预留前缀，SQLite 与 PostgreSQL 必须成对）

| 里程碑 | 前缀区间 |
| --- | --- |
| M0 | `202610010000xx` |
| M2 | `202611010000xx` |
| M3 | `202612010000xx` |
| M4 | `202701010000xx` |
| M5 | `202702010000xx` |
| M8 | `202703010000xx`（TG-803 独占） |

---

# M0 地基 — 串行，禁止并行

在 M0 全部合并进 `main` 之前，**不要创建第二个 worktree**。这些任务会动 `Cargo.toml`、`src/models.rs`、`routes.rs`、`build.rs` 和 61 个迁移涉及的表名，任何并行分支都会被撕碎。

M0 的卡上仍标了组字母，但那只表示依赖分层，**不代表可以并行**。唯一的例外是 TG-009（design tokens），它既不碰 Rust 也不碰 `packages/`。

## TG-000 修复红色基线 · S · 组 A ← **必须第一个做**

- **Outcome** `a16f422` 不是绿色提交。在任何 Telegram-parity 工作开始前恢复全绿，否则后续每个任务都无法区分「我弄坏的」和「本来就坏的」。
- **已实测结果**（2026-09-30 在 `a16f422` 上）

  ```
  cargo fmt --all -- --check                      通过
  cargo clippy --all-targets -- -D warnings       通过（冷编译 1m44s）
  python3 scripts/check_migration_parity.py       通过（49 对迁移）
  python3 scripts/check_file_sizes.py             失败 ← 唯一的红灯
    src/ai/mod.rs              427 → 453   超出基线
    src/config.rs              441 → 462   超出基线
    src/config/environment.rs  371         新的 350 行阈值违规
    src/knowledge/rag.rs       353         新的 350 行阈值违规
    web/src/chat.css           355         新的 350 行阈值违规
    （另有 32 个处于基线之下的告警，不阻断）
  cargo test --all-targets --all-features         本次盘点未跑，本任务负责确认
  cd web && bun test / typecheck                  本次盘点未跑，本任务负责确认
  ```

  注意 `clippy --all-features` 与 `--all-features` 的 test 没跑过，本任务要补齐。

- **Work**
  - 按职责拆分上述 5 个文件，**不要靠调高 `scripts/file-size-baseline.json` 来消除错误**。调基线等于删掉这道门禁。
  - `web/src/chat.css` 属于即将废弃的旧 Vue 客户端：可以例外地把它加进基线豁免并注明「随 TG-602 删除」，这是唯一允许的例外。
  - 跑完 `cargo clippy --all-targets --all-features -- -D warnings` 与 `cargo test --all-targets --all-features`，把结果记进 devlog。若也有失败，一并修复。
  - 在 `docs/tg/board.md` 记录新的**绿色基线提交号**，后续所有任务从它开始。
- **Allowed paths** `src/ai/**`、`src/config*`、`src/knowledge/**`、`scripts/file-size-baseline.json`
- **Acceptance** 六项门禁全部通过并把确切输出记进 devlog；board 上有新的绿色基线提交号。
- **Migration** 无
- **说明** `.github/workflows/ci-cd.yml:85` 确实在 CI 里执行 `check_file_sizes.py`，而 `a16f422` 已推送到 `origin/main` —— 也就是说 **`main` 的 CI 现在是红的**，只是没人盯。这与旧路线图的 `FND-001 修复当前 CI 阻断` 是同一类问题，说明这道门禁会反复变红。TG-000 除了修复，还要说明为什么红了没被发现（分支保护？通知？），否则同样的事会再发生一次。

## TG-013 修正 PostgreSQL 测试的静默跳过 · S · 组 A

- **Outcome** 本地缺 PostgreSQL 时测试**失败或明确报告跳过**，不再在全绿门禁下悄悄不跑。
- **已实测的问题**（2026-09-30，`fb818ee`）

  ```
  tests/migration_upgrade_test.rs:157 的兜底 URL
    postgresql://postgres:postgres@localhost:52735/postgres
  docker-compose.local.yaml 实际映射
    127.0.0.1:52735 凭据 chatroom:chatroom
  ```

  兜底的凭据是错的，所以永远连不上，所以永远跳过。TG-000 报告的 257 个测试通过里，**PostgreSQL 路径一次都没跑**。设上 `TEST_POSTGRES_ADMIN_URL` 后 `postgres_upgrades_from_the_pre_fnd_002_schema` 确实执行并通过 —— 已验证。

- **Work**
  - 把错误的兜底凭据改对，或干脆删掉兜底、让缺变量时明确 skip 并在测试输出里显眼地说「PostgreSQL 未验证」。倾向后者：一个连不上的兜底比没有兜底更坏，因为它把「配错了」伪装成「没配」。
  - 端口是 compose 分配的随机端口，不要硬编码。从 `docker compose port postgres 5432` 读，或在 compose 里固定端口。
  - 至少有一个 `--all-targets` 级别的提示：跑完后打印有多少个测试因为缺 PostgreSQL 而跳过。静默跳过是这个缺口的本质。
  - 同一处检查 Redis 与 Qdrant 的测试有没有同样的静默跳过模式。
- **Allowed paths** `tests/**`、`docker-compose.local.yaml`、`docs/stress-testing.md`
- **Acceptance** 故意停掉 postgres 容器后跑测试，输出里能一眼看到 PostgreSQL 未被验证；容器在时确实执行。
- **Migration** 无

## TG-001 回收构建目录并验证共享配置 · S · 组 A（依赖 TG-000）

- **Outcome** 所有 worktree 共用一个构建目录，磁盘从 194 GB 降到正常量级，CI 与 Docker 不受影响。
- **Work**
  - 删除仓库内旧 `target/`（194 GB），确认 `.cargo/config.toml` 指向的共享目录被创建。
  - 全量 `cargo build --all-targets` 与 `cargo test --all-targets` 各跑一次，记录新目录的真实体积到 devlog。
  - 安装 `cargo-sweep`，把 `cargo sweep --time 14` 写进 `scripts/` 并在本文件记录执行周期。
  - 验证 CI 与 Docker 因为 `CARGO_TARGET_DIR` 覆盖而不读 `.cargo/config.toml`。**`Dockerfile` 的 `ENV CARGO_TARGET_DIR` 靠阅读确认，不在本地构建镜像验证** —— 镜像构建归 CI。
- **Allowed paths** `.cargo/**`、`scripts/**`、`Dockerfile`、`.github/workflows/**`、`docs/tg/**`
- **Acceptance** 共享目录体积记录在案；CI 绿；两个 worktree 同时 `cargo check` 时第二个显示等锁而非报错。
- **Migration** 无

## TG-002 monorepo 骨架 · M · 组 A

- **Outcome** bun workspaces 就位，三个包能各自 build 与 typecheck，CI 纳入新包。
- **Work**
  - 根 `package.json` 声明 `workspaces: ["packages/*"]`。
  - 建 `packages/core`、`packages/ui`、`packages/web` 三个包，各自 `package.json` + `tsconfig.json`，`@tg/*` 命名。
  - `packages/web` 用 Vite + React 19，最小可运行页面。
  - 写 CI 检查：`packages/core` 不得 import `react` / `react-dom` / `@tg/ui` / `@tg/web`，不得引用 `window` / `document` / `localStorage` / `navigator`。用一个可执行的测试实现，不要只写在文档里。
  - `.github/workflows/ci-cd.yml` 增加三包的 typecheck / lint / test 步骤。
- **Allowed paths** `package.json`、`packages/**`、`tsconfig*.json`、`.github/workflows/**`、`.gitignore`
- **Out of scope** 不动 `build.rs`（TG-003 做）；不搬任何业务代码（TG-007 做）。
- **Acceptance** `bun install && bun run -F '*' typecheck` 通过；边界检查测试在故意违规时会失败。
- **Migration** 无

## TG-003 `build.rs` 切换嵌入目标 · S · 组 B（依赖 TG-002）

- **Outcome** 服务端二进制嵌入 `packages/web/dist`，旧 `web/` 不再参与 Rust 构建。
- **Work** 改 `build.rs` 的 rerun-if-changed 列表、`bun install` 目录、`bun run build` 目录与产物路径；更新 `Dockerfile` 的 COPY 列表；更新 `README.md` 的快速开始。
- **Allowed paths** `build.rs`、`Dockerfile`、`README.md`、`docs/container-deployment.md`
- **Acceptance** `cargo build --release --bin server` 成功且 `http://127.0.0.1:3000` 返回新 React 页面。**镜像构建不在本地做** —— 见下方说明。
- **镜像验证由 CI 负责，不在本地执行**（集成负责人，2026-09-30）。`.github/workflows/ci-cd.yml` 的 `image: Build container image` job 用 buildx 加 GitHub Actions 缓存构建镜像（`push: false`），且不带 `if:` 条件，因此在 `agent/**` 分支推送时同样会跑。本地 `docker build` 是冗余的、慢的、且缓存不共享。改 `Dockerfile` 的任务只需保证改动本身可推理，把镜像能否构建交给 CI 回答。
- **Migration** 无

## TG-004 Chat 数据模型迁移 · L · 组 C（依赖 TG-001）

- **Outcome** `rooms` 成为 `chats`，四种 `chat_type` 落地，公开 username 与频道关联字段就位。
- **Work**
  - 迁移 `202610010000 01`：表重命名 `rooms`→`chats`、`room_participants`→`chat_participants`、`room_memberships`→`chat_members`、`room_roles`→`chat_roles`、`room_role_permissions`→`chat_role_permissions`、`room_permissions`→`chat_permissions`、`room_reads`→`chat_reads`、`room_pins`→`chat_pins`、`room_bans`→`chat_bans`、`room_tasks`→`chat_tasks`。SQLite 侧注意 `PRAGMA legacy_alter_table` 关闭时 FK 引用会自动跟随重命名，**必须写测试验证**。
  - 迁移 `..02`：`chats` 增列 `chat_type`、`username`、`access_hash`、`is_forum`、`linked_chat_id`、`slow_mode_seconds`、`auto_delete_seconds`、`signatures_enabled`、`history_visible_to_new_members`、`member_count`；`name` 改 `title`。
  - 迁移 `..03`：数据回填 —— `direct_conversations` 里出现的 chat 打 `private`，其余打 `group`；`access_hash` 随机生成；`member_count` 从 `chat_members` 回填。
  - `chats_username_active_idx` 部分唯一索引。
  - 消息表其他列名里的 `room_id` **不改**（`messages.room_id` 保留），避免触及 61 个迁移的索引与 30 个模块的查询。这是一个有意的不一致，记录在 `CONTEXT.md`。
- **Allowed paths** `migrations/**`、`migrations-postgres/**`、`tests/**`
- **Acceptance** `scripts/check_migration_parity.py` 通过；SQLite 与 PostgreSQL 的**全新建库**与**从 `a16f422` 升级**两条路径都有测试且通过；重命名后所有既有外键仍生效（专门的 FK 测试）。
- **Migration** `20261001000001` / `..02` / `..03`

## TG-005 Rust 模块与类型重命名 · L · 组 D（依赖 TG-004）

- **Outcome** 代码里不再有 `Room` 这个词指代聊天，词汇表更新。
- **Work**
  - `src/rooms/` → `src/chats/`；`RoomId`→`ChatId`、`StoredRoom`→`Chat`、`RoomRole`→`ChatRole` 等类型重命名。
  - `ChatType` 枚举，含 `group → supergroup` 单向升级方法与触发条件（超 200 人、启用 username、启用话题、设置慢速模式）。
  - 权限判定加入 `chat_type` 固有约束层（见 `architecture.md` §4.4 的判定顺序）。
  - `CONTEXT.md` 词汇表：新增 Chat / Chat Type / Supergroup Upgrade / Public Username 条目，`Room` 条目改为指向 Chat 并注明 `messages.room_id` 的历史遗留。
- **Allowed paths** `src/**`、`CONTEXT.md`、`tests/**`
- **Out of scope** 不改 HTTP 路径（TG-006 做）。
- **Acceptance** `cargo clippy --all-targets -- -D warnings` 零告警；`cargo test --all-targets` 全绿；`grep -rn "RoomId\|StoredRoom" src/` 无结果。
- **Migration** 无

## TG-006 API 路径重命名与 deprecated alias · M · 组 D（依赖 TG-005）

- **Outcome** `/api/chats/*` 是正式契约，`/api/rooms/*` 作为转发别名存活到 M6。
- **Work** `routes.rs` 新增 `/api/chats/*`；`/api/rooms/*` 仅做路由转发，不复制逻辑；响应体字段 `room_id`→`chat_id`、`name`→`title`，alias 路径返回兼容字段；utoipa 文档标注 deprecated；`docs/client-capability-matrix.md` 更新。
- **Allowed paths** `src/routes.rs`、`src/chats/**`、`docs/client-capability-matrix.md`、`tests/**`
- **Acceptance** 新旧路径行为等价的契约测试；ratatui CLI（`cargo run --bin client`）仍能登录、建群、收发消息、传文件。
- **Migration** 无

## TG-007 WebSocket 帧扩展 · M · 组 D（依赖 TG-005）

- **Outcome** 一次性做完所有破坏性帧变更，后续里程碑只增不改。
- **Work**
  - `ChatMessage::Typing` 扩展为带 `action` 字段的粒度状态：`typing` / `recording_voice` / `recording_video_note` / `uploading_photo` / `uploading_video` / `uploading_document` / `uploading_voice` / `choosing_sticker` / `choosing_location`。
  - 新增变体骨架（先定义帧与广播路径，具体业务在各自里程碑填充）：`ChatUpdated`、`MemberUpdated`、`TopicUpdated`、`MessageViewsUpdated`（**批量**）、`PollUpdated`、`DraftUpdated`。
  - `Presence` 扩展 per-user `last_seen` 与隐私分级占位（实际隐私规则在 TG-505）。
- **Allowed paths** `src/models.rs`、`src/realtime/**`、`tests/**`
- **Out of scope** 不实现话题、投票、浏览量的业务逻辑，只定义帧。
- **Acceptance** 协议帧的序列化快照测试；旧 Vue 客户端与 ratatui 客户端对未知帧优雅忽略（验证，不是假设）。
- **Migration** 无

## TG-008 云端草稿 · M · 组 E（依赖 TG-006、TG-007）

- **Outcome** 草稿跨设备同步，不再只存浏览器。
- **Work** `chat_drafts` 表（`chat_id`、`user_id`、`text`、`reply_to_message_id`、`topic_id`、`updated_at`）；`PUT /api/chats/:id/draft`；`DraftUpdated` 帧广播给同一用户的其他连接；写入去抖（服务端幂等，客户端 1–2 秒去抖）。
- **Allowed paths** `src/chats/drafts.rs`、`src/routes.rs`、`migrations*/**`、`packages/core/src/**`
- **Acceptance** 两个连接同一账号，一端输入另一端出现草稿；断线重连后草稿不丢。
- **Migration** `20261001000004`

## TG-009 Design tokens 提取 · M · 组 A（可与 TG-001..008 并行，不碰 Rust）

- **Outcome** 三层 token 体系落地，明暗主题与强调色可运行时切换。
- **Work**
  - 从 Telegram Desktop / Web 反推 `primitive.css`：完整色阶、4px 基准的间距刻度、圆角刻度（气泡 / 卡片 / 头像）、字号与行高、**动效曲线与时长**（Telegram 用 spring，记录刚度与阻尼而非 cubic-bezier）。
  - `semantic.css`：`--tg-bg`、`--tg-bg-secondary`、`--tg-text-primary`、`--tg-text-secondary`、`--tg-accent`、`--tg-bubble-in`、`--tg-bubble-out`、`--tg-divider`、`--tg-overlay` 等。
  - `themes/day.css`、`themes/night.css`，以及至少 3 个强调色覆盖层。
  - 一份 token 对照页（`packages/ui` 的 story 或静态页），肉眼可比对 Telegram 截图。
  - 复用既有 `design/design-tokens.json` 与 `design/tokens.css` 里仍适用的部分，不要从零发明。
- **Allowed paths** `packages/ui/src/tokens/**`、`design/**`
- **Acceptance** 切换主题与强调色时无组件改动；对照页与 Telegram 截图并排可辨认为同一设计语言。
- **Migration** 无

## TG-010 `packages/ui` 基础组件 · L · 组 F（依赖 TG-002、TG-009）

- **Outcome** 19 个原子组件可用，带文档页，不含任何业务概念。
- **Work** `Button` `IconButton` `Ripple` `TextField` `Toggle` `Checkbox` `Radio` `Menu` `ContextMenu` `Popover` `Modal` `Sheet` `Tooltip` `Tabs` `Avatar` `Badge` `Spinner` `Skeleton` `ScrollArea`。Telegram 的 `Ripple` 是从点击位置扩散的，不是 Material 的中心扩散，单独对齐。焦点管理、键盘导航、`prefers-reduced-motion` 三项每个组件都要覆盖。
- **Allowed paths** `packages/ui/**`
- **Out of scope** 任何认识 Chat / Message / User 的组件。
- **Acceptance** 每个组件有文档页与交互测试；键盘可完整操作 `Menu` / `Modal` / `Tabs`；`prefers-reduced-motion` 下动效降级而非消失。
- **Migration** 无

## TG-011 `packages/core` 骨架与业务逻辑迁移 · L · 组 F（依赖 TG-002、TG-006、TG-007）

- **Outcome** 类型、API 客户端、WS 客户端、store 就位；旧客户端里框架无关的逻辑连同测试搬过来。
- **Work**
  - `types/`、`api/`、`realtime/`、`stores/`、`domain/` 五层（见 `architecture.md` §2）。
  - 从旧 `web/src` 迁移约 55 个框架无关模块及其测试，逐个判定保留 / 重写 / 废弃，判定结果写进 devlog。**明确保留**：`calculator`、`markdown`、`chatOptimistic`、`messageViewportPolicy`、`attachmentUploadProgress`、`fileHash`、`avatarColor`、`randomUuid`、`messageDeepLink`、`searchPattern` 对应逻辑。
  - 平台能力用注入接口而非直接调用：`createStorage()`、`createWebSocket()`、`createFileReader()`、`createClock()`。
  - 9 个 store 骨架（见 `architecture.md` §2），Zustand vanilla 形态。
  - WS 客户端：重连退避、订阅管理、帧编解码、断线期间的消息补拉。
- **Allowed paths** `packages/core/**`
- **Acceptance** `packages/core` 的边界检查通过；迁移过来的测试全绿；WS 客户端有断线重连与补拉的测试（用注入的假 socket）。
- **Migration** 无

## TG-012 登录与最小可用壳 · M · 组 G（依赖 TG-010、TG-011、TG-003）

- **Outcome** M0 的完成标志：新 React 客户端能登录、看到会话列表、发一条消息。
- **Work** 路由骨架、登录/注册页、三栏布局壳、最简会话列表、最简消息列表（暂不虚拟化）、最简输入框。目的是打通端到端链路，不追求视觉完成度。
- **Allowed paths** `packages/web/**`
- **Acceptance** `cargo run --bin server` 后浏览器能完成 注册 → 建群 → 发消息 → 刷新后消息仍在 → 第二个浏览器实时收到；CI 全绿。
- **Migration** 无

---

# M1 会话骨架 — 3 路并行

## TG-101 虚拟消息列表 · XL · 组 A ← **全项目风险最高的单点**

- **Outcome** 10 万条消息的聊天流畅滚动，向上加载历史不跳动，能跳转到任意历史消息。
- **Work**
  - `react-virtuoso` 的反向列表模式：`firstItemIndex` + `startReached` 实现 prepend 保锚点。
  - 动态高度：图片、相册、引用块、贴纸的高度在加载前未知，需要占位尺寸预估 + 加载后的平滑修正。
  - 跳转到消息：加载目标附近的窗口、滚动到位、高亮闪烁、保留返回原位置的按钮。
  - 粘性日期分隔头；滚动到底按钮带未读计数；新消息到达时「已在底部则跟随，否则不跟随」。
  - 消息分组（同一发送者连续消息合并头像与时间）的高度计算。
  - **性能预算写进 devlog 并做成基准测试**：滚动时掉帧率、prepend 时的布局抖动像素数、10 万条消息的内存占用。
- **Allowed paths** `packages/web/src/features/messageList/**`、`packages/core/src/domain/messageViewport*`
- **Acceptance** 基准测试达标（阈值由本任务设定并在 devlog 论证）；prepend 时视口锚点位移 0 像素；跳转到 5 万条之前的消息后能返回原位。
- **Depends** TG-012
- **说明** 这个任务不要拆给多个 agent，也不要与 TG-103 合并。它需要独占的注意力和反复的手工验证。

## TG-102 三栏布局与会话侧栏 · L · 组 B

- **Outcome** Telegram 的三栏骨架与会话列表项的全部状态。
- **Work** 可拖拽宽度的侧栏 + 折叠态；顶部搜索栏与汉堡菜单；会话项的全部状态：头像（含在线小圆点）、标题、最后一条消息预览（含发送者前缀、媒体图标、草稿标记）、时间、未读徽章、静音图标、置顶图标、已读双勾、正在输入替换预览；归档区入口行；移动端的单栏切换。
- **Allowed paths** `packages/web/src/features/chatList/**`、`packages/core/src/stores/chatListStore.ts`
- **Acceptance** 与 Telegram 截图逐项比对会话项的 12 种状态；拖拽宽度持久化；移动端视口下栏切换无布局跳动。
- **Depends** TG-012

## TG-103 消息气泡系统 · L · 组 C

- **Outcome** 气泡的全部形态与 Telegram 一致。
- **Work** 气泡尾巴（分组内只有最后一条有尾巴）；in / out 配色；时间戳与已读双勾叠在气泡右下且不与文字重叠；图片气泡的无边距形态与圆角处理；引用回复块；转发来源头；编辑标记；撤回占位；表情回应芯片行；悬停操作条；长按 / 右键上下文菜单；选择模式；系统消息居中样式。
- **Allowed paths** `packages/web/src/features/message/**`
- **Acceptance** 与 Telegram 截图比对 in/out × 文本/图片/相册/引用/转发/编辑 的组合；文字与时间戳在极端文本长度下不重叠。
- **Depends** TG-012

## TG-104 输入框 · L · 组 B（依赖 TG-102）

- **Outcome** Telegram 输入区的完整行为。
- **Work** 多行自适应高度与上限；emoji 按钮与选择器（复用 `emoji-picker-element`）；附件菜单（图片/视频/文件/位置/投票/联系人入口，未实现项禁用而非隐藏）；发送按钮与语音按钮的切换；回复态 / 编辑态 / 转发态的上方条；@提及自动完成；Markdown 快捷格式与选中文本的格式化工具条；草稿读写接 TG-008；粘贴图片与拖拽上传；快捷计算（迁移 `calculator.ts`）。
- **Allowed paths** `packages/web/src/features/composer/**`、`packages/core/src/stores/composerStore.ts`
- **Acceptance** 回复→编辑→取消的状态机有测试；粘贴多张图进入待发列表；草稿在切换会话后保留并同步到第二个设备。
- **Depends** TG-102、TG-008

## TG-105 媒体查看器 · M · 组 C（依赖 TG-103）

- **Outcome** Telegram 式全屏媒体浏览。
- **Work** 从缩略图放大的过渡动画（共享元素）；左右切换同会话媒体；缩放与拖拽；视频播放（复用 `plyr`）；下载、转发、删除操作；底部缩略图条；键盘与手势操作。
- **Allowed paths** `packages/web/src/features/mediaViewer/**`
- **Acceptance** 打开与关闭的过渡不闪白；缩放后拖拽边界正确；键盘 ←/→/Esc 可完整操作。

## TG-106 右侧信息面板 · M · 组 A（依赖 TG-101）

- **Outcome** 聊天信息面板与共享内容分页。
- **Work** 面板从右侧推入的布局联动；单聊 / 群 / 频道三种信息头；共享媒体 / 文件 / 链接 / 语音 / GIF 五个分页（各自独立分页游标）；成员列表（群）与订阅者数（频道）；通知与静音设置入口；搜索本聊天入口。
- **Allowed paths** `packages/web/src/features/chatInfo/**`
- **Acceptance** 五个分页各自独立滚动与分页；面板开合时消息列表不重排（复用 TG-101 的锚点机制）。

## TG-107 粒度输入状态与在线状态 UI · S · 组 B

- **Outcome** 「正在输入」「正在录音」等九种状态在会话列表与聊天头部正确显示。
- **Work** 接 TG-007 的 `action` 字段；多人同时输入的文案合并（「A 和其他 2 人正在输入」）；超时自动清除；聊天头部的 last seen 文案（「上次上线于…」的相对时间规则与 Telegram 一致）。
- **Allowed paths** `packages/web/src/features/presence/**`、`packages/core/src/stores/presenceStore.ts`
- **Acceptance** 九种状态各有文案；发送端停止输入后 5 秒内清除；多人输入的文案在 1/2/3+ 人时分别正确。

## TG-108 动效审计与 reduced-motion · M · 组 C（M1 收尾）

- **Outcome** M1 所有交互的动效与 Telegram 手感一致，且可降级。
- **Work** 逐项校准：侧栏折叠、会话切换、消息进入、气泡长按、面板推入、媒体查看器开合、模态出现。统一用 spring 而非 ease。`prefers-reduced-motion` 下全部降级为透明度变化或瞬时。
- **Allowed paths** `packages/ui/src/tokens/**`、`packages/web/src/**`（仅动效相关）
- **Acceptance** 动效参数集中在 token 层而非散落组件内；reduced-motion 下无位移动画残留。

---

# M2 社交骨架 — 3 路并行

## TG-201 超级群与权限体系 · XL · 组 A

- **Outcome** `supergroup` 可用，Telegram 的管理员权限与成员限制落地。
- **Work** `chat_permissions` 注册表新增 14 个 key（见 `architecture.md` §4.4）；`chat_member_restrictions` 表与到期清理；`group → supergroup` 单向升级；`default_permissions`（群整体的成员默认限制）；成员列表分页（20 万成员不能一次拉）；管理员任命界面与权限勾选；`member_count` 投影的事务内维护。
- **Allowed paths** `src/chats/**`、`migrations*/**`、`packages/web/src/features/chatAdmin/**`、`packages/core/src/api/chats.ts`
- **Acceptance** 读路径与写路径都判定权限的测试；限制到期后自动恢复；升级为 supergroup 后原有成员与消息完整；20 万成员的分页查询有性能测试。
- **Migration** `20261101000001`（权限 key + 限制表）、`..02`（default_permissions）

## TG-202 频道广播语义 · L · 组 B（依赖 TG-201）

- **Outcome** `channel` 可用：仅管理员发布、订阅者、浏览量、作者签名。
- **Work** `message.post` 权限门；订阅者加入不进 `chat_members` 的角色体系而是轻量订阅（20 万+ 规模）；`messages.views_count` 与**批量**的 `MessageViewsUpdated` 帧（逐条广播会打爆连接，必须聚合）；`signatures_enabled` 与 `post_author`；频道信息头与订阅按钮。
- **Allowed paths** `src/chats/**`、`migrations*/**`、`packages/web/src/features/channel/**`
- **Acceptance** 非管理员发布被拒（读写双向）；浏览量在 1000 个订阅者同时浏览时的广播次数有上限测试。
- **Migration** `20261101000003`

## TG-203 频道评论区 · M · 组 B（依赖 TG-202）

- **Outcome** 频道帖子下方的评论区，由关联讨论群承载。
- **Work** `linked_chat_id` 的双向关联与解绑；频道帖子自动转发到讨论群并建立映射；评论数统计与「查看评论」入口；评论视图复用消息列表但限定到该帖的回复链。
- **Allowed paths** `src/chats/discussion.rs`、`packages/web/src/features/channel/**`、`migrations*/**`
- **Acceptance** 解绑后历史评论仍可访问；删除频道帖子时讨论群的对应消息处理有明确定义并测试。
- **Migration** `20261101000004`

## TG-204 话题（论坛模式）· L · 组 C（依赖 TG-201）

- **Outcome** `is_forum` 的超级群可分话题。
- **Work** `forum_topics` 表（含图标、颜色、置顶、关闭状态、创建者）；`messages.topic_id`；话题列表视图与话题内消息视图的双层导航；「General」默认话题；话题级未读与静音；话题级权限（`chat.topics`）。
- **Allowed paths** `src/chats/topics.rs`、`migrations*/**`、`packages/web/src/features/forum/**`
- **Acceptance** 关闭的话题禁止发言（读写双向）；话题级未读独立于聊天级未读；话题删除时消息的处理有定义。
- **Migration** `20261101000005`

## TG-205 邀请链接体系 · M · 组 A（依赖 TG-201）

- **Outcome** 多链接并存、有效期、人数上限、按链接统计、撤销、加入请求。
- **Work** `chat_invite_links` 表（`link`、`creator`、`expires_at`、`usage_limit`、`usage_count`、`requires_approval`、`revoked_at`、`title`）；主链接与附加链接；加入请求队列（复用既有 `join-requests`）；链接管理界面与统计。
- **Allowed paths** `src/chats/invite_links.rs`、`migrations*/**`、`packages/web/src/features/inviteLinks/**`
- **Acceptance** 过期与超限的链接拒绝加入；撤销后立即失效；`usage_count` 在并发加入下不超卖（事务测试）。
- **Migration** `20261101000006`

## TG-206 公开 username 与聊天发现 · M · 组 C

- **Outcome** 超级群与频道可设公开句柄，可通过句柄访问与搜索。
- **Work** username 的可用性校验（长度、字符集、保留词）与唯一性；`t.me/xxx` 形态的公开链接路由；未加入时的预览页（标题、简介、成员数、加入按钮）；`/api/chats/discover` 扩展为按 username 与标题搜索；`access_hash` 的使用（公开解析不泄露内部 id）。
- **Allowed paths** `src/chats/public_handles.rs`、`packages/web/src/features/chatPreview/**`、`migrations*/**`
- **Acceptance** 保留词与非法字符被拒；未加入用户只能看到预览允许的字段（授权测试）。
- **Migration** `20261101000007`

## TG-207 慢速模式与成员限制 UI · S · 组 A（依赖 TG-201）

- **Outcome** 慢速模式生效并有倒计时提示，成员限制可视化管理。
- **Work** `slow_mode_seconds` 的服务端强制（按 `chat_id + user_id` 的最后发言时间）；客户端倒计时与发送按钮禁用；成员限制面板（勾选禁止项 + 到期时间）。
- **Allowed paths** `src/chats/slow_mode.rs`、`packages/web/src/features/chatAdmin/**`
- **Acceptance** 服务端是唯一真相（绕过客户端直接调 API 也被拒）；管理员不受慢速模式约束。
- **Migration** 无（列在 TG-004 已加）

## TG-208 单聊路径统一 · M · 组 B

- **Outcome** `private` 聊天走与群完全相同的读写路径，消除两套逻辑。
- **Work** 审查 `src/direct_conversations/` 与 `src/conversations/`，把重复的读写路径合并到 `src/chats/`；`direct_conversations` 降级为纯粹的「按两个 user 反查 chat_id」索引表；前端不再区分单聊与群的数据获取路径。
- **Allowed paths** `src/direct_conversations/**`、`src/conversations/**`、`src/chats/**`
- **Acceptance** `src/direct_conversations/` 只剩查找逻辑，无消息读写；既有单聊功能的测试全部仍绿。
- **Migration** 无

---

# M3 表达力 — 2 路并行

## TG-301 TGS 解码与 Lottie 渲染 · L · 组 A ← **性能风险高**

- **Outcome** 动态贴纸能渲染，且同屏多个不掉帧。
- **Work** `pako` 解 gzip 得到 Lottie JSON；`lottie-web` 渲染；**同屏多贴纸的性能策略**：视口外暂停、离屏 canvas 复用、首帧静态图占位、同一贴纸多实例共享解析结果、超过阈值时降级为静态首帧。三种渲染器（svg / canvas / html）实测选型并把结论写进 devlog。
- **Allowed paths** `packages/core/src/domain/tgs*`、`packages/web/src/features/sticker/renderer/**`
- **Acceptance** 同屏 20 个动态贴纸的帧率基准达标（阈值本任务设定并论证）；视口外贴纸 CPU 占用接近 0。

## TG-302 贴纸数据模型与服务端 · L · 组 B

- **Outcome** 贴纸包与贴纸的存储、上传、分发。
- **Work** `sticker_sets`、`stickers`、`user_sticker_sets`（安装与排序）、`user_recent_stickers`、`user_favorite_stickers`、`custom_emoji` 表；贴纸文件走既有附件存储（`opendal`，本地 + OSS）；三种格式（WebP / TGS / WebM）的校验与尺寸限制；`messages.media_kind` 列；贴纸消息的发送与渲染契约。
- **Allowed paths** `src/stickers/**`、`migrations*/**`、`src/routes.rs`
- **Acceptance** 三种格式的上传校验测试；贴纸包的安装/卸载/排序有并发测试；贴纸文件的授权与普通附件一致。
- **Migration** `20261201000001`（贴纸表）、`..02`（`media_kind`）

## TG-303 贴纸面板 · L · 组 A（依赖 TG-301、TG-302）

- **Outcome** Telegram 的贴纸/emoji/GIF 三合一面板。
- **Work** 三个顶级分页；已装贴纸包的侧边导航条与吸附滚动；最近使用与收藏；贴纸搜索；**按输入的 emoji 建议贴纸**（输入框里打出 emoji 时上方浮出候选）；长按预览大图；贴纸包管理页（安装/卸载/排序/归档/分享）。
- **Allowed paths** `packages/web/src/features/sticker/**`、`packages/core/src/stores/stickerStore.ts`
- **Acceptance** 面板打开时不阻塞主线程（大量贴纸的懒加载）；emoji 建议在 200ms 内出现。

## TG-304 自定义 emoji · M · 组 B（依赖 TG-302）

- **Outcome** 消息文本内联渲染自定义 emoji，头像旁可显示 emoji 状态。
- **Work** 消息实体（entity）体系：文本中的自定义 emoji 占位与替换渲染；emoji 选择器里的自定义分页；emoji 状态的设置与在会话列表/聊天头部/成员列表的显示。
- **Allowed paths** `src/stickers/custom_emoji.rs`、`packages/web/src/features/customEmoji/**`
- **Acceptance** 内联渲染不破坏文本选择与复制（复制得到 emoji 而非占位符）；未安装的自定义 emoji 有兜底显示。
- **Migration** `20261201000003`

## TG-305 GIF · M · 组 A（依赖 TG-303）

- **Outcome** GIF 收藏、发送与自动播放策略。
- **Work** `user_saved_gifs` 表；GIF 作为 MP4 存储与播放（体积与解码都远优于真 GIF）；面板内的 GIF 分页与瀑布流；自动播放策略（视口内播放、`prefers-reduced-motion` 与省电模式下禁用）；从消息中收藏 GIF。
- **Allowed paths** `src/stickers/gifs.rs`、`packages/web/src/features/gif/**`、`migrations*/**`
- **Acceptance** 视口外 GIF 暂停解码；reduced-motion 下不自动播放。
- **Migration** `20261201000004`

## TG-306 静态与视频贴纸 · S · 组 B（依赖 TG-301、TG-302）

- **Outcome** WebP 与 WebM 贴纸与 TGS 走同一渲染入口。
- **Work** 统一的 `<Sticker>` 组件按格式分派；WebM 贴纸的循环播放与视口暂停；格式不支持时的兜底。
- **Allowed paths** `packages/web/src/features/sticker/renderer/**`
- **Acceptance** 三种格式在同一面板内混排正常；Safari 的 WebM 兜底路径有验证。

---

# M4 消息能力 — 4 路并行

## TG-401 语音消息 · L · 组 A

- **Outcome** 录制、波形、变速播放、已听状态。
- **Work** `MediaRecorder` 录制（Opus）；录制中的波形可视化与取消手势；服务端生成波形峰值数据并随消息下发（客户端解码整段音频算波形在长语音上不可接受）；播放器（进度拖拽、1x/1.5x/2x 变速、连续播放下一条）；已听状态；`recording_voice` 输入状态接 TG-107。
- **Allowed paths** `src/attachments/voice.rs`、`packages/web/src/features/voice/**`、`migrations*/**`
- **Acceptance** 波形由服务端提供且与音频对齐；Safari 的录制格式兜底有验证；已听状态双向同步。
- **Migration** `20270101000001`

## TG-402 圆形视频消息 · M · 组 A（依赖 TG-401）

- **Outcome** Telegram 的 video note。
- **Work** 圆形取景录制（摄像头 + 圆形裁剪）；最长 60 秒；气泡内圆形播放器与点击放大；静音自动播放 + 点击出声的 Telegram 行为。
- **Allowed paths** `packages/web/src/features/videoNote/**`、`src/attachments/**`
- **Acceptance** 圆形裁剪在非正方形摄像头下取中；自动播放策略与 Telegram 一致。
- **Migration** `20270101000002`

## TG-403 相册 / 媒体组 · M · 组 B

- **Outcome** 多张图片视频作为一条消息，马赛克布局。
- **Work** `messages.grouped_id`；一次发送多个媒体的原子性（全成功或全失败）；Telegram 的马赛克布局算法（按宽高比分组排列，不是等分网格）；相册内单项的删除与整组删除；转发整组。
- **Allowed paths** `src/messages/albums.rs`、`packages/web/src/features/album/**`、`migrations*/**`
- **Acceptance** 2/3/4/5/6+ 张的布局与 Telegram 比对；上传中途失败不留半个相册。
- **Migration** `20270101000003`

## TG-404 定时发送与静默发送 · M · 组 B

- **Outcome** 定时投递与不打扰对方的发送。
- **Work** `messages.scheduled_at` 与 `silent`；定时消息不进入正常消息流直到投递（查询要排除）；投递调度（复用 `src/work_queue.rs`）；「定时消息」列表视图与编辑/取消；静默发送跳过通知与推送；发送按钮长按菜单。
- **Allowed paths** `src/messages/scheduled.rs`、`src/work_queue.rs`、`packages/web/src/features/composer/**`、`migrations*/**`
- **Acceptance** 服务重启后定时消息仍会投递；定时消息不出现在普通历史查询与搜索结果里（授权与可见性测试）。
- **Migration** `20270101000004`

## TG-405 自毁计时器 · M · 组 C

- **Outcome** per-chat 的消息自动删除。
- **Work** `chats.auto_delete_seconds` 与 `messages.auto_delete_at`；后台清理任务；设置界面与系统消息提示（「A 将自动删除时间设为 1 天」）；清理时同步删除附件。
- **Allowed paths** `src/messages/auto_delete.rs`、`src/tasks/**`、`packages/web/src/features/chatInfo/**`
- **Acceptance** 清理任务幂等且能从中断恢复；附件与消息一起删除，无孤儿文件。
- **Migration** `20270101000005`

## TG-406 投票与测验 · L · 组 C

- **Outcome** 匿名/公开、单选/多选、quiz 三种模式。
- **Work** `polls`、`poll_options`、`poll_votes` 表；投票的创建界面；实时票数（`PollUpdated` 帧，**聚合广播**）；公开投票的投票人列表；quiz 模式的正确答案与解析；关闭投票；转发投票的语义（Telegram 转发会复制为新投票）。
- **Allowed paths** `src/messages/polls.rs`、`packages/web/src/features/poll/**`、`migrations*/**`
- **Acceptance** 并发投票的票数一致（事务测试）；匿名投票不泄露投票人（授权测试）；1000 人同时投票的广播次数有上限。
- **Migration** `20270101000006`

## TG-407 位置与实时位置 · M · 组 D

- **Outcome** 静态位置与限时共享的移动位置。
- **Work** `message_locations` 表；地图供应商选型（需自托管或注意隐私，**选型结论写进 devlog 并需集成负责人确认**）；静态位置消息；实时位置的定时更新与到期；同一聊天多人实时位置的合并视图。
- **Allowed paths** `src/messages/locations.rs`、`packages/web/src/features/location/**`、`migrations*/**`
- **Acceptance** 实时位置到期后停止更新且历史不可继续追踪；位置精度的隐私提示。
- **Migration** `20270101000007`

## TG-408 链接预览 · M · 组 D ← **有安全风险，需评审**

- **Outcome** 消息中的链接自动展开预览卡。
- **Work** 服务端抓取 OG / oEmbed 元数据；`link_previews` 缓存表；**SSRF 防护**：禁止内网地址、限制重定向次数、限制响应体大小、超时、禁止非 http(s) 协议；抓取走独立的受限出口；发送者可取消预览。
- **Allowed paths** `src/messages/link_previews.rs`、`migrations*/**`、`packages/web/src/features/linkPreview/**`
- **Acceptance** SSRF 用例（`127.0.0.1`、`169.254.169.254`、`file://`、DNS rebinding、重定向到内网）全部被拒且有测试；抓取失败不阻塞消息发送。
- **Migration** `20270101000008`
- **需集成负责人评审通过后才能合并。**

## TG-409 引用片段与跨聊天回复 · M · 组 B

- **Outcome** 引用对方消息的一段，以及跨聊天回复。
- **Work** `reply_quote_text` / `reply_quote_offset`：选中文本后「引用」；引用块渲染与点击跳原消息；原消息被编辑后引用的处理（Telegram 标记为已修改）；`reply_to_chat_id`：跨聊天回复的来源头与跳转。
- **Allowed paths** `src/messages/**`、`packages/web/src/features/message/**`、`migrations*/**`
- **Acceptance** 原消息编辑后引用有「已修改」标记；跨聊天回复在无权访问源聊天时优雅降级（授权测试）。
- **Migration** `20270101000009`

## TG-410 联系人名片与消息翻译 · S · 组 D

- **Outcome** 分享联系人名片；消息翻译入口。
- **Work** 联系人名片消息类型与添加好友入口；翻译走既有 AI 能力（`src/ai/`），不引入新供应商；逐条翻译与整聊天翻译模式。
- **Allowed paths** `src/messages/contacts.rs`、`src/ai/**`、`packages/web/src/features/message/**`
- **Acceptance** 翻译不修改原消息（是投影，符合 `CONTEXT.md` 的原则）；无 AI 配置时翻译入口隐藏而非报错。
- **Migration** `20270101000010`

## TG-411 消息效果与动画 · S · 组 C

- **Outcome** Telegram 的消息发送动效与表情回应动画。
- **Work** 发送时气泡从输入框飞入的过渡；表情回应的迸发动画；大号 emoji 单独发送时的放大动画与点击效果。
- **Allowed paths** `packages/web/src/features/message/**`、`packages/ui/src/tokens/**`
- **Acceptance** 动效全部 reduced-motion 可降级；不引入布局抖动。

---

# M5 组织与设置 — 4 路并行

## TG-501 聊天文件夹 · L · 组 A

- **Outcome** Telegram 的 folder 体系。
- **Work** `chat_folders` + `chat_folder_rules` 表；规则维度：按类型（单聊/群/频道/机器人）、显式包含的聊天、显式排除的聊天、排除已读/静音/归档；文件夹顶部标签栏与左侧竖排两种形态；文件夹的创建/编辑/排序；每个文件夹的未读计数。
- **Allowed paths** `src/chats/folders.rs`、`migrations*/**`、`packages/web/src/features/folders/**`
- **Acceptance** 规则组合的求值有测试矩阵；文件夹未读计数与聊天级未读一致。
- **Migration** `20270201000001`

## TG-502 归档区 · S · 组 A

- **Outcome** Telegram 的归档行为。
- **Work** 归档区作为列表顶部的可折叠行；归档内有未读时的徽章;「静音的归档聊天来新消息不弹回」的规则；滑动归档手势。
- **Allowed paths** `packages/web/src/features/chatList/**`
- **Acceptance** 归档聊天来消息时的弹回规则与静音状态正确组合。

## TG-503 Saved Messages 作为真实聊天 · M · 组 B

- **Outcome** 「收藏」变成一个聊天形态的界面，与既有 `favorites` 共存而非重复。
- **Work** 为每个用户提供一个 `chat_type='private'` 的自聊；既有 `favorites` 数据与之的关系明确定义（**不要造第二套存储** —— 决定是投影还是迁移，结论写进 devlog 并需集成负责人确认）；转发到 Saved Messages 的快捷入口。
- **Allowed paths** `src/favorites/**`、`src/chats/**`、`packages/web/src/features/savedMessages/**`
- **Acceptance** 既有 `favorites` 的全部测试仍绿；不存在同一条收藏的两份真相。
- **Migration** `20270201000002`

## TG-504 全局搜索分栏 · M · 组 B

- **Outcome** Telegram 的搜索结果分栏。
- **Work** 聊天 / 消息 / 媒体 / 链接 / 文件 / 音乐 / 语音 七个分页；各自独立分页游标；按发送者与日期筛选；最近搜索与建议；复用既有 `/api/messages/search`，按 `media_kind` 扩展筛选维度。
- **Allowed paths** `src/messages/global_search/**`、`packages/web/src/features/search/**`
- **Acceptance** 七个分页各自独立分页；授权在读时重新判定（不能因为搜索索引里有就返回）。
- **Migration** `20270201000003`

## TG-505 隐私设置矩阵 · L · 组 C

- **Outcome** Telegram 的隐私分级与例外名单。
- **Work** `user_privacy_rules` + `user_privacy_exceptions` 表；维度：最后上线、头像、转发署名、邀请入群、手机号（若有）、语音消息；每个维度三档（所有人/联系人/无人）+ 允许例外 + 禁止例外；**last seen 的模糊化**（Telegram 对无权者显示「最近上线」「本周内上线」等区间）；所有读路径接入判定。
- **Allowed paths** `src/accounts/privacy.rs`、`migrations*/**`、`packages/web/src/features/settings/privacy/**`
- **Acceptance** 每个维度每一档都有授权测试；模糊化区间不可被多次请求反推出精确时间（这是真实的泄露路径，需专门测试）。
- **Migration** `20270201000004`

## TG-506 两步验证云密码 · M · 组 C

- **Outcome** 登录的第二重密码，带提示与恢复邮箱。
- **Work** `two_factor_credentials` 表；设置/修改/关闭流程；提示语；恢复邮箱验证；登录流程的二阶段；与既有 `device_sessions` 的交互（开启 2FA 是否终止其他会话 —— 决策写进 devlog）。
- **Allowed paths** `src/accounts/two_factor.rs`、`migrations*/**`、`packages/web/src/features/settings/security/**`
- **Acceptance** 密码用 `argon2`（与既有口令一致）；恢复流程不能绕过第二重验证；限流接入既有 `src/accounts/auth_limits.rs`。
- **Migration** `20270201000005`

## TG-507 主题与聊天背景 · L · 组 D

- **Outcome** 强调色切换、per-chat 壁纸、自定义主题、明暗自动切换。
- **Work** 强调色选择（至少 8 色）；`chat_wallpapers` 表与壁纸上传/内置壁纸/纯色渐变；壁纸的模糊与暗化参数；自定义主题的导入导出；跟随系统与按时间自动切换明暗。
- **Allowed paths** `src/accounts/appearance.rs`、`migrations*/**`、`packages/ui/src/tokens/**`、`packages/web/src/features/settings/appearance/**`
- **Acceptance** 主题切换无组件改动（验证 token 分层有效）；壁纸不影响消息列表的滚动性能（基准测试）。
- **Migration** `20270201000006`

## TG-508 通知例外与自定义声音 · M · 组 D

- **Outcome** per-chat 通知覆盖与声音。
- **Work** `notification_exceptions` 表；per-chat 的静音时长（1小时/8小时/2天/永久）、声音、预览开关、提及例外；全局默认与例外的优先级；接既有 `src/push_notifications/`。
- **Allowed paths** `src/notifications/**`、`src/push_notifications/**`、`migrations*/**`、`packages/web/src/features/settings/notifications/**`
- **Acceptance** 例外优先于全局的判定有测试矩阵；静音到期自动恢复。
- **Migration** `20270201000007`

## TG-509 数据与存储 · M · 组 A

- **Outcome** 缓存管理与自动下载策略。
- **Work** 按网络类型（wifi / 蜂窝 / 漫游）与媒体类型的自动下载规则；缓存占用统计与按聊天清理；最大缓存与保留期；与 Service Worker 缓存的协调。
- **Allowed paths** `packages/web/src/features/settings/storage/**`、`packages/core/src/stores/settingsStore.ts`
- **Acceptance** 缓存统计与实际占用一致；清理后不破坏正在查看的媒体。

## TG-510 多语言 · M · 组 B

- **Outcome** 界面文案外置，至少中英双语。
- **Work** i18n 方案选型（轻量优先）；抽取所有硬编码文案；复数与时间相对格式的本地化（Telegram 的「上次上线」文案规则各语言不同）；语言切换不刷新页面。
- **Allowed paths** `packages/web/src/**`、`packages/core/src/**`、`packages/ui/src/**`
- **Acceptance** CI 检查无硬编码中文字符串残留在组件里；复数规则有测试。

## TG-511 多头像与二维码名片 · S · 组 C

- **Outcome** 头像历史可翻阅；二维码名片。
- **Work** `user_avatar_files` 已存在，扩展为保留历史与排序；个人资料页的头像轮播；二维码名片生成（含主题色）与扫码添加。
- **Allowed paths** `src/accounts/avatars.rs`、`packages/web/src/features/profile/**`
- **Acceptance** 删除头像后历史顺序正确；二维码在深浅背景下均可扫。
- **Migration** `20270201000008`

---

# M6 收口 — 串行

## TG-601 PWA 强化 · M

- **Outcome** 安装体验、离线、推送与 Telegram Web 对齐。
- **Work** 复用既有 `pwa.ts` / `pwaBuild.ts`；离线时的可用范围明确定义（哪些聊天可读、能否草稿）；推送点击的深链跳转；安装引导；更新提示。
- **Acceptance** 离线可读最近会话；推送点击直达对应消息。

## TG-602 删除旧客户端与 alias · M

- **Outcome** 仓库里只有一个 web 客户端，词汇统一。
- **Work** 删除 `web/`；删除 `/api/rooms/*` alias；更新 ratatui CLI 到 `/api/chats/*`；`docs/client-capability-matrix.md` 重写；`README.md` 架构图更新。
- **Acceptance** `grep -rn "/api/rooms" src/ packages/` 无结果；ratatui CLI 全功能可用。

## TG-603 PySide6 桌面端处置 · S

- **Outcome** 桌面端有明确归属。
- **Work** 三选一并记录决策：迁 Tauri 复用 React 客户端 / 保留 PySide6 并接新 API / 删除。**这是产品决策，需用户确认，不要由 agent 自行决定。**

## TG-604 性能压测 · L

- **Outcome** 规模下的行为有数据支撑。
- **Work** 复用 `src/bin/stress.rs` 与 `docs/stress-testing.md`；场景：10 万条消息的聊天、500 个会话的列表、20 万成员的超级群、1000 人同时在线的频道浏览量广播、同屏 20 个动态贴纸。每个场景有阈值与实测值。
- **Acceptance** 全部场景达标或有明确的已知限制记录。

## TG-605 `packages/core` 抽离验证 · M

- **Outcome** 证明移动端的门确实开着。
- **Work** 在 `packages/core` 之外写一个最小的非 DOM 宿主（Node 脚本即可），注入 storage / websocket / clock，完成登录 → 拉会话 → 发消息。这是对 §2 两条硬约束的真实验证，不是文档承诺。
- **Acceptance** 该脚本在 Node 环境下跑通，不引入任何 DOM polyfill。

## TG-606 无障碍与键盘操作审计 · M

- **Outcome** 全键盘可用，屏幕阅读器可用。
- **Work** 焦点顺序与焦点陷阱；消息列表的键盘导航；快捷键体系（与 Telegram Desktop 对齐）；ARIA 标注；对比度检查。
- **Acceptance** 不用鼠标完成 登录 → 切会话 → 发消息 → 回复 → 打开设置。对比度见下方裁决，**不是无条件的 WCAG AA**。
- **对比度裁决（集成负责人，2026-09-30）** TG-009 在落 token 时报告了一个真实冲突：Telegram 自己的若干原色不达 WCAG AA。实测 白字压 `#3390ec` 为 3.31:1，cyan 强调色为 2.11:1，均低于正文所需的 4.5:1。

  「完全复刻 Telegram」与「无条件 WCAG AA」不能同时成立。裁决如下，**TG-606 不得为了达标而改动这些值**：

  | 用途 | 要求 |
  | --- | --- |
  | 正文与任何承载信息的文字 | 必须 ≥ 4.5:1。TG-009 已按 Telegram 自己的答案处理：`--tg-text-link` 用 `#00488f`（9.04:1）而非亮蓝 |
  | 强调色填充上的文字（选中的会话行、主按钮、发出气泡） | **保留 Telegram 原值**，允许低于 4.5:1。这是产品的视觉身份，改了就不是复刻了 |
  | 纯装饰、状态点、图标描边 | 保留原值，但必须有非颜色的第二信号（形状、图标、文字） |

  TG-606 的交付物里应包含一份**明确的偏离清单**：哪些 token 在哪些用途下低于 AA、当前的实测比值、以及为什么保留。目标是让偏离成为记录在案的决定，而不是无人知晓的缺陷。

  重新评估的触发条件：若将来需要满足某项合规要求，届时的选择是加一个「高对比」主题覆盖层（token 分层已经支持），而不是改动默认主题。

---

# 不在范围内（明确记录，避免反复讨论）

| 项 | 原因 |
| --- | --- |
| 音视频通话、群语音房、屏幕共享 | 需要独立 SFU 媒体服务，属于另一个工程 |
| 端到端加密密聊 | 需要客户端密钥存储与协议层改造；`@noble/hashes` 保留但不实现 |
| Bot API、内联查询、Mini App | 需要新增账号类型与沙箱容器 |
| 手机号登录与短信验证 | 现有账号模型是用户名 + 口令，改造牵涉注册邀请、2FA、隐私矩阵 |
| Stars、付费内容、礼物、Premium | 商业化体系 |
| 附近的人 | 位置隐私风险高，收益低 |
| 多账号切换 | 可作为 M5 后的候选，当前不排期 |

---

# M7 客户端补齐 — 旧 Vue 客户端删除后 React 客户端缺失的能力（2026-10-01 审计）

审计方法：对照已删除的 Vue 客户端（`f6b3d42:web/src`）的页面与组件、以及服务端全部 `/api/*` 路由，
找出服务端已有、`packages/core` 甚至已有客户端函数、但 React 界面从未调用的能力。AI 功能按用户决定继续关闭。

## TG-701 聊天资料与生命周期 · M
- **Work** 编辑群/频道标题、简介、头像 emoji（`PATCH /api/chats/:id`）；删除聊天（所有者）；退出聊天；邀请成员（`/invitations`）；移出成员（`members/:user_id` action）。
- **Acceptance** 每个操作在界面可达、权限不足时不显示；服务端拒绝时有错误提示；web 测试覆盖。

## TG-702 联系人 · M
- **Work** 联系人页：好友列表、好友申请收件箱（接受/拒绝）、按用户名添加、删除好友、备注、黑名单（拉黑/解除）、从联系人发起私聊；好友申请推送深链指向联系人页。
- **Acceptance** 不用旧客户端即可完成好友全流程；拉黑后对方无法发起私聊（服务端既有规则）。

## TG-703 通知中心 · S
- **Work** 侧栏铃铛 + 未读数（`/api/notifications/unread-count`，账号 WebSocket 信号刷新）；列表、单条已读、全部已读、点击跳转到来源消息。
- **Acceptance** 新提及/回复/好友申请出现在列表并可跳转。

## TG-704 账号安全补齐 · S
- **Work** 修改登录密码（`/api/users/me/password`），成功后按服务端规则处理其他会话。
- **Acceptance** 旧密码错误、新密码不合规有明确提示。

## TG-705 管理后台 · M
- **Work** 系统管理员可见的 `/admin`：概览、系统锁、聊天锁、注册邀请码、系统管理员、维护清理。AI 治理与模型面板保持关闭。
- **Acceptance** 非管理员不可见且服务端拒绝；每个面板有测试。

## TG-706 聊天任务与审计日志 · S
- **Work** 聊天信息面板中的任务列表（增删改、完成）与聊天审计日志（有权限者可见）。
- **Acceptance** 与服务端既有权限一致。


# M8 缺陷收口与 Telegram 对标（2026-10-01，用户反馈「联系人功能好像还有 bug」）

来源：用户反馈 + 各 devlog/看板里遗留的「集成待办」「后端缺口」。AI 继续关闭。

## TG-801 联系人修复与 Telegram 化 · M
- **Work** 用两个真实账号在真实浏览器里走完联系人全流程（搜索→申请→对方实时看到→接受→发消息→备注→删除→拉黑→解除），复现并修复所有缺陷，每个缺陷配回归测试。补齐：主菜单「联系人」入口的待处理申请徽章；账号 WebSocket 好友事件到达时联系人页与徽章实时刷新（不用手动刷新）；联系人列表按 Telegram 样式显示在线状态/最后上线、列表内过滤搜索、点行打开资料。
- **Acceptance** 两账号 E2E 全流程通过且有截图；每个修复的缺陷有失败→通过的测试。

## TG-802 会话列表媒体摘要与转发隐私 · S
- **Work** 最后一条消息摘要携带 `media_kind`，会话列表正确显示 语音/圆形视频/贴纸/GIF/投票/位置/名片/相册 的图标与文案（不再按扩展名猜）；转发语音/圆形视频进单聊时校验接收方的语音消息隐私设置。
- **Acceptance** 每种媒体的会话行文案有测试；隐私拒绝时服务端返回明确错误、客户端有提示。

## TG-803 信息面板共享内容分类与成员分页 · M
- **Work** `/files`（或新端点）按 媒体/文件/语音/GIF/链接 过滤；链接索引（迁移前缀 `202703010000xx`，SQLite+PG 成对，含历史回填策略）；成员列表游标分页；`WorkspaceShell` 信息面板遗留项（常挂、头部按钮切换、清理旧面板样式）。
- **Acceptance** 信息面板六个分类各自分页且只返回有权限的消息；迁移双库 fresh/upgrade 测试通过。

## TG-804 Telegram 对标走查 · M（只产出报告，不改产品代码）
- **Work** 真实浏览器逐屏对照 Telegram Web（A 版）走查：会话列表、聊天、输入框、右键菜单、信息面板、设置、联系人、搜索、移动端宽度。产出 `docs/tg/m8-parity-audit.md`：按严重度排序的差距与缺陷清单，每条带截图路径、复现步骤、建议归属文件。
- **Acceptance** 清单可直接拆成下一波任务卡。

## TG-805 动画对标 Telegram · M
- **Work** 对照 Telegram Web A 审计并改进动效：切换聊天、消息出现、发送键 mic↔send 形变、右键菜单/下拉从原点缩放、对话框与右侧面板滑入、会话行按压反馈、未读徽章弹出、新消息时会话行重排、输入中圆点、反应弹出、回到底部按钮、骨架屏。时长 150–250ms，只动 transform/opacity，每个动画有 `prefers-reduced-motion` 回退。
- **Acceptance** 只改样式/动画代码，不改行为；首屏包预算不增长。

## TG-806 首屏包回到预算 · S
- **Work** `main` 首屏 gzip 326 KB 超出 300 KB 预算（TG-801 前已超）。用 `import()` 懒加载非首屏功能（非当前语言的文案目录、只在交互后才需要的面板等），不抬预算。
- **Acceptance** `scripts/check_web_bundle.py` 通过；懒加载的功能在真实浏览器里仍可用。


# M9 走查收口（2026-10-01，来源 `docs/tg/m8-parity-audit.md`）

用户指示「剩下的也要一直一起串行完成」：负责人在主线程逐卡串行完成。迁移前缀 `202704010000xx`（如需）。

## TG-901 消息右键菜单与置顶条 · M
- **Work** 右键（桌面）/长按（触屏）在指针处弹出「反应条 + 操作菜单」，与 ⋮ 菜单同源；聊天头部下方的置顶消息条：显示最新置顶的摘要，点击跳到该消息并循环到上一条，有权限者可取消置顶，置顶/取消实时更新。
- **Acceptance** 浏览器实测右键菜单与置顶条（含跳转、取消）；单元测试覆盖置顶条模型。

## TG-902 面板与手机几何 · S
- **Work** 设置面板宽度与内边距对齐侧栏列；390 px 下聊天浮层不越界、粘性日期不压正文；会话列表右下角「新建」浮动按钮（新建私聊/群/频道）。
- **Acceptance** 1440 px 与 390 px 截图前后对比；新建按钮可用。

## TG-903 联系人 Telegram 化 · M
- **Work** 联系人列表按 Telegram 样式：在线/最后上线（尊重隐私）、按最后上线排序、列表内过滤、点行进资料页，操作（发消息/备注/删除/拉黑）移入资料页或行菜单。
- **Acceptance** 两账号浏览器实测；排序与过滤有单元测试。

## TG-904 频道帖与搜索默认页 · M
- **Work** 频道帖对所有人左对齐、显示频道身份与浏览数、无已读对勾；全局搜索默认页同时列出匹配的会话与消息。
- **Acceptance** 截图对比；渲染模型与搜索合并有测试。

## TG-905 打磨批 · S
- **Work** 英文复数；管理员探测不再产生 403；投票气泡间距；图片解码失败占位；信息面板通知行样式；信息面板「音乐」分类（`/files?kind=music`）。
- **Acceptance** 每项有测试或截图；`kind=music` 双库测试。

## TG-906 core 卫生 · S
- **Work** `packages/core` 拆分 src/test 两个 tsconfig（src 不再获得 bun/WHATWG 类型）；`chatSocket.createSocket` 同步抛错时不卡在 connecting；补拉过滤不再用 RFC3339 字典序比较时间。
- **Acceptance** 各自有失败→通过的测试；core 边界检查仍通过。

## TG-907 TUI 对接新功能 · M
- **Work** 用户 2026-10-01 指示「tui 的也要对接好功能」。盘点 `src/bin/client_tui` 与 Web 的功能差距（会话列表媒体摘要、置顶、反应、回复/引用、转发、编辑/撤回、投票、联系人/好友申请、未读、搜索等），按 TUI 形态补齐可用的子集；未知帧不崩溃。
- **Acceptance** TUI 单元/集成测试覆盖新增命令与帧处理；README 或 `--help` 列出命令。

# M10 第二轮走查收口（来源 `docs/tg/m10-parity-audit.md`）

## TG-1001 第二轮对标走查 · S（报告）

## TG-1002 设置与主菜单打磨 · M
- **Work** 设置主列表各项直达页面（去掉「我的账号 › 我的账号」式中间层，子项各有图标）；「聊天文件夹」页按设置卡片样式重做；大小上限等选择改为 Telegram 式行内控件；主菜单项加图标、「夜间模式」为开关行。
- **Acceptance** 1440/390 截图对比；设置导航有测试。

## TG-1003 首屏余量与复数排查 · S
- **Work** 首屏余量仅约 5 KB：把只在交互后才需要的语音/圆形视频录制逻辑移出首屏；排查其余 `t(key, 格式化数字)` 复数调用。
- **Acceptance** 首屏余量 ≥ 15 KB；录音按钮在真实浏览器中仍可用；复数调用有测试。

# M11 补齐未覆盖项（2026-10-01，用户：「把剩下没覆盖的也串行做完」）

## TG-1101 论坛话题与两步验证走查并修复 · M
- **Work** 真实浏览器走查论坛（开启话题、建话题、话题内发消息、关闭/置顶话题、General）与两步验证（开启、登录时输入密码、修改、关闭、找回入口），修复发现的缺陷。
- **Acceptance** 两条流程浏览器 E2E 全通过；缺陷各有回归测试。

## TG-1102 联系人在线状态实时 · S
- **Work** 联系人列表的在线/最后上线不再等 60 秒轮询：好友上线/下线经账号 WebSocket 推送（遵守 last_seen 互惠规则），页面即时更新。
- **Acceptance** 两账号浏览器实测：一方上线/下线，另一方联系人页 3 秒内变化；隐私拒绝时不推送。

## TG-1103 TUI 投票、文件夹与媒体显示 · M
- **Work** TUI 显示投票并可投票/查看结果；按聊天文件夹筛选会话列表；语音、圆形视频、贴纸、GIF、位置、名片在消息里有可读的文字表示（终端无法录音，录音不做）。
- **Acceptance** 单元 + 渲染测试；对真实服务器的端到端测试覆盖投票与文件夹。

## TG-1104 首屏余量 ≥ 15 KB · S
- **Work** 继续把首屏不需要的代码移出（如登录页之于已登录用户、频道/投票创建等交互后才用的部分），不抬预算。
- **Acceptance** `check_web_bundle.py` 首屏 ≤ 285 000 B gzip；涉及的功能浏览器实测可用。
