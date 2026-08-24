# Container deployment

## Architecture

The Compose stack runs three services on one private bridge network. It runs
the backend plus one selected frontend image; React and Vue are built and
published as separate frontend images:

| Service | Image | Purpose | Host port |
| --- | --- | --- | --- |
| `frontend` | `chatroom-frontend-react` or `chatroom-frontend-vue` | Serves the selected application and proxies HTTP/WebSocket traffic | `3000` |
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

Copy `.env.example` to `.env` when you need to change the defaults. Select one
frontend with `CHAT_ROOM_FRONTEND=react` or `CHAT_ROOM_FRONTEND=vue`, then build
and start the stack:

```sh
docker compose up --build -d
docker compose ps
```

To build both frontend images locally, run the builds separately. Each command
creates its own image tag and does not change the database volume:

```sh
CHAT_ROOM_FRONTEND=react docker compose build frontend
CHAT_ROOM_FRONTEND=vue docker compose build frontend
```

Switch the running frontend by changing `CHAT_ROOM_FRONTEND` in `.env` and
running `docker compose up -d` again. The backend and PostgreSQL services remain
the same.

ChatRoom is available at <http://localhost:3000>. Compose waits for PostgreSQL
and the backend health check before starting the frontend. Database records stay
in `postgres_data`; uploaded files stay in `attachment_data`.

## Use published images

Pushes to `master` and `v*` tags publish the backend, React frontend, and Vue
frontend images to both GitHub Container Registry and Docker Hub. Docker Hub
publishing requires these GitHub Actions repository secrets:

- `DOCKERHUB_USERNAME`: Docker Hub account name.
- `DOCKERHUB_TOKEN`: Docker Hub access token with write permission.

When either secret is missing, CI skips Docker Hub and continues publishing to
GHCR. Once both are present, the same build is tagged and pushed to both
registries.

Run the setup wizard from the repository root to create the token and store
both values as GitHub Actions secrets without writing them to disk:

```sh
./scripts/setup-dockerhub-publishing.sh
```

The Docker Hub repository prefix defaults to `desonglll/chat_room`. Set the
GitHub Actions repository variable `DOCKERHUB_IMAGE_NAME` to override it.

Select one frontend and set its published image in `.env`:

```dotenv
CHAT_ROOM_BACKEND_IMAGE=ghcr.io/desonglll/chat_room-backend:latest
CHAT_ROOM_FRONTEND=react
CHAT_ROOM_FRONTEND_IMAGE=ghcr.io/desonglll/chat_room-frontend-react:latest
```

For Vue, use `CHAT_ROOM_FRONTEND=vue` and
`ghcr.io/desonglll/chat_room-frontend-vue:latest`.

The equivalent Docker Hub images are:

```dotenv
CHAT_ROOM_BACKEND_IMAGE=desonglll/chat_room-backend:latest
CHAT_ROOM_FRONTEND=react
CHAT_ROOM_FRONTEND_IMAGE=desonglll/chat_room-frontend-react:latest
```

For Vue, use `desonglll/chat_room-frontend-vue:latest`.

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
