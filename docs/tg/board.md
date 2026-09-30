# 任务看板

**全局状态的唯一真相源。只在 `main` 分支上更新，由集成负责人在合并时维护。不要在 worktree 里改这个文件。**

任务详情见 `docs/tg/roadmap.md`。每个进行中任务的细节见 `docs/devlog/<TASK-ID>.md`。

- 盘点提交：`a16f422`
- **绿色基线提交：`fb818ee`** —— TG-000 合并后 `main` 首次全绿，pre-push 钩子从拒绝转为通过（闭环验证）
- 归档 tag：`archive/master-2026-09-30`（见 D-008）
- 盘点日期：2026-09-30
- 当前里程碑：**M0**

### ⚠ 已知验证缺口：PostgreSQL 测试静默跳过

`TEST_POSTGRES_ADMIN_URL` 未设置时，所有 PostgreSQL 测试**静默跳过**而非失败。TG-000 报告的「257 个测试通过」因此**完全没有执行过 PostgreSQL 路径**。

`tests/migration_upgrade_test.rs:157` 的兜底 URL 是 `postgres:postgres@localhost:52735`，而本地容器的凭据是 `chatroom:chatroom` —— 兜底永远连不上，所以永远跳过。设了环境变量则会 panic 而非跳过，这是想要的行为。

本地正确的连接串（`docker-compose.local.yaml` 映射的随机端口）：

```sh
export TEST_POSTGRES_ADMIN_URL="postgresql://chatroom:chatroom@127.0.0.1:52735/postgres"
```

**任何涉及迁移的任务必须导出它**，否则 PostgreSQL 迁移在全绿的门禁下完全未经验证。CI 里有 postgres service 所以 CI 路径是覆盖的；缺口只在本地。修正那个错误的兜底 URL 应该单独立卡。

## 当前在飞的任务

三个 worktree 并行，路径不重叠。

| 任务 | worktree | 分支 | 说明 |
| --- | --- | --- | --- |
| TG-003 | `.claude/worktrees/tg-003` | `agent/tg-003-embed-react-bundle` | 改 `build.rs` + `Dockerfile`；已收到 `archive/master` 的 feature 方案与 `--all-features` landmine |
| TG-004+005+006 | `.claude/worktrees/tg-004` | `agent/tg-004-chat-model-rename` | Chat 重命名垂直切片，M0 风险最高项 |
| TG-010 | `.claude/worktrees/tg-010` | `agent/tg-010-ui-primitives` | 19 个基础组件，消费 TG-009 冻结的 197 个语义 token |

TG-003 与 TG-004 都用 cargo，会在共享构建目录上串行等锁。这是预期行为。TG-004 已获授权在需要时切到私有 target 目录。

### 已集成

| 任务 | 提交 | 集成负责人的独立验证 |
| --- | --- | --- |
| TG-002 monorepo 骨架 | `296612a` | rebase 到 main 后：`bun install` 无变更，三包 typecheck 全 0，测试 22/0；**自己种入一个含 `react` + `document` + `localStorage` + `navigator` + `window` 的文件，边界检查确实失败并列出全部 5 处违规带行号与证据；删除后恢复 19/0**。没有采信 agent 的自我报告。 |
| TG-001 构建目录 | `4499a7e` | 见下方实测记录 |
| TG-000 绿色基线 | `fb818ee` | rebase 到 main 后重跑：`check_file_sizes.py` 通过（33 个基线告警、无增长）、迁移 parity 49 对、`cargo fmt` 干净、`clippy --all-targets --all-features` 零告警、`cargo test --all-targets --all-features` **exit 0**。另外确认 `src/lib.rs` / `src/models.rs` / `src/routes.rs` / `Cargo.toml` 与 main 零差异，即公开接口未变。**合并后 pre-push 钩子从拒绝转为通过** —— 这是「基线变绿」最直接的闭环证据。 |
| CI 加固 + D-008 | `8b9d178` | 钩子在红树上拒绝、在绿树上通过，两个方向都实测过 |
| TG-009 Design tokens | `785953a` | rebase 到 main 后独立验证：**`day.css` 与 `night.css` 各声明 70 个 token 且集合完全一致**，6 个强调色文件集合亦完全一致 —— 主题切换不可能留下未定义变量（这是 agent 没提、但最容易出问题的不变量）。`preview.html` 对 `--tg-raw-` 原语的引用数为 **0**，且十六进制、`rgb()`/`hsl()`、命名颜色字面量各为 **0** —— 它确实只靠语义 token 上色，所以是证明而非效果图。原语仅被 token 层自身引用。每个值带 `[web]`/`[desktop]`/`[ios]`/`[derived]`/`[ours]` 出处标注，真实值与猜测值可区分。文件大小最大 253 行。 |

## 顺序约束

