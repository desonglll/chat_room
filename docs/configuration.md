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

The server loads `.env`, then reads `chat-room.toml`, and finally applies
`CHAT_ROOM_*` environment overrides. Environment values do not use template
syntax inside TOML; they override the matching runtime field after TOML is
parsed. Every runtime section has corresponding variables in `.env.example`.
Empty or invalid optional overrides are ignored, while the effective result is
still checked by the normal configuration validation. AI extra-body variables
use JSON object syntax.

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

[observability]
json_logs = false
# Supported values: redis, vector_store, ai_provider. Database is always required.
required_dependencies = []

[backup]
enabled = false
interval_minutes = 1440
retention_count = 7
target_backend = "local"
directory = "chat_backups"
include_files = false

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
vision_model = ""
vision_base_url = ""
vision_api_key_env = "CHAT_ROOM_AI_API_KEY"
vision_max_images = 8
vision_max_total_images = 64
vision_max_image_mib = 8
vision_request_timeout_secs = 60
max_context_messages = 30
analysis_context_messages = 5000
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
score_threshold = 0.55
embedding_base_url = ""
embedding_model = ""
embedding_api_key_env = "CHAT_ROOM_AI_API_KEY"
rerank_enabled = false
rerank_base_url = ""
rerank_model = ""
rerank_api_key_env = "CHAT_ROOM_AI_API_KEY"
rerank_timeout_ms = 2000
rerank_score_threshold = 0.35
worker_interval_ms = 500

[admin]
# Deprecated one-time upgrade import; persistent roles use user IDs.
usernames = []
orphan_retention_hours = 168
deleted_room_retention_days = 30

[auth]
session_lifetime_days = 30
registration_mode = "open"
rate_limit_window_secs = 60
rate_limit_ip_attempts = 60
rate_limit_account_attempts = 10

[security]
cors_allowed_origins = []
trust_proxy_headers = false
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

`GET /health/live` reports only that the process can serve HTTP. `GET
/health/ready` probes the database and configured Redis, Qdrant, and AI model
adapters. The database is always required. An unavailable optional dependency
returns HTTP 200 with `status = "degraded"`; an unavailable dependency listed in
`observability.required_dependencies` returns HTTP 503 with `status =
"not_ready"`. `GET /metrics` exposes Prometheus text with aggregate request,
latency, connection, and fixed dependency gauges. It never labels metrics with
Room IDs, account IDs, message content, raw URLs, or request IDs.

Every HTTP response includes `x-request-id`. A caller-supplied identifier is
accepted only when it is short and uses a restricted ASCII character set;
otherwise the server generates a UUID. Request logs contain that ID, the HTTP
method, the matched route template, status, and latency, but no query string or
body. Set `observability.json_logs = true` or
`CHAT_ROOM_OBSERVABILITY_JSON_LOGS=true` for structured JSON log lines.

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

`vision_model` optionally enables image understanding for AI room analysis. It
uses an OpenAI-compatible chat-completions endpoint from `vision_base_url` (or
falls back to `ai.base_url`) and reads its secret from `vision_api_key_env`.
Only non-sensitive image attachments already present in the authorized message
context are eligible. Targeted questions rank up to `vision_max_images` images
by their surrounding message text. Broad room summaries preserve chronology and
process up to `vision_max_total_images` images in bounded concurrent batches.
Each image must be no larger than `vision_max_image_mib`. The resulting
structured OCR and visual observations are cached and bound to
the original attachment label and message ID before being supplied to the
answer model. Files that are not images remain available as attachment metadata
but are not sent to the vision model. Individual image or provider failures are
skipped without failing the whole AI Run.

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
history. Every hit is then batch-rechecked in the relational database for
active membership and current recall state. When `rerank_enabled` is true,
those authorized candidates are sent to the configured cross-encoder reranker
and its best `top_k` results are used. A failed or timed-out rerank falls back
to vector similarity ordering. Only the final `top_k` messages are sent to the
chat model as LangChain `Document` values.
Each injected result has a stable `S1`, `S2`, ... source label, message ID,
sender, timestamp, and relevance score; answers are instructed to cite those
labels when relying on retrieved evidence. The answer stores the matching source
metadata and shows a source list whose entries deep-link to the original room
message.
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

## Automated backup and restore drills

SQLite and PostgreSQL both support scheduled backups, immediate retained
backups, downloadable exports, validation, and confirmed restore from `/admin`.
Configure `[backup]` or the corresponding `CHAT_ROOM_BACKUP_*` environment
variables. `target_backend = "local"` writes archives to `directory`; the
Compose deployment mounts `chat_backups` as a persistent volume. The scheduler
runs immediately when no current scheduled result exists, then at
`interval_minutes`, and keeps the newest `retention_count` automatic archives.
Manual retained runs use the selected scope but do not change the schedule.

The two backup scopes are:

- **Database only** contains every account,
  room, message, setting, and other database record. Restoring it does not
  change the current attachment files.
