# Container deployment

The Compose stack contains exactly two application services on one private
bridge network:

| Service | Image | Purpose | Host port |
| --- | --- | --- | --- |
| `frontend` | `chatroom-frontend` | Serves the React application and proxies HTTP/WebSocket traffic | `3000` |
| `backend` | `chatroom-backend` | Runs the Rust API with SQLite | None |

The browser only connects to the frontend. Nginx forwards `/api`, `/api-docs`,
and `/ws` to `backend:3000` over the private Compose network.

## Run locally

Copy `.env.example` to `.env` when you need to change the defaults, then build
and start both images:

```sh
docker compose up --build -d
docker compose ps
```

ChatRoom is available at <http://localhost:3000>. Compose waits for the backend
health check before starting the frontend.

The stack keeps the SQLite database in `backend_data` and uploaded files in
`attachment_data`. Neither the backend port nor these volumes are exposed by
the frontend container.

## Use published images

The GitHub Actions workflow tests the Rust, Vue, and React projects. Pushes to
`master` and `v*` tags publish separate backend and frontend images to GitHub
Container Registry and, when configured, Docker Hub. Set both image variables
in `.env`:

```dotenv
CHAT_ROOM_BACKEND_IMAGE=ghcr.io/desonglll/chat_room-backend:latest
CHAT_ROOM_FRONTEND_IMAGE=ghcr.io/desonglll/chat_room-frontend:latest
# Or use <dockerhub-username>/chat_room-backend:latest and
# <dockerhub-username>/chat_room-frontend:latest.
```

Then pull and start the services without a local rebuild:

```sh
docker compose pull backend frontend
docker compose up -d --no-build
```

Published tags include `latest` for `master`, the branch or version tag, and a
`sha-<commit>` tag. Private packages require a `docker login ghcr.io` with
`read:packages` permission before pulling.

### Enable Docker Hub publishing

Create the Docker Hub repositories `chat_room-backend` and
`chat_room-frontend`, then add these under **GitHub repository settings >
Secrets and variables > Actions**:

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
docker compose logs -f frontend backend
docker compose ps
```

Stop containers without deleting stored data:

```sh
docker compose down
```

Back up both named volumes before upgrades. `docker compose down --volumes`
deletes the SQLite database and uploaded attachments and should not be used for
a normal shutdown.
