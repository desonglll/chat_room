# 多 worktree 协作与交接协议

本文件的唯一目的：**让任何一个新 agent 在读完一个文件后就能接手一个进行中的任务，不需要读 diff 猜意图。**

补充 `AGENTS.md`，不替代它。冲突时以本文件为准（本文件更具体）。

---

## 1. 共享构建目录

配置在 `.cargo/config.toml`，指向 `/Volumes/Tuo-APFS/workspace/.cargo-target/chat_room`。

**为什么共享** —— `target/` 曾长到 194 GB，卷上只有 456 GB 可用。每个 worktree 独立 target 装不下三个。

**你要接受的代价** —— Cargo 对构建目录加排他锁。两个 worktree 同时 `cargo build` / `check` / `clippy` / `test` 会串行，后一个显示 `Blocking waiting for file lock on build directory`。这不是故障，不要去杀进程。

**什么时候可以例外** —— 你的任务需要长时间反复编译（例如 TG-004 的迁移测试、TG-201 的权限重构），而另一个 worktree 也在密集构建。此时在你的 shell 里：

```sh
export CARGO_TARGET_DIR=/Volumes/Tuo-APFS/workspace/.cargo-target/<task-id>
```

预留 20 GB，任务合并后删除它，并在 devlog 里记一句。

**CI 与 Docker** 通过 `CARGO_TARGET_DIR` 环境变量覆盖，永不读 `.cargo/config.toml`。改那个文件不会影响 CI。

**定期清理** —— `cargo sweep --time 14 /Volumes/Tuo-APFS/workspace/.cargo-target/chat_room`，每月一次。这个目录不会自己收缩。

---

## 2. 一个任务的完整生命周期

### 2.1 开工

```sh
./scripts/tg-worktree.sh new TG-101 virtual-message-list
```

脚本会创建 worktree、分支、以及从模板生成的 `docs/devlog/TG-101.md`。然后**在写任何代码之前**：

1. 读 `docs/tg/README.md`、本文件、`docs/tg/roadmap.md` 里你那张卡。
2. 填 devlog 的 `Owner`、`Allowed paths`、`Migration prefix`，以及 **Frozen interface** 一节。
3. 在 `main` 分支的 `docs/tg/board.md` 里认领任务（不是在你的 worktree 里改，避免合并冲突）。
4. `cd .claude/worktrees/tg-101`

第 2 步不是形式主义。先写下契约再写实现，是让别人能并行依赖你的前提。

### 2.2 进行中

- 只写你卡上 `Allowed paths` 列出的路径。需要动别人的路径 → 找集成负责人，不要自己解决。
- 每个非显然的选择在 devlog 的 **Decisions** 一节记一行，**必须带原因**。「改用 X」没有价值，「改用 X 而不是 Y，因为 Z」才有。
- devlog 和它描述的代码放进同一个 commit。

### 2.3 结束任何一次会话前（这是最重要的一条）

**重写** devlog 的 **Handoff snapshot** 一节。不是追加，是覆盖 —— 它永远描述现在。

其中 `Next concrete action` 必须指向一个文件和一个操作：

- ✗ 「继续开发消息列表」
- ✗ 「处理剩下的边界情况」
- ✓ 「`packages/web/src/features/messageList/useAnchor.ts` —— 实现 prepend 路径的 `restoreAnchor()`，append 路径已经工作；参考同文件 line 84 的 append 实现」

哪怕这次会话只做了十分钟，也要更新。一次遗漏就会让下一个 agent 从读 diff 开始，那正是本协议要消灭的成本。

### 2.4 交接检查

```sh
./scripts/tg-worktree.sh check TG-101
```

检查 devlog 完整性、迁移 parity、文件大小、`cargo fmt`、`cargo clippy`。**通不过就在 devlog 里记录失败，不要留下无人知晓的红灯。**

