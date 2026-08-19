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

[admin]
usernames = []
orphan_retention_hours = 168
deleted_room_retention_days = 30
```

`max_file_size_mib` is the maximum size of one file, measured in MiB. It must
be greater than zero. Restart the server after changing it. The browser client
reads the effective value from `GET /api/config`, so its validation and error
messages stay aligned with the server.

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
