# Chat Room Knowledge

This context defines how conversations become searchable knowledge without changing the room's authorization boundary. Original messages remain the source of truth.

## Language

**Room**:
A private conversation and its membership boundary. Knowledge from different rooms must never be merged or retrieved together.
_Avoid_: Tenant, workspace, channel

**Episode**:
The graph projection of one active text message, named with the message UUID and retaining that message as provenance.
_Avoid_: Document, chunk, event

**Room Knowledge Graph**:
The temporal entities and facts inferred from the Episodes of exactly one Room.
_Avoid_: Global graph, shared knowledge base

**Graph Fact**:
An inferred, time-aware relationship backed by one or more Episodes. It is untrusted derived evidence and never an authorization source.
_Avoid_: Truth, memory

**Graph Sync Job**:
A durable intent to add, replace, or remove an Episode after its source message changes.
_Avoid_: Task, notification

**Graph Evidence**:
A Graph Fact whose source Episodes have been re-authorized against active Room membership and non-recalled messages before use.
_Avoid_: Answer, ground truth
