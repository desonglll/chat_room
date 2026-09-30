# Echo Gate Search Knowledge

This context defines how conversations become searchable knowledge without changing the Chat's authorization boundary. Original messages remain the source of truth.

## Language

**Chat**:
A conversation and its membership boundary — the authorization and knowledge-isolation unit. Knowledge from different Chats must never be merged or retrieved together. Stored in `chats`; `/api/chats/*` is its contract.
_Avoid_: Room, tenant, workspace, channel (a Channel is one Chat Type, not a synonym)

**Chat Type**:
Which of four shapes a Chat has: `private`, `group`, `supergroup`, `channel`. It decides the member ceiling, who may speak, what history a joiner sees, whether a Public Username, forum topics or slow mode are possible, and it is the last layer of the authorization decision — it can turn an allow into a deny but never the reverse.
_Avoid_: Room kind, conversation mode, is_group flag

**Supergroup Upgrade**:
The one-way, irreversible promotion of a `group` to a `supergroup`, triggered by passing 200 members, taking a Public Username, enabling forum topics, or setting slow mode. No other Chat Type is ever promoted and nothing is ever demoted.
_Avoid_: Room migration, group conversion, downgrade

**Public Username**:
A Chat's public `@handle`, unique among Chats that are not soft-deleted. Only a `supergroup` or a `channel` may hold one. Resolving it publicly uses `chats.access_hash`, never the row id.
_Avoid_: Room name, slug, invite link

**Room**:
Retired name for Chat. It survives on purpose in three places, and **none of them is an oversight**: the column `messages.room_id` and every other foreign key to `chats(id)` (renaming them would touch the indexes of 61 migrations and queries in about 30 modules to buy nothing but tidiness — `docs/tg/decisions.md` D-003); the deprecated `/api/rooms/*` routing alias, which keeps the frozen Vue, PySide6 and ratatui clients running until M6; and the data values those clients already consume (`room.settings` / `room.delete` permission keys, the `room_join_request` notification kind, `room.*` audit actions, the `x-room-password` header, `top_rooms` / `rooms_deleted` / `chat_rooms_locked` admin fields, and the WebSocket wire strings the clients string-match: System frame content such as `{user} joined the room` / `{user} left the room` / `room renamed to {title}` / `{user} was banned|removed from the room`, disconnect reasons such as `room password changed` / `room deleted` / `room locked`, and `auth_fail` reasons such as `room not found` / `this room requires a password - send auth, not join` — matched byte-for-byte in `web/src/roomSystemEvents.ts` and `web/src/chatProtocol.ts`, pinned by `tests/ws_frozen_wire_strings_test.rs`). Do not "fix" these.
_Avoid_: using Room for anything new

**Message Vector**:
The searchable semantic representation of one active text message, scoped to its Chat and retaining that message as its source.
_Avoid_: Graph node, global embedding

**Vector Sync Job**:
A durable intent to add, replace, or remove a Message Vector after its source message changes.
_Avoid_: Graph job, notification

**Retrieved Evidence**:
Active Chat messages selected by semantic similarity and re-authorized before use. It is untrusted source material, never an answer or authorization source.
_Avoid_: Graph fact, truth, memory

**System Administrator**:
A User entrusted with deployment-wide operations independently of any Chat role. The authority belongs to the User identity, not a username.
_Avoid_: Admin, Chat admin

**Registration Invitation**:
An expiring, single-use permission to create one User while registration is invite-only.
_Avoid_: Invite code, shared registration password

**Catch-up Run**:
A durable personal AI Run whose unread-message boundaries are frozen by the server from the requesting User's Chat read cursor.
_Avoid_: Client-selected summary range, Chat summary message

**Extraction Run**:
A durable personal AI analysis over a User-selected time range in one Chat. Its message context and every cited source are re-authorized by the server.
_Avoid_: Background task creation, cross-Chat analysis

**Extraction Candidate**:
A deduplicated proposed decision or task that remains a projection until its User confirms it. Confirmation creates a personal Favorite or a new unassigned `open` Chat Task; it never changes an existing Task.
_Avoid_: AI decision, AI-owned Task, source message

**Chat AI Policy**:
The Chat owner's rule for admitting new Chat-scoped AI work: disabled, all active members, or Chat administrators only. A policy change does not cancel AI Runs that were already admitted.
_Avoid_: Model configuration, membership role, running-Run kill switch

**AI Usage Record**:
A privacy-safe terminal accounting projection containing feature, Chat, model, status, estimated tokens, duration, and estimated cost. It never contains prompts, message text, attachment names, or Retrieved Evidence.
_Avoid_: AI transcript, provider log, billing source of truth

**Backup Run**:
A scheduled or administrator-triggered creation of one verified database package, optionally including the local attachment scope. Its record contains operational status and checksums, never message or attachment contents.
_Avoid_: Database copy, vector snapshot, restore point authorization

**Restore Validation**:
A read-only verification of archive paths, database kind, scope, and every declared SHA-256 before any live state changes. Validation never authorizes execution; restore requires a separate confirmation and leaves Chats locked afterward.
_Avoid_: Dry-run restore, implicit confirmation, checksum-only restore