**TG-004 合并进 `main` 之前不要再创建碰 `src/**` 或 `migrations*/**` 的 worktree。** 它重命名十余张表并改动约 30 个模块，任何并行的 Rust 分支都会被撕碎。不碰 Rust 的任务（TG-009、TG-010、TG-013）不受影响。

剩余顺序：`TG-004+005+006`（进行中）→ `TG-007` → `TG-008`；`TG-009` → `TG-010`；`TG-006`+`TG-007` → `TG-011`；三者齐 → `TG-012`。

**TG-004 / TG-005 / TG-006 是一个垂直切片，由同一个 worktree 连续完成。** 迁移重命名了表名而 Rust 代码仍在查旧表名 —— 单独合并 TG-004 会让树无法编译。这是出卡时的疏漏，已在此更正。

## M0 地基

| 任务 | 规模 | 状态 | Owner | Worktree | 依赖 |
| --- | --- | --- | --- | --- | --- |
| TG-000 修复红色基线 | S | **merged** `fb818ee` | agent:TG-000 | — | — |
| TG-001 回收构建目录并验证共享配置 | S | **merged** `4499a7e` | 集成负责人 | — | — |
| TG-002 monorepo 骨架 | M | **merged** `296612a` | agent:TG-002 | — | — |
| TG-003 `build.rs` 切换嵌入目标 | S | **in-progress** | agent:TG-003 | `tg-003` | TG-002 ✓ |
| TG-004 Chat 数据模型迁移 | L | **in-progress** | agent:TG-004 | `tg-004` | TG-000 ✓ |
| TG-005 Rust 模块与类型重命名 | L | **in-progress**（同一 worktree） | agent:TG-004 | `tg-004` | TG-004 |
| TG-006 API 路径重命名与 alias | M | **in-progress**（同一 worktree） | agent:TG-004 | `tg-004` | TG-005 |
| TG-007 WebSocket 帧扩展 | M | not-started | — | — | TG-005 |
| TG-008 云端草稿 | M | not-started | — | — | TG-006, TG-007 |
| TG-009 Design tokens 提取 | M | **merged** `785953a` | agent:TG-009 | — | — |
| TG-013 修正 PostgreSQL 测试静默跳过 | S | not-started | — | — | — |
| TG-010 `packages/ui` 基础组件 | L | **in-progress** | agent:TG-010 | `tg-010` | TG-002 ✓, TG-009 ✓ |
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
| `docker build` 验证 | **未执行** —— 留给 TG-003（它要改 `Dockerfile` 的 COPY 列表，一起验证更省一次完整镜像构建） |

结论：194GB 里 99% 以上是沉积。一次 `clippy --all-targets` 的真实足迹是 1.5GB。

## 协议失效记录

发现 `docs/tg/agent-protocol.md` §8 列出的任一失效信号，记在这里。空着是好事，但只有真的看过才能空着。

- **2026-09-30** 盘点发现 `a16f422` 的 `scripts/check_file_sizes.py` 有 5 个错误，即基线本身是红的。旧路线图的 `FND-001 修复当前 CI 阻断` 是同一类问题，说明这道门禁会反复变红而没人盯。TG-000 负责修复，并确认 CI 是否真的在 PR 上执行这个脚本。
- **2026-09-30** TG-002 的 agent 报告 `scripts/tg-worktree.sh check` 从 worktree 内部无法运行：`REPO_ROOT` 用了 `git rev-parse --show-toplevel`，在 worktree 里返回 worktree 自己的根，于是去找 `<worktree>/.claude/worktrees/<task>`。**这是脚本的真实缺陷，协议 §2.4 对每个 agent 都会失效。** 已改用 `--git-common-dir` 解析主仓库，并加了「worktree 里的旧副本自动委托给主仓库当前版本」的自愈。自愈只对此修复之后创建的 worktree 生效，基线为 `5ee18f4` 或更早的 worktree 需按 §2.4 用绝对路径调用。该 agent 手工复现了全部检查项，没有静默跳过 —— 这是正确的处理方式。
- **2026-09-30** TG-002 的 agent 提交了 `bun.lock`，虽然它不在该任务的 allowed paths 里。理由成立（创建 bun workspace 必然产生它，CI 的 `--frozen-lockfile` 没有它无法工作），且已在 devlog 中显式标注而非静默提交。集成负责人追认。**这说明 allowed paths 应该预见到锁文件** —— 后续涉及包管理的任务卡要把锁文件写进 allowed paths。

## 待用户决策

| 决策 | 涉及任务 | 状态 |
| --- | --- | --- |
| 地图供应商选型 | TG-407 | 未提出 |
| `favorites` 与 Saved Messages 的关系 | TG-503 | 未提出 |
| 开启 2FA 是否终止其他设备会话 | TG-506 | 未提出 |
| PySide6 桌面端去留 | TG-603 | 未提出 |
