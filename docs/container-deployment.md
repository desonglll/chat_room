# Container deployment

## Run locally

Copy `.env.example` to `.env` when you need to change the defaults, then start
the application and PostgreSQL:

```sh
docker compose up --build -d
docker compose ps
```

ChatRoom is available at <http://localhost:3000>. The application waits for
PostgreSQL to become healthy and applies `migrations-postgres` automatically at
startup.

The Compose stack keeps database records in `postgres_data` and uploaded files
in `attachment_data`. PostgreSQL is only reachable from the internal Compose
network. Use `docker compose exec postgres psql -U chatroom -d chatroom` for
local database access.

For non-local deployments, set a strong `POSTGRES_PASSWORD`. If it contains URL
reserved characters, percent-encode them because the same value is placed in
`CHAT_ROOM_DATABASE_URL`.

## Use a published image

The GitHub Actions workflow tests Rust and Vue changes on pull requests. Pushes
to `master` and `v*` tags also publish the image to GitHub Container Registry.
Set this in `.env` to use it instead of the local image name:

```dotenv
CHAT_ROOM_IMAGE=ghcr.io/desonglll/chat_room:latest
```

Then pull and start the services without a local rebuild:

```sh
docker compose pull chatroom
docker compose up -d --no-build
```

Published tags include `latest` for `master`, the branch or version tag, and a
`sha-<commit>` tag. Private packages require a `docker login ghcr.io` with
`read:packages` permission before pulling.

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

Back up both named volumes before upgrades. `docker compose down --volumes`
deletes the PostgreSQL database and uploaded attachments and should not be used
for a normal shutdown.
