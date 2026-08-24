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

The Compose stack keeps database records in `postgres_data` and uploaded files
in `attachment_data`. PostgreSQL is only reachable from the internal Compose
network. Use `docker compose exec postgres psql -U chatroom -d chatroom` for
local database access.

For non-local deployments, set a strong `POSTGRES_PASSWORD`. If it contains URL
reserved characters, percent-encode them because the same value is placed in
`CHAT_ROOM_DATABASE_URL`.

## Use a published image

The GitHub Actions workflow tests Rust and Vue changes on pull requests. Pushes
to `master` and `v*` tags publish the same image to GitHub Container Registry
and, when configured, Docker Hub. Set one of these in `.env` to use a published
image instead of the local image name:

```dotenv
CHAT_ROOM_IMAGE=ghcr.io/desonglll/chat_room:latest
# Or: CHAT_ROOM_IMAGE=<dockerhub-username>/chat_room:latest
```

Then pull and start the services without a local rebuild:

```sh
docker compose pull chatroom
docker compose up -d --no-build
```

Published tags include `latest` for `master`, the branch or version tag, and a
`sha-<commit>` tag. Private packages require a `docker login ghcr.io` with
`read:packages` permission before pulling.

### Enable Docker Hub publishing

Create a Docker Hub repository with the same name as the GitHub repository
(`chat_room`), then add these under **GitHub repository settings > Secrets and
variables > Actions**:

| Type | Name | Value |
| --- | --- | --- |
| Variable | `DOCKERHUB_USERNAME` | Docker Hub username |
| Secret | `DOCKERHUB_TOKEN` | Docker Hub access token with read/write permission |

Do not store the Docker Hub account password in GitHub. Once the variable and
secret are present, the next `master` push or `v*` tag publishes matching tags
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

Back up `postgres_data`, `attachment_data`, and `redis_data` before upgrades,
or use the verified PostgreSQL/local-file commands documented in
`docs/configuration.md`. `docker compose down --volumes` deletes all three and
should not be used for a normal shutdown.
