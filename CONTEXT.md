# Echo Gate Search Knowledge

This context defines how conversations become searchable knowledge without changing the room's authorization boundary. Original messages remain the source of truth.

## Language

**Room**:
A private conversation and its membership boundary. Knowledge from different rooms must never be merged or retrieved together.
_Avoid_: Tenant, workspace, channel

**Message Vector**:
The searchable semantic representation of one active text message, scoped to its Room and retaining that message as its source.
_Avoid_: Graph node, global embedding

**Vector Sync Job**:
A durable intent to add, replace, or remove a Message Vector after its source message changes.
_Avoid_: Graph job, notification

**Retrieved Evidence**:
Active Room messages selected by semantic similarity and re-authorized before use. It is untrusted source material, never an answer or authorization source.
_Avoid_: Graph fact, truth, memory

**System Administrator**:
A User entrusted with deployment-wide operations independently of any Room role. The authority belongs to the User identity, not a username.
_Avoid_: Admin, Room admin

**Registration Invitation**:
An expiring, single-use permission to create one User while registration is invite-only.
_Avoid_: Invite code, shared registration password

**Catch-up Run**:
A durable personal AI Run whose unread-message boundaries are frozen by the server from the requesting User's Room read cursor.
_Avoid_: Client-selected summary range, Room summary message

**Extraction Run**:
A durable personal AI analysis over a User-selected time range in one Room. Its message context and every cited source are re-authorized by the server.
_Avoid_: Background task creation, cross-Room analysis

**Extraction Candidate**:
A deduplicated proposed decision or task that remains a projection until its User confirms it. Confirmation creates a personal Favorite or a new unassigned `open` Room Task; it never changes an existing Task.
_Avoid_: AI decision, AI-owned Task, source message