然后按 `AGENTS.md` 的要求跑完整门禁：

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
bun run -F '*' typecheck && bun run -F '*' test
python3 scripts/check_migration_parity.py
python3 scripts/check_file_sizes.py
```

### 2.5 接手别人的任务

```sh
./scripts/tg-worktree.sh status TG-101     # 只打印 Handoff snapshot
```

读到 `<fill in>` 说明上一位违反了协议。在 `board.md` 标记它，然后从 **Decisions** 和 **Progress log** 重建上下文 —— 但要在 board 上记一笔，这是流程失效的信号，不是你的问题。

---

## 3. 文件所有权

### 3.1 集成负责人独占

并行期间这些文件只有集成负责人能写：

`Cargo.toml` · `Cargo.lock` · `src/lib.rs` · `src/models.rs` · `src/routes.rs` · `src/state*.rs` · `build.rs` · 根 `package.json` · `bun.lock` · `packages/*/package.json` · `packages/web/src/app/**` · `.github/workflows/**` · `AGENTS.md` · `docs/tg/board.md` · `CONTEXT.md`

要改这些，在 devlog 里写一份**集成补丁清单**（具体到要加哪几行），由集成负责人在你的模块变绿之后应用。

### 3.2 为什么 store 一个域一个文件

`packages/core/src/stores/` 下每个域一个文件，是为了让多个 worktree 不抢同一个文件。需要跨 store 读数据时用选择器组合，不要把两个域塞进一个文件。

### 3.3 devlog 的所有权

**一个 worktree 只写自己那一个 devlog 文件。** 这是零冲突设计的关键 —— 每个 worktree 只 touch `docs/devlog/<自己的ID>.md`，合并时永不冲突。

`docs/tg/board.md` 是另一回事：它是全局状态，**只在 `main` 分支上更新**，由集成负责人在合并时维护。不要在 worktree 里改它。

---

## 4. 数据库迁移

- SQLite 与 PostgreSQL 迁移必须**同一任务内成对创建**，版本号与语义名完全一致。
- 用你卡上预留的前缀（见 `roadmap.md` 顶部的分配表），在其内递增。
- 永不编辑已应用的迁移，只加前向迁移。
- 迁移的 owner 同时拥有回滚推理与两个适配器的测试。
- 每个迁移任务必须有**全新建库**和**从基线升级**两条路径的测试。

M0 的 TG-004 会重命名十余张表。**在它合并进 `main` 之前不要创建任何其他 worktree** —— 任何并行分支的迁移都会与它冲突。

---

## 5. 前端纪律

- `packages/core` 不得 import `react` / `react-dom` / `@tg/ui` / `@tg/web`，不得引用 `window` / `document` / `localStorage` / `navigator`。CI 检查这一点。违反它，将来接移动端就要重做一遍。
- `packages/ui` 的组件不得认识 Chat / Message / User 等业务概念。
- 组件只消费 `semantic` 层的 CSS 变量，永不直接用 `primitive` 值。
- 动效参数放 token 层，不散落在组件里。每个动效都要有 `prefers-reduced-motion` 降级。
- 手写源文件 350 行告警、500 行阻断。按职责拆分，不要靠注释分段。

---

## 6. 授权纪律（继承 `AGENTS.md`，重申因为最容易出错）

- **读路径与写路径都要判定权限。** 搜索索引、通知投影、向量检索的结果都必须在返回前重新授权。
- 不要记录 token、口令、能力 URL、供应商密钥、私信正文、检索到的证据。
- `Chat` 是授权与知识隔离边界。不同 chat 的知识永不合并检索。
- 原始消息是唯一真相，索引 / 通知 / 摘要 / AI 输出都是投影。

---

## 7. 什么情况下必须停下来问人

不要自行决定这些，它们是产品决策：

- 地图供应商选型（TG-407）
- `favorites` 与 Saved Messages 的关系（TG-503）
- 开启 2FA 是否终止其他设备会话（TG-506）
- PySide6 桌面端的去留（TG-603）
- 任何需要新增第三方依赖的决定（`AGENTS.md`：新依赖需要 owner、当下的真实需求、集成负责人评审）
- 任何要改「已锁定的决策」表（`docs/tg/README.md`）的想法

TG-408 链接预览的 SSRF 防护**必须经集成负责人评审才能合并**。

---

## 8. 这套协议的失效信号

发现以下任一情况，在 `board.md` 记录，不要默默绕过：

- devlog 的 `Handoff snapshot` 含 `<fill in>` 或超过一天未更新但任务标记为 in-progress
- 两个 worktree 写了同一个文件
- 某个 devlog 的 `Next concrete action` 写的是「继续」类含糊表述
- 迁移前缀被两个任务使用
- 有代码提交但对应 devlog 没有同 commit 更新
