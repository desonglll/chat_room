# Room Knowledge Graph Implementation Plan

## Goal

Automatically project every active text message in every room into an isolated temporal knowledge graph, expose an authorized graph view, and add source-backed graph evidence to room AI answers. Existing chat behavior must remain unchanged when the feature is disabled or temporarily unavailable.

## Success Criteria

- Existing messages are backfilled and new, edited, recalled, or deleted messages converge automatically.
- Each room has a separate graph and every API read is checked against current room membership.
- AI retrieval only uses facts with at least one currently active source message in the same room.
- Graph outages never block message writes or ordinary chat; failed jobs retry with bounded exponential backoff.
- The feature is disabled by default, observable through admin service status, documented, tested, and deployable with pinned containers and packages.

## Architecture

1. SQLite and PostgreSQL migrations create a `message_graph_outbox`. Database triggers enqueue a generation-numbered Graph Sync Job for inserts, relevant updates, recalls, and deletes, and backfill all existing active text messages.
2. The Rust graph module drains ready jobs across rooms concurrently while preserving message order within a room. A job is removed only after the graph service completes the operation; newer generations cannot be removed by an older in-flight request.
3. An internal FastAPI service validates a shared bearer token and UUIDs, maps each room to its own FalkorDB graph, and calls pinned Graphiti APIs synchronously. Per-room locks serialize episode mutations and a process-wide semaphore bounds LLM work.
4. Graphiti uses the configured OpenAI-compatible chat and embedding endpoints to extract temporal entities and relationships. FalkorDB persists the graphs.
5. The Rust API exposes a room graph snapshot only after session and active-membership checks. It removes facts whose episode provenance no longer resolves to active messages in that room.
6. AI context retrieval searches only that room's graph. The application re-authorizes returned episode UUIDs, bounds the rendered evidence, labels facts as `G1`, `G2`, and treats the result as untrusted conversation data.

## Delivery Sequence

1. Record the domain vocabulary, technology decision, alternatives, licensing, and operational risks.
2. Define and test the authenticated graph-service contract with a fake engine.
3. Implement Graphiti episode upsert/delete, hybrid search, snapshots, readiness, idempotency, per-room serialization, and dependency pinning.
4. Add dual-database outbox migrations and tests for backfill, insert, edit, recall, and delete behavior.
5. Implement the Rust client, ordered worker, retry/generation guards, source authorization, graph endpoint, and health/status reporting.
6. Retrieve vector evidence and graph evidence independently for room AI, preserving graceful degradation and recording their combined retrieval usage.
7. Add Compose services, disabled-by-default configuration, secret handling, persistence, backup/restore notes, rollout instructions, and rollback behavior.
8. Run Python tests and static checks, Rust format/lint/tests, frontend tests/typecheck/build, Compose validation, migrations, source-size audit, and a live service smoke test where credentials permit.
9. Rebase or merge the latest remote `main`, resolve conflicts by preserving both changes, rerun affected verification, merge the feature branch to `main`, commit, and push both branches.

## Failure And Recovery

- The outbox is the durable handoff. Service timeouts and non-success responses retain the job and store a truncated error with exponential retry delay.
- Upserts use a deterministic `message:<UUID>` ingestion name because Graphiti 0.29.3 cannot create an Episode with a caller-provided UUID. Replays reconcile that name and content before changed content replaces the old Episode.
- Deletes are idempotent. The outbox stores the room UUID so deletion still targets the correct graph after the source row is gone.
- Startup does not require the optional graph service. Read paths use short timeouts and return no graph evidence, or an explicit service-unavailable response for graph visualization.
- Disabling the feature stops workers and retrieval without deleting SQL jobs or FalkorDB data. Re-enabling resumes convergence.

## Security And Privacy

- The graph service is internal-only in production and all `/v1` routes require a constant-time bearer-token comparison.
- Room UUIDs are converted to controlled graph names; callers cannot supply database or graph identifiers.
- Graph search is never an ACL. The Rust service validates current membership and every fact's source messages before returning or prompting with it.
- Prompts identify graph output as untrusted derived evidence and require source labels. Content limits bound prompt injection and resource consumption.
- Secrets remain in environment variables and are not returned by configuration or status APIs.

## Rollout And Rollback

Deploy FalkorDB and the graph service first, verify readiness, then enable `[knowledge_graph]` in one application environment. Observe pending jobs, retry counts, service latency, LLM usage, and room isolation before broader rollout. To roll back, disable the feature and deploy the previous application; SQL outbox rows and graph data may remain for later resumption. Destructive graph cleanup is a separate explicit operation.
