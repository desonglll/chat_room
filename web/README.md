# Echo Gate Web

Vue 3 + Vite + TypeScript browser client for the Echo Gate Rust server.

From the repository root, `cargo run` installs missing web
dependencies with Bun, builds the Vite bundle, and serves the web app together
with the REST and WebSocket endpoints on port 3000.

For frontend-only development, run the Rust server and Vite separately:

```sh
cargo run --bin server -- --no-web
cd web
bun run dev
```

Vite proxies `/api` and `/ws` to `127.0.0.1:3000`.

Authenticated users see group rooms and private conversations in one recent-
activity list. The contacts view at `/contacts` supports account search, friend
requests, accepted friends, blocked users, and starting a private conversation.
Private conversations reuse the server's room/message/WebSocket stack while
remaining hidden from room discovery and group-management actions.
