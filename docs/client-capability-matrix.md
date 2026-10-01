# Client Capability Matrix

The Rust HTTP and WebSocket contracts are the source of truth. The web, desktop and terminal
clients expose different interaction surfaces, but none of them owns chat authorization,
notification projection, conversation preferences, favorites, or AI rules.

**One vocabulary (TG-602).** `/api/chats/*` is the only chat contract; the pre-rename
`/api/rooms/*` alias and its `name` duplicate of `title` were removed after every client moved
(TG-603 desktop, TG-602 terminal). `title` is the display name; `name` is still *accepted* on
input for compatibility. Names that stay on purpose — every `room_id` field, the WebSocket path
`/ws/{room_id}`, the `x-room-password` header — are listed in `CONTEXT.md` under **Room**.
`/api/chats` lists every chat the viewer is in, private chats included; the desktop and terminal
clients show private chats from `/api/conversations` and filter them out of the chat directory.

| Capability | Web (`packages/web`, React) | Desktop (PySide6) | CLI (ratatui) | Server contract |
| --- | --- | --- | --- | --- |
| Login, chats, messages, files | Complete | Complete | Complete | `/api/users/*`, `/api/chats/*`, `/ws/{room_id}` |
| Friends and private chats | Complete | Complete | Not exposed | `/api/friends`, `/api/direct-chats`, `/api/conversations` |
| Supergroups, channels, topics, invite links, slow mode | Complete | Not exposed | Not exposed | `/api/chats/{id}/…` (TG-201…TG-207) |
| Channel comments | Complete | Not exposed | Not exposed | `/api/chats/{id}/discussion`, `/api/chats/{id}/posts/{post}/comments` |
| Global search with tabs | Complete (7 tabs, filters, recent) | Query, chat/type filters, source jump | Not exposed | `GET /api/messages/search` |
| Notifications, exceptions, sounds | Complete | Filters, read state, source jump | Not exposed | `/api/notifications*`, `/api/chats/{id}/notification-exception` |
| Conversation preferences, archive, folders | Complete | Preferences | Not exposed | `/api/conversations/{room_id}/preferences`, `/api/users/me/folders` |
| Favorites / Saved Messages | Complete | List/create/edit/delete/message source | Not exposed | `/api/favorites*` |
| Rich messages (voice, round video, stickers, GIFs, polls, albums, contacts, locations, link previews) | Complete | Text and files | Text and files | per-kind endpoints under `/api/chats/{id}/…`, `/api/link-preview` |
| Cloud drafts, scheduled messages | Complete | Not exposed | Not exposed | `/api/chats/{id}/draft`, `/api/chats/{id}/scheduled*` |
| Privacy, 2FA, sessions, profile photos | Complete | Profile | Not exposed | `/api/users/me/privacy*`, `/api/users/me/2fa*`, `/api/sessions*` |
| Themes, wallpapers, language | Complete | Not exposed | Not exposed | `/api/users/me/wallpapers*` (theme/accent/language are per device) |
| AI reply suggestions and threads | Disabled for now (product decision) | Complete | Not exposed | `POST /api/chats/{id}/ai/suggest`, `/api/ai/threads*` |
| Administration and operations | Complete | Not exposed | Server admin commands only | `/api/admin/*`, server subcommands |

Desktop network code is an adapter over these contracts: `FeatureApiMixin` maps endpoints and
payloads, the Qt HTTP adapter owns transport and authentication, contract tests use a fake JSON
adapter, and WebSocket tests inject fake sockets into `RealtimeClient`.

The CLI stays focused on login, chat discovery/creation, live chat and file transfer. Interactive
parity there waits for a concrete terminal workflow; no domain rule is copied into the CLI.

Cloud-draft guarantees (TG-008): a `PUT` identical to the stored draft is idempotent (no
`updated_at` bump, no broadcast); an empty text with no reply target clears the draft; `GET`
answers the stored draft or JSON `null` (the reconnect path). Clients that do not implement
drafts ignore the `draft_updated` frame (unknown-frame tolerance, TG-007).
