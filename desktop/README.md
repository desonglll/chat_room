# Echo Gate Desktop

Native PySide6 client for Echo Gate. It uses the same HTTP and WebSocket contracts as the web client; direct messages remain two-person rooms.

## Included

- Login and registration against a configurable server.
- Unified group/direct conversation list with unread counters.
- Room WebSocket history, text messages, replies, edits, recalls, read state, typing state, and emoji reactions.
- Attachment upload and capability-link download with the server's configured size limit.
- Message forwarding to one or more conversations and optional AI reply suggestions.
- Friend search, requests, accepted friends, blocked users, and direct-chat creation.
- Public-room discovery, private-room lookup by ID, room creation, and join requests.
- Owner/admin member review, invitations, role changes, and member removal.
- Profile editing for avatar, display name, signature, and homepage.
- Account WebSocket updates for new messages, unread counts, and social changes.
- System tray notifications that open the relevant conversation when clicked.
- Global message search with Room/type filters and reliable jumps to older results.
- A server-backed notification center with filters, unread state, and source navigation.
- Per-conversation pin, archive, notification-level, and timed-mute preferences.
- Favorite list/create/edit/delete plus one-click message favorites and source navigation.
- Durable AI threads, multi-message selected context, source citations, and run polling.

Direct messages use the same room transport and message timeline as group chats; the server keeps them private to exactly two members.

See the [client capability matrix](../docs/client-capability-matrix.md) for the
frozen Web/Desktop/CLI contract boundary and intentionally Web-only advanced
surfaces.

## Run locally

The official Qt wheels include Qt itself. Python 3.10 through 3.14 is supported by the selected PySide6 range.

```sh
cd desktop
uv sync --extra dev
uv run echo-chat --server http://127.0.0.1:3000
```

Run the Rust server separately when it is not already available:

```sh
cargo run --bin server
```

Closing the window keeps the application in the system tray when the platform provides one. Use the tray menu's **退出** command to stop it completely.

## Notifications

Notifications use Qt's native `QSystemTrayIcon` integration. The operating system may require notification permission and may suppress banners while Focus/Do Not Disturb is active. Unread badges remain visible in the conversation list even when a banner is not shown.

## Verify

```sh
uv run pytest
uv run ruff check src tests
uv run ruff format --check src tests main.py
QT_QPA_PLATFORM=offscreen uv run python -m echo_chat --help
```

## Package

Qt's deployment tool can create a platform-specific application bundle from the installed environment:

```sh
uv run pyside6-deploy main.py
```

Build separately on macOS, Windows, and Linux; native Qt application bundles are platform-specific.
