# Brief for every TG task agent (from the integration lead)

You own exactly one TG task. The user has authorised the whole remaining
Telegram-parity programme (M1–M6) to be built autonomously; they will only look
at the final result, so the bar is "actually works and is verified", not
"looks done".

## Read first (in this order, do not skip)

1. `docs/tg/README.md`, `docs/tg/agent-protocol.md`, `AGENTS.md`, `CONTEXT.md`
2. Your card in `docs/tg/roadmap.md`, plus `docs/tg/architecture.md` sections it touches
3. `docs/tg/decisions.md` — especially **D-009** (execution rules) and **D-010** (user decisions)

## Setup

```sh
cd /home/mike/workspace/chat_room
./scripts/tg-worktree.sh new TG-xxx <slug>      # creates .claude/worktrees/tg-xxx + devlog
cd .claude/worktrees/tg-xxx
bun install                                      # each worktree needs its own node_modules
export CARGO_TARGET_DIR=/home/mike/workspace/.cargo-target/tg-xxx   # PRIVATE per task (see hazard below); outside the repo
export CARGO_BUILD_JOBS=6 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0   # RAM + disk: debug info makes a test build ~40 GB
export TEST_POSTGRES_ADMIN_URL=postgresql://chatroom:chatroom@127.0.0.1:52735/postgres
export TEST_REDIS_URL=redis://127.0.0.1:6379/
export PATH=/tmp/pgx/usr/bin:$PATH LD_LIBRARY_PATH=/tmp/pgx/usr/lib   # pg_dump for backup tests
```

Put the exports in front of every cargo command (shell state does not persist
between your tool calls). Delete your private target dir when your task is done. The machine has
14 GB RAM: never run two cargo commands of your own concurrently.

Do all work inside your worktree. Never edit files in the main checkout.

## Rules that are specific to this phase (D-009)

- **AI features are disabled.** Do not build AI UI; do not depend on AI.
- **Do not edit `docs/tg/board.md`** and do not push. Commit on your branch
  only; the lead merges and pushes.
- You may make **mount-level** edits to shared hotspots (`src/lib.rs`,
  `src/routes.rs`, `packages/web/src/app/**`, `packages/*/package.json`,
  `src/models.rs` only if a wire type truly must live there) — declare a module,
  mount a router, register a route/page. List every such edit under an
  "Integration patch list" heading in your devlog.
- Stay inside your card's allowed paths otherwise. If you truly must touch
  another path, do the minimum and record it in the devlog with the reason.
- Approved deps already installed in `packages/web`: react-virtuoso, motion,
  lottie-web, pako, emoji-picker-element(+data), dompurify, marked, plyr,
  leaflet, qrcode (+types). Any other new dependency: avoid; if unavoidable,
  justify in the devlog.
- Migrations: SQLite + PostgreSQL pairs, your card's reserved version, tests for
  fresh-schema and upgrade paths on both adapters.
- Authorization at read time and write time. Never log secrets or message bodies.
- Files: 350-line warning, 500-line block. Split by responsibility.
- `packages/core` must stay platform-free (no react/DOM globals). `packages/ui`
  knows no business concepts. Components use semantic `--tg-*` tokens only.
- Every animation has a `prefers-reduced-motion` fallback.
- UI copy: the existing client is Chinese-first; follow the surrounding copy.

## Done means

All of these pass in your worktree, with exact counts recorded in the devlog
"Verification" section:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features            # only if you touched Rust/migrations/web embed; report binaries/pass/fail
bun run -F '@tg/*' lint && bun run -F '@tg/*' typecheck && bun run -F '@tg/*' test
(cd packages/web && bun run build) && python3 scripts/check_web_bundle.py
python3 scripts/check_migration_parity.py && python3 scripts/check_file_sizes.py
```

Frontend-only tasks may skip `cargo test`, but `cargo clippy` also builds the web
bundle through `build.rs`, so run it at least once at the end.

If the bundle budget in `scripts/check_web_bundle.py` is exceeded, prefer
lazy-loading (`import()`) the heavy feature; do not raise the budget yourself —
report the numbers and the lead decides.

If a headless Chromium is available (`ls ~/.cache/ms-playwright`; Playwright
is installed at `/tmp/pw/node_modules`), take screenshots of user-facing
changes against a running server (`cargo run --bin server -- -p <free port>`
with a sqlite DB under /tmp: `--database-type sqlite --database /tmp/<task>.db`)
and save them under `/tmp/tg-shots/<task>/`. Optional but valued.

Commit on your branch (`agent/tg-xxx-...`) with the devlog in the same commit.
Rewrite the devlog "Handoff snapshot" before you finish.

## Final message to the lead (keep it under 250 words)

- branch + head commit
- what now works (user-visible), and what was deliberately left out
- gate results with numbers
- Integration patch list (hotspot edits) and any path outside your card
- risks / anything the lead must decide

## Known hazard (found by TG-102)

With the shared `CARGO_TARGET_DIR`, the web bundle embedded by `build.rs` into a
`cargo run` server binary may come from ANOTHER worktree that built last. For UI
screenshots, serve your own worktree's `packages/web` via `bun run dev` (vite
proxies API/WS to a running server — check `packages/web/vite.config.ts`) and
treat the embedded bundle as untrusted. Use `cargo run` only for the API.

## Hazard 2 (found by TG-302): shared target dir mixes worktrees

Workspace-member artifacts from different worktrees collide in a shared target
dir, so your tests can link against another worktree's `chat_room` library.
Therefore every task uses its own `CARGO_TARGET_DIR=/home/mike/workspace/.cargo-target/<task-id>`
(D-007's shared dir is suspended on this machine; the lead records this).
