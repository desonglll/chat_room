# Server configuration

The server reads `chat-room.toml` from its working directory by default. Use
`--config PATH` to load another file. If the selected file does not exist, the
built-in defaults are used.

```toml
[uploads]
max_file_size_mib = 512
chunk_size_mib = 8
abandoned_upload_gc_hours = 24

[attachments]
directory = "chat_attachments"

[attachments.oss]
enabled = false
endpoint = ""
bucket = ""
access_key_id = ""
access_key_secret = ""
root = "/"

[database]
kind = "sqlite"
sqlite_path = "chat_rooms.db"
postgres_url = ""
max_connections = 10

[redis]
enabled = false
url = "redis://127.0.0.1:6379/"
key_prefix = "chat-room"
connect_timeout_ms = 1500
command_timeout_ms = 500

[admin]
usernames = []
orphan_retention_hours = 168
deleted_room_retention_days = 30
```

`max_file_size_mib` is the maximum size of one file, measured in MiB. It must
be greater than zero. Restart the server after changing it. The browser client
reads the effective value from `GET /api/config`, so its validation and error
messages stay aligned with the server.

The browser uses a durable chunked session for every attachment. It first
reads the file in bounded chunks to compute SHA-256, then reports upload
progress only from byte offsets confirmed by the server. An interrupted task
is listed again after a refresh; the browser security model requires the user
to reselect the original file, after which upload continues from the confirmed
offset. If the same account has already uploaded healthy content with the same
SHA-256 and size, the server creates a new logical attachment reference without
receiving the file bytes again. The server still verifies newly received bytes
against the declared hash before publishing them.

`attachments.directory` is always used for in-progress staging. When OSS is
disabled it also stores final attachment bytes. Relative paths are resolved
from the server working directory. Final object keys are SHA-256 hashes, so
identical uploads share physical bytes while retaining separate metadata and
permissions. SQLite/PostgreSQL stores metadata only.

Enable `[attachments.oss]` to use Aliyun OSS for final objects. `endpoint`,
`bucket`, `access_key_id`, and `access_key_secret` are required when enabled;
chunked uploads still stage locally. Restart the server after changing any
storage setting.

`database.kind` accepts `sqlite` or `postgres`. The CLI `--database-type` and
`--database` options override the selected backend and connection path/URL.

Redis is an optional cache for bearer-session lookups; PostgreSQL/SQLite
remains the source of truth. Set `redis.enabled = true`, or set
`CHAT_ROOM_REDIS_URL`, to enable it. Startup falls back to direct database
reads if Redis is temporarily unavailable. Profile, password, avatar, logout,
and account-deletion changes invalidate affected cached sessions.

## Complete PostgreSQL backup and restore

Stop the chat server while taking a maintenance backup so the PostgreSQL
snapshot and attachment directory describe the same point in time. PostgreSQL
client tools (`pg_dump` and `pg_restore`) must be installed and match the
server's major version.

```sh
cargo run -- export --output backups/chat-room-2026-08-24
cargo run -- restore --input backups/chat-room-2026-08-24
```

Both commands use `chat-room.toml` and accept the global `--config`,
`--database-type postgres`, and `--database` overrides. An export creates a new
directory containing `database.dump`, the complete local `attachments`
directory, and `manifest.json` with a SHA-256 and byte count for every file.
The output path must not already exist.

Restore verifies the complete manifest before changing data, then runs
`pg_restore --clean --if-exists --no-owner --no-privileges --exit-on-error`.
It stages attachment bytes before restoring PostgreSQL. The current attachment
directory is renamed to a unique `.pre-restore-*` sibling before restored files
are activated, so it remains available for manual rollback. When Redis is
enabled, restore requires Redis to be reachable and clears this application's
cache namespace before replacing the database. The commands intentionally reject SQLite and
OSS-backed attachment configurations rather than producing a partial backup.

The system dashboard is available at `/admin`. Access requires a normal logged
in account whose username appears in `admin.usernames` (case-insensitive).
An environment variable can override the list at startup without changing the
file:

```sh
CHAT_ROOM_ADMIN_USERNAMES=ops-admin,on-call cargo run --bin server
```

The dashboard reports request latency, connections, rooms, messages, sessions,
and logical/physical attachment usage. Its maintenance action permanently
removes orphaned attachment objects and soft-deleted rooms only after their
configured retention windows. The action rechecks shared content references
before deleting bytes.

When upgrading a database that still has `attachments.data`, startup exports
every BLOB to this directory before the migration removes the column. Startup
stops without dropping the column if any export fails. Later startup backfills
SHA-256 metadata for legacy UUID-keyed objects without moving them. Back up the
database and the selected attachment backend together.