- **Database and files** adds every durable local attachment file. The server
  briefly enters maintenance mode and disconnects chat clients while creating
  this package so the database and file snapshot stay consistent. This scope is
  unavailable when OSS is the attachment backend.

SQLite snapshots use `VACUUM INTO`, which creates a transactionally consistent
online database image rather than copying the live database, WAL, or shared
memory files. PostgreSQL snapshots use `pg_dump`; the runtime must include
matching `pg_dump` and `pg_restore` clients. Browser and retained exports are
`.tar.gz` archives containing `database.dump`, an optional
`attachments` directory, and `manifest.json` with the backup scope, SHA-256,
and byte count of every file. Restore rejects unsafe archive paths, unsupported
formats, extra files, missing files, and checksum mismatches before changing
the database.

Uploading to `/api/admin/backups/restore` only validates the archive and reports
its database kind, scope, total bytes, file count, checksum state, and validation
duration. It never changes live data. Execution uses the separate
`/api/admin/backups/restore/execute` endpoint, requires the fixed second
confirmation, enters maintenance mode, disconnects chat clients, and locks all
chat rooms. PostgreSQL uses `pg_restore --clean --if-exists --no-owner
--no-privileges --exit-on-error --single-transaction`; SQLite drains its pool,
preserves the current database beside the configured path, and atomically
activates the verified snapshot. Complete restores also preserve current local
files under `.pre-restore-*`. Redis is cleared, room caches are rebuilt, and
Qdrant is repopulated asynchronously. The system stays locked until an operator
checks `/admin` and unlocks it.

The operational RPO is `interval_minutes` plus the duration of an in-progress
backup. RTO is measured rather than assumed: backup runs store `duration_ms`,
validation reports `validation_duration_ms`, and confirmed restore reports
`restore_duration_ms`. Include index catch-up and manual verification in the
deployment's RTO objective. Rehearse restore against a temporary instance after
schema or storage changes; automated tests restore both database adapters and
compare key account, room, message, and attachment counts. A failed scheduled
run is persisted in `backup_runs`, logged, and shown prominently in `/admin`.

Database-only backup works with OSS. Complete backup and restore require local
attachment storage; use the object storage provider's versioning and backup
policy for OSS objects. Qdrant is derived state and is rebuilt, not packaged.

For offline complete backups, stop the chat server so the
PostgreSQL snapshot and attachment directory describe the same point in time,
then use the maintenance commands:

```sh
cargo run -- export --output backups/chat-room-2026-08-24
cargo run -- restore --input backups/chat-room-2026-08-24
```

Both commands use `chat-room.toml` and accept the global `--config`,
`--database-type postgres`, and `--database` overrides. An export creates a new
directory containing `database.dump`, the complete local `attachments`
directory, and `manifest.json` with a SHA-256 and byte count for every file.
The output path must not already exist.

The current offline maintenance commands produce complete PostgreSQL packages.
SQLite online backup and all scheduled operations are managed through the
running service. Neither path packages Qdrant.

The `/admin` dependency section can also enqueue a full vector synchronization
manually. This resets failed retries and is safe to run while the worker is
active.

The system dashboard is available at `/admin`. Access requires a persistent
system-administrator role tied to the account ID. For a new deployment, first
register the account while registration is open, stop the server, and grant the
first role through local database access:

```sh
cargo run --bin server -- bootstrap-admin --username ops-admin
```

The command succeeds only once and never sends a bootstrap secret over HTTP.
Afterward, an administrator can grant or revoke other administrators through
`/api/admin/system-admins`; the last administrator cannot be revoked or delete
their account.

`admin.usernames` and `CHAT_ROOM_ADMIN_USERNAMES` are deprecated upgrade aids.
On the first startup with this schema, matching accounts that already exist are
imported once and the setting is permanently ignored afterward. Registering an
unclaimed configured name later never grants authority.

Set `auth.registration_mode` (or `CHAT_ROOM_AUTH_REGISTRATION_MODE`) to `open`,
`invite_only`, or `disabled`. In invite-only mode, an administrator creates a
one-time invitation at `POST /api/admin/registration-invites`; only its hash is
stored, it expires, and the bearer value is returned only in that response.

Authentication attempts are limited independently by IP digest and normalized
account digest before Argon2 work. Defaults allow 60 attempts per IP and 10 per
account in 60 seconds. When Redis is enabled and reachable, all instances share
the counters; otherwise each process uses an in-memory fallback. Metrics expose
only aggregate allowed, blocked, and Redis fallback counts.

`security.cors_allowed_origins` accepts exact `http://` or `https://` origins
without a path, for example `https://chat.example.com`. An empty list is the
same-origin default; wildcard origins are rejected. The equivalent environment
variable is a comma-separated `CHAT_ROOM_CORS_ALLOWED_ORIGINS` list. Keep
`security.trust_proxy_headers` false unless a trusted reverse proxy removes
client-supplied `X-Forwarded-For` and `X-Real-IP` values before forwarding.

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
