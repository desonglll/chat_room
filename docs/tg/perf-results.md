# Performance results (TG-604)

Host: shared 16-core Linux dev box (other builds running), SQLite file-backed unless noted,
debug builds for the test-suite scenarios and a release build for the end-to-end stress run.
Every scenario has an automated test with a ceiling that only a full scan or an N+1 regression
would break; measured values are from 2026-10-01.

| # | Scenario | Test | Threshold | Measured | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | Chat with **100 000 messages**: newest page, a page 1 000 pages back, jump to the oldest | `tests/perf_scale_test.rs` | each < 300 ms (median of 5) | newest **13 ms**; 1 000 pages back **59 ms**; jump to oldest (context window) **94 ms** | ✅ |
| 2 | **500 conversations** in the list | `tests/perf_scale_test.rs` | < 2 s (median of 5) | `/api/conversations` **183 ms**; `/api/chats` **70 ms** | ✅ |
| 3 | **200 000-member** supergroup roster, keyset pages of 100 | `tests/member_page_perf_test.rs` (TG-201) | page < 250 ms at any depth | SQLite first 4.4 ms / middle 3.8 ms / last 0.4 ms (OFFSET at the same depth: 60 ms); PostgreSQL 6.3 / 3.3 / 2.3 ms | ✅ |
| 4 | **1 000 viewers** of a channel at once — view-count broadcasts | `tests/channel_views_test.rs` (TG-202) | frames < viewers / 10 | 1 000 viewers × 2 posts in 287 ms → **1** `message_views_updated` frame | ✅ |
| 5 | **20 animated stickers** on screen | TG-301 browser bench (`packages/web/src/features/sticker/renderer/bench`, headless Chromium) | page ≥ 50 fps, p95 ≤ 25 ms | page **59.8 fps**, p95 16.7 ms, 0 long tasks, page CPU 0.30 (worker renderer); 60 distinct → 24 animate (cap) at 58.6 fps | ✅ (TG-301 devlog) |
| 6 | Mixed load, end to end (`stress`, release): 16 HTTP + 8 WebSocket + 2 upload workers, 20 s | `scripts/stress-test` | error rate ≤ 1 % | HTTP 3 692 ops/s p95 9.4 ms, 0 errors; WebSocket round trip p50 626 ms p95 699 ms, 0 errors; uploads 109 ops/s p95 27.6 ms, 0 errors | ✅ after two fixes |

## Defects found and fixed by this task

1. **Concurrent uploads failed with `database is locked` on SQLite** (410 of 2 016 in the first
   mixed run). The attachment transaction read (permission check) before writing, and a deferred
   read-then-write transaction cannot be upgraded once another writer has committed
   (`SQLITE_BUSY_SNAPSHOT`); `busy_timeout` does not help. The check now rides on the first
   `INSERT … WHERE EXISTS`, so the transaction takes the write lock up front.
2. **Live delivery could lose a text message under write contention.** The poller walks a
   `(created_at, id)` cursor, but `created_at` is taken before the insert waits for the lock, so a
   message could commit behind the cursor and never be sent live (8 of 8 WebSocket workers failed
   in the mixed run). Each poll now also lists a 10-second trailing window and delivers ids the
   connection has not sent (`src/realtime/late_commits.rs`, bounded memory,
   `tests/late_commit_delivery_test.rs`).

## Known limits

- Live delivery is poll-based (`realtime.poll_interval_ms`, default 250 ms): the round trip is
  ~230 ms on an idle server and ~620 ms under the mixed load above. Push-based fan-out
  (e.g. Postgres `LISTEN/NOTIFY` or Redis pub/sub) is the route to lower latency.
- SQLite serialises writers: ~110 uploads/s alongside chat traffic on this host. Deployments
  that expect heavier write load should use PostgreSQL.
- A late commit more than 10 s behind the cursor is still only visible after a reload (the
  window is a constant in `late_commits.rs`).
