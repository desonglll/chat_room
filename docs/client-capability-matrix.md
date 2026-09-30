# Client Capability Matrix

The Rust HTTP and WebSocket contracts are the source of truth. Web, Desktop, and
CLI clients may expose different interaction surfaces, but they do not own Chat
authorization, notification projection, conversation preference, Favorite, or AI
rules.

**TG-006 renamed the chat contract.** `/api/chats/*` is the contract; `/api/rooms/*`
survives as a routing-only deprecated alias onto the same handlers and is removed in
M6. All three clients below still call `/api/rooms/*` and keep working: the alias adds
the pre-rename `name` field beside `title` on chat descriptors, and both spellings are
accepted on input, on either path. `/api/conversations` has no alias of its own, so it
serves both spellings unconditionally. Everything else — including every `room_id`
field and the `x-room-password` header — is unchanged; see `CONTEXT.md` under **Room**
for the list of names that stay on purpose.

| Capability | Web | Desktop | CLI | Frozen server contract |
| --- | --- | --- | --- | --- |
| Login, chats, messages, files | Complete | Complete | Complete | `/api/users/*`, `/api/chats/*` (alias `/api/rooms/*`), `/ws/{room_id}` |
| Friends and direct conversations | Complete | Complete | Not exposed | `/api/friends`, `/api/direct-chats`, `/api/conversations` |
| Global message search | Complete | Query, Chat/type filters, source jump | Not exposed | `GET /api/messages/search` |
| Notification center | Complete | Filters, read state, source jump | Not exposed | `/api/notifications*`, account WebSocket signal |
| Conversation preferences | Complete | Complete | Not exposed | `PATCH /api/conversations/{room_id}/preferences` |
| Favorites | Complete | List/create/edit/delete/message source | Not exposed | `/api/favorites*` |
| AI reply suggestions | Complete | Complete | Not exposed | `POST /api/chats/{id}/ai/suggest` (alias `/api/rooms/{id}/ai/suggest`) |
| AI threads and selected-message questions | Complete | Threads and selected context with polling | Not exposed | `/api/ai/threads*`, `/api/ai/runs/{id}` |
| Administration and operations | Complete | Not exposed | Server admin commands only | `/api/admin/*`, server subcommands |
| Cloud drafts (TG-008) | Not yet (new React client, TG-105) | Not exposed | Not exposed | `PUT`/`GET /api/chats/{id}/draft` (alias `/api/rooms/{id}/draft`), `draft_updated` WebSocket frame — delivered only to the drafting account's own connections |

Desktop network code is intentionally an adapter over these released contracts.
`FeatureApiMixin` contains endpoint and payload mappings, while the Qt HTTP adapter
owns transport and authentication. Contract tests provide a fake JSON adapter;
WebSocket tests inject fake socket adapters into `RealtimeClient`.

The CLI remains focused on login, chat discovery/creation, live chat, and file
transfer. Adding interactive parity there is deferred until a concrete terminal
workflow requires it; no domain rule should be copied into the CLI in the meantime.
It deliberately still calls `/api/rooms/*`: that is the end-to-end check that the
deprecated alias works, and `tests/chat_api_alias_test.rs` is the automated half.

Clients may rely on these cloud-draft guarantees (TG-008): a `PUT` identical to the
stored draft is idempotent — it does not bump `updated_at` and broadcasts nothing;
saving an empty text with no reply target clears the draft; `GET` answers the stored
draft or JSON `null`, which is the reconnect path (a client that lost its socket
re-reads the draft over REST). The frozen Vue/PySide6/CLI clients ignore the
`draft_updated` frame (unknown-frame tolerance, TG-007) and never call the endpoints.

Web-only advanced surfaces remain explicit: global-search date/sender filters,
Favorite attachments/collaborators/forwarding, AI model selection, catch-up,
extraction, and SSE progress. They are not duplicated into Desktop until a desktop
workflow justifies the additional UI; the underlying server contracts remain shared.
