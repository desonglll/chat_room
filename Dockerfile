# syntax=docker/dockerfile:1.7

ARG BUN_VERSION=1.3.14
ARG RUST_VERSION=1.97.1

FROM oven/bun:${BUN_VERSION} AS bun

FROM rust:${RUST_VERSION}-bookworm AS source
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun

WORKDIR /app
# CARGO_TARGET_DIR overrides the host-specific build.target-dir in
# .cargo/config.toml, which points at a macOS path and would bypass the cache
# mount on /app/target below.
ENV CARGO_HTTP_TIMEOUT=600 \
    CARGO_NET_RETRY=10 \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_TARGET_DIR=/app/target

COPY Cargo.toml Cargo.lock build.rs ./
COPY migrations ./migrations
COPY migrations-postgres ./migrations-postgres
# Workspace manifests only, so the install layer is cached until a dependency changes.
COPY package.json bun.lock tsconfig.base.json ./
COPY packages/core/package.json ./packages/core/
COPY packages/ui/package.json ./packages/ui/
COPY packages/web/package.json ./packages/web/

RUN --mount=type=cache,id=chatroom-bun,target=/root/.bun/install/cache \
    bun install --frozen-lockfile

COPY src ./src
# build.rs builds packages/web (its public/ holds the files src/web.rs embeds); packages/core
# and packages/ui are needed because @tg/web's build runs `tsc --noEmit` across them first.
COPY packages ./packages

FROM source AS builder

RUN --mount=type=cache,id=chatroom-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=chatroom-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=chatroom-target,target=/app/target \
    cargo build --locked --release --bin server && \
    cp target/release/server /tmp/chat-room

FROM postgres:17-bookworm AS runtime

RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid 10001 chatroom && \
    useradd --system --uid 10001 --gid chatroom --home-dir /app --shell /usr/sbin/nologin chatroom && \
    mkdir -p /app/chat_attachments /app/chat_backups && \
    chown -R chatroom:chatroom /app

WORKDIR /app
COPY --from=builder --chown=chatroom:chatroom /tmp/chat-room /usr/local/bin/chat-room

USER chatroom
ENV RUST_LOG="chat_room=info"

EXPOSE 3000
VOLUME ["/app/chat_attachments", "/app/chat_backups"]
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
    CMD curl --fail --silent --show-error http://127.0.0.1:3000/health/ready >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/chat-room"]
CMD ["--database-type", "postgres"]
