# Chat Room Search Knowledge

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
