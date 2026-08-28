# Client Capability Matrix

The Rust HTTP and WebSocket contracts are the source of truth. Web, Desktop, and
CLI clients may expose different interaction surfaces, but they do not own Room
authorization, notification projection, conversation preference, Favorite, or AI
rules.

| Capability | Web | Desktop | CLI | Frozen server contract |
| --- | --- | --- | --- | --- |
| Login, rooms, messages, files | Complete | Complete | Complete | `/api/users/*`, `/api/rooms/*`, `/ws/{room_id}` |
| Friends and direct conversations | Complete | Complete | Not exposed | `/api/friends`, `/api/direct-chats`, `/api/conversations` |
| Global message search | Complete | Query, Room/type filters, source jump | Not exposed | `GET /api/messages/search` |
| Notification center | Complete | Filters, read state, source jump | Not exposed | `/api/notifications*`, account WebSocket signal |
| Conversation preferences | Complete | Complete | Not exposed | `PATCH /api/conversations/{room_id}/preferences` |
| Favorites | Complete | List/create/edit/delete/message source | Not exposed | `/api/favorites*` |
| AI reply suggestions | Complete | Complete | Not exposed | `POST /api/rooms/{room_id}/ai/suggest` |
| AI threads and selected-message questions | Complete | Threads and selected context with polling | Not exposed | `/api/ai/threads*`, `/api/ai/runs/{id}` |
| Administration and operations | Complete | Not exposed | Server admin commands only | `/api/admin/*`, server subcommands |

Desktop network code is intentionally an adapter over these released contracts.
`FeatureApiMixin` contains endpoint and payload mappings, while the Qt HTTP adapter
owns transport and authentication. Contract tests provide a fake JSON adapter;
WebSocket tests inject fake socket adapters into `RealtimeClient`.

The CLI remains focused on login, room discovery/creation, live chat, and file
transfer. Adding interactive parity there is deferred until a concrete terminal
workflow requires it; no domain rule should be copied into the CLI in the meantime.

Web-only advanced surfaces remain explicit: global-search date/sender filters,
Favorite attachments/collaborators/forwarding, AI model selection, catch-up,
extraction, and SSE progress. They are not duplicated into Desktop until a desktop
workflow justifies the additional UI; the underlying server contracts remain shared.
