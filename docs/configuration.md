# Server configuration

The server reads `chat-room.toml` from its working directory by default. Use
`--config PATH` to load another file. If the selected file does not exist, the
built-in defaults are used.

For local development, start only the infrastructure containers and run the
Rust server on the host:

```sh
docker compose -f docker-compose.local.yaml up -d
cargo run --bin server -- --web
```

The server loads `.env` before `chat-room.toml`. `CHAT_ROOM_AI_MODEL` and
`CHAT_ROOM_AI_BASE_URL` override the default model, while
`CHAT_ROOM_EMBEDDING_MODEL`, `CHAT_ROOM_EMBEDDING_BASE_URL`, and
`CHAT_ROOM_VECTOR_ENABLED` configure full-room RAG. See `.env.example` for the
complete local PostgreSQL, Redis, Qdrant, and AI variable set.

```toml
[uploads]
max_file_size_mib = 512
chunk_size_mib = 8
abandoned_upload_gc_hours = 24

[attachments]
directory = "chat_attachments"

[attachments.oss]
enabled = false
local_mirror_enabled = false
direct_upload_enabled = false
presign_expiry_secs = 900
operation_timeout_secs = 30
endpoint = ""
bucket = ""
access_key_id = ""
access_key_secret = ""
root = "/"
presign_endpoint = ""
presign_addressing_style = "virtual"

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
message_ttl_secs = 30

[work_queue]
message_concurrency = 32
upload_concurrency = 4
wait_timeout_secs = 30

[ai]
enabled = false
provider = "openai"
api_key_env = "CHAT_ROOM_AI_API_KEY"
model = ""
fast_model = ""
base_url = ""
standard_extra_body = { temperature = 0.2 }
reasoning_extra_body = { reasoning_effort = "high" }
max_context_messages = 30
analysis_context_messages = 120
request_timeout_secs = 60
stream_idle_timeout_secs = 30
stream_total_timeout_secs = 300
suggest_cooldown_secs = 10

[vector_store]
enabled = false
url = "http://127.0.0.1:6333"
collection = "chat_messages"
api_key_env = ""
dimensions = 1024
top_k = 6
score_threshold = 0.35
embedding_base_url = ""
embedding_model = ""
embedding_api_key_env = "CHAT_ROOM_AI_API_KEY"
worker_interval_ms = 500

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
chunked uploads still stage locally. Keep the credentials on the server. They
are never returned by the upload API.

Set `local_mirror_enabled = true` to publish every completed object to both
local disk and OSS. Upload completion first makes the local copy durable, then
attempts OSS for at most `operation_timeout_secs`. If OSS rejects or times out,
the message remains available from the local copy. Downloads prefer OSS but
fall back to local when the object is missing or the service is unavailable.
For browser-direct uploads, the server reads the new OSS object once to verify
its length and SHA-256 while creating the local mirror.

Set `direct_upload_enabled = true` to let the browser request a short-lived,
object-scoped PUT URL after it has calculated the file hash. `presign_expiry_secs`
controls that URL's lifetime. If the server uses an internal OSS endpoint, set
`presign_endpoint` to a browser-reachable public endpoint or bucket CNAME and
select `presign_addressing_style = "virtual"`, `"path"`, or `"cname"` as
appropriate. A failed direct PUT automatically falls back to the existing
resumable server upload.

The OSS Bucket must have a CORS rule for the web application's exact origin:
allow `PUT` and the `Content-Type` request header. Avoid `*` origins in
production. The application binds `Content-Type` into the signed request, so a
different header value is rejected by OSS. Restart the server after changing
any storage setting.

`database.kind` accepts `sqlite` or `postgres`. The CLI `--database-type` and
`--database` options override the selected backend and connection path/URL.

Redis is an optional read-through cache for bearer-session lookups and paged
message history; PostgreSQL/SQLite remains the source of truth. Set
`redis.enabled = true`, or set `CHAT_ROOM_REDIS_URL`, to enable it. Startup and
request handling fall back to direct database reads if Redis is unavailable.
The supplied Compose file binds Redis only to `127.0.0.1:6379`, so both the
containerized service and a local `cargo run` can use the same cache without
exposing Redis on the network.
Message writes, edits, recalls, reactions, forwards, and attachment messages
advance a per-room cache version, so stale pages stop being addressable without
an expensive key scan. `message_ttl_secs` bounds memory used by old versions.
Profile, password, avatar, logout, and account-deletion changes invalidate
affected cached sessions.

`work_queue` applies fair, bounded admission to durable message writes and file
I/O. Requests wait for a permit for at most `wait_timeout_secs`; HTTP uploads
then return `503` and resumable chunks include their last confirmed byte offset.
The browser can retry without duplicating messages or restarting files.

The queue deliberately does not put chat messages or file bodies in RabbitMQ.
Messages already enter the database with a unique client message id before the
WebSocket poller publishes them, and upload sessions persist every confirmed
offset while bytes remain in local staging. Adding a broker in front would
create a second source of truth and a database/broker dual-write failure mode;
large attachment payloads are also unsuitable broker messages. The database,
staging files, and resumable session rows provide crash recovery, while the
fair admission queue provides overload control.

`ai.api_key_env` is the name of the environment variable that contains the
provider key; it is not the key itself. The public configuration endpoint
reports `ready` only when AI is enabled and that environment variable is
present in the server process. `fast_model` is optional. When an AI session's
deep-thinking switch is off, the server uses `fast_model` when configured.
`provider = "openai"` accepts any OpenAI-compatible endpoint.
`standard_extra_body` and `reasoning_extra_body` are optional objects forwarded
exactly for each mode; no vendor fields are inferred from `base_url`.

Streaming has independent limits. `request_timeout_secs` bounds the initial
provider connection, `stream_idle_timeout_secs` is reset by every valid
reasoning or content event, and `stream_total_timeout_secs` is the hard upper
bound for one response. AI Runs and placeholder messages are persisted before
execution, so closing or refreshing the browser does not cancel an answer.
When Redis is enabled and reachable, live answer revisions are kept there and
delivered to the browser over one server-sent event connection. Completion
writes the terminal answer to PostgreSQL (or SQLite in SQLite deployments) and
keeps the terminal Redis snapshot until its TTL expires. Redis errors fall back
to relational-database revisions instead of aborting the Run.

The environment model appears as a read-only option in the admin page. Admins
can add more OpenAI-compatible or Anthropic endpoints there using a label,
base URL, model name, and the name of an API-key environment variable. Secret
values are never stored in the database. Users select any enabled, credentialed
endpoint from the AI conversation toolbar.

The vector store is strictly opt-in. With `vector_store.enabled = false`, the
server does not connect to Qdrant or the embeddings endpoint. When enabled,
`dimensions` must exactly match `embedding_model`. Message insert, edit, recall,
and delete operations write a transactional outbox; a background worker updates
Qdrant and retries failures. AI Runs use a LangChain `Retriever` to embed the
question and search the full-room index. Qdrant returns three times `top_k`
candidates (capped at 50) with relevance scores; retrieval is filtered to the
one room attached to the AI session. Message IDs already present in the bounded
recent transcript are excluded inside the Qdrant query, so they cannot consume
the semantic result budget and older evidence is selected from the full indexed
history. Every hit is then batch-rechecked in the
relational database for active membership and current recall state before the
best `top_k` messages are sent to the model as LangChain `Document` values.
Each injected result has a stable `S1`, `S2`, ... source label, message ID,
sender, timestamp, and relevance score; answers are instructed to cite those
labels when relying on retrieved evidence.
The migration backfills every existing non-recalled text message into the
outbox, so after the queue reaches zero every room message is eligible for RAG.
The answer footer displays recent transcript messages separately from full-room
RAG matches. These are the messages actually injected into one answer, not the
total number of indexed messages; bounded injection keeps latency and model
context usage stable while retrieval searches the complete indexed room corpus.

To verify RAG, open `/admin`, wait until the vector queue shows no pending or
retrying jobs, choose a room, and search for a distinctive fact from an older
message. A healthy probe returns that message with a relevance score. The
Qdrant point count measures indexed messages across all rooms, while the probe
and AI Retriever always apply a room filter. If the embedding request, Qdrant,
or retrieval deadline fails, the AI Run logs a warning and safely falls back to
the recent transcript instead of failing the whole answer.

The Compose stack exposes Qdrant REST and gRPC on loopback ports 6333 and 6334
by default and stores its data in `qdrant_data`. Containers use
`CHAT_ROOM_VECTOR_URL=http://qdrant:6333`; override the host ports with
`QDRANT_HTTP_PORT` and `QDRANT_GRPC_PORT`. Configure an API key and TLS before
making Qdrant reachable beyond the local host.

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
