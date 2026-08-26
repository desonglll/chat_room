# Container deployment

## Run locally

Copy `.env.example` to `.env` when you need to change the defaults, then start
the application and PostgreSQL:

```sh
docker compose up --build -d
docker compose ps
```

ChatRoom is available at <http://localhost:3000>. The application waits for
PostgreSQL and Redis to become healthy and applies `migrations-postgres`
automatically at startup. Redis caches authenticated sessions; PostgreSQL
remains authoritative and the application falls back to it if cache commands
fail.

Compose mounts the repository's `chat-room.toml` at `/app/chat-room.toml` as a
read-only file. Keep it beside `docker-compose.yaml`; after changing it, recreate
the application container so startup reloads the configuration:

```sh
docker compose up -d --force-recreate chatroom
curl http://localhost:3000/api/config
```

When `[ai].enabled = true`, set the environment variable named by
`[ai].api_key_env` in `.env`. A healthy AI configuration reports
`"ai_status":"ready"`; `disabled` means the mounted TOML did not enable AI,
while `missing_credentials` means the named environment variable is absent.

## Enable room knowledge graphs

Knowledge graph containers are behind an explicit Compose profile. Configure
these values in `.env` first:

```dotenv
CHAT_ROOM_KNOWLEDGE_GRAPH_ENABLED=true
CHAT_ROOM_KNOWLEDGE_GRAPH_TOKEN=<long-random-internal-token>
FALKORDB_PASSWORD=<long-random-database-password>
CHAT_ROOM_AI_API_KEY=<provider-key>
GRAPH_LLM_BASE_URL=https://api.openai.com/v1
GRAPH_LLM_MODEL=gpt-4.1-mini
GRAPH_EMBEDDING_BASE_URL=https://api.openai.com/v1
GRAPH_EMBEDDING_MODEL=text-embedding-3-small
GRAPH_EMBEDDING_DIMENSIONS=1536
```

Then start the profile:

```sh
docker compose --profile knowledge-graph up --build -d
docker compose --profile knowledge-graph ps
curl http://localhost:3000/api/config
```

Production Compose keeps FalkorDB and the graph service on the internal
network. For a host-local Rust server, use `docker-compose.local.yaml`; it binds
the graph API to `127.0.0.1:8090` and FalkorDB to `127.0.0.1:6380`.

The chat container intentionally has no startup dependency on this optional
profile. If the graph service starts later or restarts, durable outbox jobs
remain in PostgreSQL and resume automatically. Check the admin dashboard for a
healthy Knowledge Graph service and a zero pending-job count before judging a
room's graph complete.

The Compose stack keeps database records in `postgres_data` and uploaded files
in `attachment_data`. PostgreSQL is only reachable from the internal Compose
network. Use `docker compose exec postgres psql -U chatroom -d chatroom` for
local database access.

For non-local deployments, set a strong `POSTGRES_PASSWORD`. If it contains URL
reserved characters, percent-encode them because the same value is placed in
`CHAT_ROOM_DATABASE_URL`. The production Compose stack always connects the app
container to `postgres`, `redis`, and `qdrant` by their Compose service names.
Values such as `redis://127.0.0.1:6379/` in `.env` are for a host-local
`cargo run`; inside a container, `127.0.0.1` refers to that container itself.

## Use a published image

The GitHub Actions workflow tests Rust and Vue changes while building the
container image in parallel. After both jobs pass, pushes to `main` and `v*`
tags publish the same image to GitHub Container Registry
and, when configured, Docker Hub. Set one of these in `.env` to use a published
image instead of the local image name:

```dotenv
CHAT_ROOM_IMAGE=ghcr.io/desonglll/chat_room:latest
# Or: CHAT_ROOM_IMAGE=<dockerhub-username>/chat_room:latest
```

Then pull and start the services without a local rebuild:

```sh
docker compose pull chatroom
docker compose up -d --no-build --force-recreate
```

Published tags include `latest` for `main`, the branch or version tag, and a
`sha-<commit>` tag. Private packages require a `docker login ghcr.io` with
`read:packages` permission before pulling.

### Enable Docker Hub publishing

Create a Docker Hub repository with the same name as the GitHub repository
(`chat_room`), then add these under **GitHub repository settings > Secrets and
variables > Actions**:

| Type     | Name                 | Value                                              |
| -------- | -------------------- | -------------------------------------------------- |
| Variable | `DOCKERHUB_USERNAME` | Docker Hub username                                |
| Secret   | `DOCKERHUB_TOKEN`    | Docker Hub access token with read/write permission |

Do not store the Docker Hub account password in GitHub. Once the variable and
secret are present, the next `main` push or `v*` tag publishes matching tags
to both registries. GHCR uses the built-in `GITHUB_TOKEN` and needs no extra
secret.

## Operations

Inspect logs and health status with:

```sh
docker compose logs -f chatroom
docker compose ps
```

Stop containers without deleting stored data:

```sh
docker compose down
```

Back up `postgres_data`, `attachment_data`, `redis_data`, and (when enabled)
`falkordb_data` before upgrades,
or use the verified PostgreSQL/local-file commands documented in
`docs/configuration.md`. `docker compose down --volumes` deletes all three and
should not be used for a normal shutdown. With the knowledge-graph profile it
also permanently deletes the derived room graphs.
