# Container deployment

## Architecture

The Compose stack runs three services on one private bridge network while
building only two application images:

| Service | Image | Purpose | Host port |
| --- | --- | --- | --- |
| `frontend` | `chatroom-frontend` | Serves the React application and proxies HTTP/WebSocket traffic | `3000` |
| `backend` | `chatroom-backend` | Runs the Rust API and PostgreSQL migrations | None |
| `postgres` | `postgres:17` | Stores the existing application database | None |

Nginx forwards `/api`, `/api-docs`, and `/ws` to `backend:3000`. The backend
connects to `postgres:5432` with the same `CHAT_ROOM_DATABASE_URL` used by the
previous single-image application service.

## Database compatibility

The PostgreSQL volume defaults to the stable name `chatroom_postgres_data`,
which is the volume created by the previous Compose file with `name: chatroom`.
An existing named volume is reused in place; the backend applies only pending
`migrations-postgres` migrations.

The defaults `POSTGRES_USER=postgres`, `POSTGRES_PASSWORD=postgres`, and
`POSTGRES_DB=chatroom` match the legacy local stack. An initialized PostgreSQL
volume does not recreate users when these variables change, so deployments with
custom legacy credentials must keep the same values in `.env`.

To use a differently named existing volume, set it in `.env`:

```dotenv
POSTGRES_VOLUME_NAME=existing_postgres_volume
```

Older Compose versions may have created an anonymous PostgreSQL volume. Stop
PostgreSQL before copying it once into the stable volume, and keep the source
volume until the migrated stack has been verified:

```sh
docker volume create chatroom_postgres_data
docker run --rm --user root --entrypoint cp \
  --mount type=volume,src=<legacy-volume>,dst=/from,readonly \
  --mount type=volume,src=chatroom_postgres_data,dst=/to \
  postgres:17 -a /from/. /to/
```

Do not copy into a volume that already contains a PostgreSQL database.

## Run locally

Copy `.env.example` to `.env` when you need to change the defaults, then build
and start the stack:

```sh
docker compose up --build -d
docker compose ps
```

ChatRoom is available at <http://localhost:3000>. Compose waits for PostgreSQL
and the backend health check before starting the frontend. Database records stay
in `postgres_data`; uploaded files stay in `attachment_data`.

## Use published images

Pushes to `master` and `v*` tags publish separate backend and frontend images
to GitHub Container Registry. Set both variables in `.env`:

```dotenv
CHAT_ROOM_BACKEND_IMAGE=ghcr.io/desonglll/chat_room-backend:latest
CHAT_ROOM_FRONTEND_IMAGE=ghcr.io/desonglll/chat_room-frontend:latest
```

Then pull and start without a local rebuild:

```sh
docker compose pull backend frontend
docker compose up -d --no-build
```

## Operations

Inspect logs and health status with:

```sh
docker compose logs -f frontend backend postgres
docker compose ps
```

Stop containers without deleting stored data:

```sh
docker compose down
```

Back up `postgres_data` and `attachment_data` before upgrades. Running
`docker compose down --volumes` deletes both and is not a normal shutdown.
