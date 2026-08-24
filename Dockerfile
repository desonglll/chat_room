# syntax=docker/dockerfile:1.7

ARG BUN_VERSION=1.3.14
ARG RUST_VERSION=1.98.0

FROM rust:${RUST_VERSION}-bookworm AS backend-builder

WORKDIR /app
ENV CARGO_HTTP_TIMEOUT=600 \
    CARGO_NET_RETRY=10 \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    SQLX_OFFLINE=true

COPY deploy/cargo-config.toml /usr/local/cargo/config.toml
COPY Cargo.toml Cargo.lock build.rs ./
COPY migrations ./migrations
COPY migrations-postgres ./migrations-postgres
COPY src ./src

RUN --mount=type=cache,id=chatroom-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=chatroom-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=chatroom-target,target=/app/target \
    cargo build --locked --release --bin server --features api-only && \
    cp target/release/server /tmp/chat-room

FROM oven/bun:${BUN_VERSION} AS frontend-builder

WORKDIR /app
COPY web2/package.json web2/bun.lock ./
RUN --mount=type=cache,id=chatroom-bun-react,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

COPY web2/index.html web2/tsconfig.json web2/tsconfig.app.json web2/tsconfig.node.json web2/vite.config.ts ./
COPY web2/public ./public
COPY web2/src ./src
RUN bun run build

FROM nginx:1.29-alpine AS frontend

COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=frontend-builder /app/dist /usr/share/nginx/html

EXPOSE 80
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD wget --quiet --output-document=/dev/null http://127.0.0.1/healthz || exit 1

FROM debian:bookworm-slim AS backend

RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid 10001 chatroom && \
    useradd --system --uid 10001 --gid chatroom --home-dir /app --shell /usr/sbin/nologin chatroom && \
    mkdir -p /app/chat_attachments && \
    chown -R chatroom:chatroom /app

WORKDIR /app
COPY --from=backend-builder --chown=chatroom:chatroom /tmp/chat-room /usr/local/bin/chat-room

USER chatroom
ENV RUST_LOG="chat_room=info,tower_http=info"

EXPOSE 3000
VOLUME ["/app/chat_attachments"]
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
    CMD curl --fail --silent --show-error http://127.0.0.1:3000/api/config >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/chat-room"]
CMD ["--no-web", "--database-type", "postgres"]
