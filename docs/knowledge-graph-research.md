# Chatroom knowledge graph and GraphRAG research

Research snapshot: 2026-08-26

Scope: automatically analyze all visible messages, build one isolated knowledge
graph per room, and use the graph as additional retrieval context for the
existing AI conversation flow. Only official documentation, official source
repositories, official release records, and official license material are used
below.

## Executive decision

Use **Graphiti 0.29.3 with FalkorDB as a private Python sidecar**, while retaining
the existing Qdrant message index for raw-message semantic evidence.

The sidecar should import `graphiti-core==0.29.3`; it should not expose the
upstream demo server directly. The Rust application remains the system of
record, authorization boundary, outbox owner, and answer orchestrator. Each
room UUID becomes a Graphiti `group_id`, and each message version becomes an
`EpisodeType.message` episode. Graphiti search contributes current facts and
relationships; Qdrant continues to contribute verbatim messages with stable
message IDs and citations.

This choice is driven by the workload:

- Chat is an ordered, continuously changing event stream, not a mostly static
  document corpus. Graphiti is explicitly designed for incremental episodes,
  temporal validity, contradiction invalidation, and provenance without full
  graph recomputation.
- Graphiti accepts `group_id` on both ingestion and search. Its FalkorDB driver
  maps each group to a separate named graph, so the sidecar can turn a room UUID
  into a controlled graph name and prevent cross-room entity resolution.
- Microsoft GraphRAG is now a maintenance-mode research project and remains a
  batch table pipeline. Neo4j's own automatic KG builder is still marked
  experimental. LightRAG has the most complete ready-made REST service, but its
  stable deployment model binds a server instance to one immutable workspace;
  putting private rooms into one workspace allows cross-room entity merging,
  while one process/port per room is operationally unsuitable.
- Neo4j remains the stronger general-purpose graph database, but Graphiti
  0.29.3 routes a non-default group through its driver database-selection path.
  One physical Neo4j database per room is operationally unsuitable and depends
  on edition capabilities. FalkorDB's group-to-graph routing matches the room
  isolation contract directly. Its SSPLv1 license requires deployment review.

This is a pragmatic production recommendation, not a claim that Graphiti is a
turnkey enterprise product. Its OSS core is still `0.x`, and Graphiti's own
comparison says self-hosting requires the adopter to build the surrounding
security and operational system. Zep is the vendor's managed alternative when
an SLA, hosted governance, and support are required.

Primary evidence:

- [Graphiti README: incremental temporal context graphs, Graphiti versus Zep,
  requirements, and provider caveats](https://github.com/getzep/graphiti/blob/v0.29.3/README.md)
- [Graphiti 0.29.3 release](https://github.com/getzep/graphiti/releases/tag/v0.29.3)
- [Graphiti namespacing: add and search with `group_id`](https://help.getzep.com/graphiti/core-concepts/graph-namespacing)
- [Graphiti search modes](https://help.getzep.com/graphiti/working-with-data/searching)
- [Neo4j official Docker operations guide](https://neo4j.com/docs/operations-manual/current/docker/introduction/)

## Current repository constraints

The repository already has most of the application-side foundation that a
graph service needs:

- Rust/Axum is the API and authorization boundary.
- PostgreSQL and SQLite are supported as source-of-truth stores.
- Qdrant is already deployed in Compose and indexed asynchronously through
  `message_index_outbox`.
- Message insert, edit, recall, and delete enqueue a generation-checked
  `upsert` or `delete`; the migration backfills existing non-recalled messages.
- Qdrant payloads contain `room_id` and `message_id`; searches filter by room,
  then the Rust layer rechecks active membership and current recall state in the
  relational database before using a result.
- AI runs already degrade to recent conversation history when vector retrieval
  fails.
- Rooms are soft-deleted, so derived graph data must be explicitly removed or
  made unreachable when the room becomes inactive.

The graph implementation should preserve these properties rather than create a
second authority.

One existing component cannot be shared: `message_index_outbox`. Its Qdrant
worker deletes a job after applying it. A graph worker consuming the same rows
would race the vector worker and one destination would miss changes. Graph
delivery therefore needs a separate destination-specific outbox and checkpoint.

## Candidate comparison

| Candidate | Maturity and license | Incremental behavior | Room isolation | Retrieval and service surface | Fit for this repository |
| --- | --- | --- | --- | --- | --- |
| Microsoft GraphRAG 3.1.2 | MIT. Large official research codebase, but the README says it is largely in maintenance mode, accepts no new features/PRs, is a methodology demonstration, and is not a supported Microsoft offering. | Has `graphrag update`, but the 3.1.2 source detects deltas by document `title`, returns only `new_inputs`, and concatenates delta communities rather than reclustering the whole graph. Existing-title edits and deletes are not a reliable online message-sync contract. | No request-level room/tenant ACL. The safe model would be a separate project, output tables, and vector namespace per room. | Local, global, DRIFT, and basic search through CLI/Python APIs. Default output is Parquet plus a vector store. No supported production HTTP service/container. | Reject for the online path. It remains useful for periodic offline whole-corpus analysis or evaluation. |
| Neo4j GraphRAG for Python 1.18.0 | First-party Neo4j 1.x package. Package is Apache-2.0 with a small amount of PSF-licensed code. Neo4j Community is GPLv3; Enterprise is commercial. Retrieval is established, but the automatic Knowledge Graph Builder is explicitly experimental. | No first-class event cursor, document version, edit, or delete orchestration. Re-running `SimpleKGPipeline` requires application-owned stable IDs, deduplication, and cleanup. | A database can be selected, and retrieval supports filters/custom Cypher. However automatic extraction cannot reliably stamp caller metadata on every derived entity/relation without custom writers, so safe shared-database room scoping takes substantial custom work. | Excellent vector, hybrid, vector-plus-Cypher, Text2Cypher, and external Qdrant retrievers. It is a Python library; the application must build its own API and queue. | Strong graph database and retrieval toolkit, but not the shortest safe path to continuously building graphs from chat. |
| LightRAG 1.5.6 | MIT. Active project with an EMNLP 2025 paper, complete API/WebUI, official images, many backends, and substantial operational work. The latest GitHub item is named `v1.5.7rc2`; production should pin the last clearly stable `v1.5.6`, never `latest`. | Strong document lifecycle: custom IDs, asynchronous insert/status/recovery, selective document deletion, and shared entity/relation cleanup. Content/parser/chunking changes generally require delete plus reinsert. It is document-oriented rather than temporally modeling conversation contradictions. | `workspace` isolates files, database namespaces, table rows, Qdrant payloads, or Neo4j labels depending on backend. The stable server documentation still launches separate server instances/ports for different workspaces and warns that sharing one workspace across instances can corrupt/confuse data. One shared room workspace would also merge same-named entities across rooms. | Most complete ready-made REST surface: documents, query/stream/data, graph browse/mutation, local/global/hybrid/naive/mix, WebUI, Docker. | Best runner-up for a small number of document knowledge bases. Per-room instance routing and non-temporal message semantics make it a poor default for many chatrooms. |
| Graphiti 0.29.3 + FalkorDB | Graphiti is Apache-2.0 and active but still `0.x`; FalkorDB 4.18.8 is SSPLv1. OSS Graphiti is self-managed; managed Zep is the supported enterprise option. | `add_episode` integrates one chronological event, extracts/deduplicates entities and facts, invalidates contradicted facts, tracks validity windows, and preserves episode provenance without a full rebuild. | First-class `group_id` on episodes and searches. The FalkorDB driver selects a separate named graph for each room-derived group. | Hybrid semantic + BM25 fact search with RRF, graph-distance reranking, configurable node/edge/community recipes, core Python API, and an official FastAPI example. | Recommended. It most directly models ordered chat and gives this deployment a concrete per-room storage boundary behind a small Rust-owned service contract. |

## Detailed findings

### Microsoft GraphRAG

Microsoft GraphRAG 3.1.2 is sophisticated offline GraphRAG: its standard
pipeline extracts entities, relationships and claims, runs hierarchical Leiden
community detection, generates community reports, and embeds index fields. It
offers local entity-focused search, global map-reduce search, DRIFT, and a basic
vector baseline.

It is not a good online ingestion service for this product:

- The official repository now says the project is largely in maintenance mode,
  will not add features, and is not an officially supported Microsoft product.
- The default output model is a set of Parquet tables and a configured vector
  store, not a transactional graph database serving event updates.
- The CLI exposes `standard-update` and `fast-update`, but the 3.1.2 source
  computes new/deleted documents by `title`, then the update loader returns only
  `new_inputs`. A message edit retaining its title/identity is not selected for
  reprocessing. Community update code shifts delta IDs and concatenates old and
  new communities rather than reclustering the combined graph.
- The Python API expects callers to load and pass index tables. There is no
  supported multi-room HTTP service or request-level authorization/filtering.
- Indexing is LLM-intensive, and the official quick start warns users to start
  small. Minor/major upgrades can require regenerated configuration or index
  migration/rebuild.

Official sources:

- [GraphRAG repository, support status, MIT license, and versioning warning](https://github.com/microsoft/graphrag)
- [GraphRAG 3.1.2 release](https://github.com/microsoft/graphrag/releases/tag/v3.1.2)
- [Indexing methods](https://microsoft.github.io/graphrag/index/methods/)
- [Indexer outputs](https://microsoft.github.io/graphrag/index/outputs/)
- [CLI, including `update`](https://microsoft.github.io/graphrag/cli/)
- [Query engine modes](https://microsoft.github.io/graphrag/query/overview/)
- [Python API example](https://microsoft.github.io/graphrag/examples_notebooks/api_overview/)
- [`get_delta_docs` in 3.1.2](https://github.com/microsoft/graphrag/blob/v3.1.2/packages/graphrag/graphrag/index/update/incremental_index.py)
- [3.1.2 update loader returns only new inputs](https://github.com/microsoft/graphrag/blob/v3.1.2/packages/graphrag/graphrag/index/workflows/load_update_documents.py)
- [3.1.2 incremental community concatenation](https://github.com/microsoft/graphrag/blob/v3.1.2/packages/graphrag/graphrag/index/update/communities.py)

### Neo4j GraphRAG for Python

Neo4j GraphRAG for Python 1.18.0 is the strongest general graph toolkit in the
comparison. Its retrievers cover native Neo4j vector/full-text search,
vector-plus-Cypher traversal, hybrid search, Text2Cypher, and external vector
stores including Qdrant. Neo4j itself supplies a mature ACID graph engine,
official Docker images, volumes, operations manuals, and a managed Aura option.

The boundary is automatic graph construction. `SimpleKGPipeline` includes text
splitting, optional chunk embeddings, automatic or prescribed schema,
Document/Chunk lexical graphs, entity/relation extraction, pruning, writing,
and entity resolution, but its official documentation marks the whole feature
experimental. It has no application-level message event log or edit/recall
protocol. The service, idempotency rules, deletion semantics, and safe room
metadata propagation would all be custom code.

This package remains useful inside a future custom graph platform, but using it
instead of Graphiti would mean designing the temporal conversation model and
incremental lifecycle ourselves.

Official sources:

- [Neo4j GraphRAG Python repository and release](https://github.com/neo4j/neo4j-graphrag-python)
- [Package license](https://github.com/neo4j/neo4j-graphrag-python/blob/main/LICENSE.txt)
- [Knowledge Graph Builder experimental warning and pipeline](https://neo4j.com/docs/neo4j-graphrag-python/current/user_guide_kg_builder.html)
- [Retriever catalog, Qdrant adapter, and beta boundaries](https://neo4j.com/docs/neo4j-graphrag-python/current/user_guide_rag.html)
- [Neo4j Community/Enterprise feature and license summary](https://neo4j.com/pricing/)
- [Official Neo4j Docker image and persistence](https://neo4j.com/docs/operations-manual/current/docker/introduction/)
- [Production Docker configuration warning](https://neo4j.com/docs/operations-manual/current/docker/configuration/)

### LightRAG

LightRAG 1.5.6 has the most complete packaged application: a REST API, WebUI,
official signed container images, asynchronous document processing, status and
recovery, graph browsing/mutation, multiple query modes, and pluggable KV,
document status, graph, and vector stores. It can reuse PostgreSQL, Qdrant, and
Neo4j in several combinations.

Its workspace architecture is the blocker for this use case. The core
`workspace` value is immutable after initialization. The stable server guide
starts different workspaces as different instances on different ports and
warns that instances must not share a workspace. Although 1.5.6 contains a
`LIGHTRAG-WORKSPACE` request header helper for health state, supported document,
query, and graph routers are constructed around one initialized `LightRAG`
object; this is not a mature request-routed multi-workspace pool. Putting all
rooms in that one workspace would let entity resolution create cross-room graph
links. Running a container per room does not scale operationally.

Additional production cautions:

- The latest release entry is named `v1.5.7rc2` even though its GitHub release
  metadata is inconsistent. Pin `v1.5.6` or a later clearly stable, tested tag
  and an image digest.
- The official security advisory says server versions through 1.5.4 defaulted
  to no authentication while listening on `0.0.0.0`; 1.5.5 patched the issue.
  Any deployment still needs explicit auth, a non-default secret, network
  restriction, and rate limiting.
- Embedding model/dimension changes require clearing/reindexing the affected
  workspace. Backend selection should also be fixed before ingestion.
- The system owns four coordinated persistence roles (KV, vector, graph, and
  document status), which increases backup and consistency complexity when
  mixing existing PostgreSQL/Qdrant with a new graph database.

Official sources:

- [LightRAG repository](https://github.com/HKUDS/LightRAG)
- [LightRAG 1.5.6 release](https://github.com/HKUDS/LightRAG/releases/tag/v1.5.6)
- [MIT license](https://github.com/HKUDS/LightRAG/blob/main/LICENSE)
- [Core programming guide: insertion, deletion, storage, and workspace isolation](https://github.com/HKUDS/LightRAG/blob/v1.5.6/docs/ProgramingWithCore.md)
- [Stable API server guide, workspace instances, storage, and query API](https://github.com/HKUDS/LightRAG/blob/v1.5.6/docs/LightRAG-API-Server.md)
- [Official Docker deployment and image verification](https://github.com/HKUDS/LightRAG/blob/v1.5.6/docs/DockerDeployment.md)
- [Authentication security advisory and patched versions](https://github.com/HKUDS/LightRAG/security/advisories/GHSA-mmg5-8x8q-v934)

### Graphiti and Zep

Graphiti is purpose-built for conversational memory. An episode is a source
event; extracted entities and facts retain provenance back to it. Facts have
valid and invalid times, and newer events can invalidate contradictory facts
without erasing history. `add_episode` accepts text, structured JSON, and a
message source type. Search combines vector similarity and BM25, with RRF and
optional graph-distance reranking.

Dependencies and licenses:

- Pin `graphiti-core==0.29.3`; it requires Python 3.10+ and is Apache-2.0.
- Graphiti supports Neo4j 5.26+, FalkorDB 1.1.2+, and Amazon Neptune. Kuzu is
  deprecated in the official repository.
- Graphiti defaults to OpenAI for extraction and embeddings and supports other
  providers. Its README warns that ingestion works best with structured-output
  models; weaker or merely syntax-compatible models can return invalid schemas.
- Neo4j Community Edition is GPLv3 with community support. Enterprise features
  such as clustering, high availability, advanced security, and online backup
  require the commercial edition. License selection needs normal product/legal
  review; the Python sidecar's Apache license does not change the database
  license.

Service maturity is mixed. The repository publishes a FastAPI service image
and sample endpoints, but its source has no application auth, `/messages`
places work on an in-memory `asyncio.Queue`, and returns `202` before Graphiti
has committed the episode. A restart can lose queued work and a caller cannot
use that response as a durable outbox acknowledgement. The sample is useful as
an API reference, not as the production boundary.

The production sidecar should therefore be small and synchronous from the
outbox worker's perspective: await the Graphiti operation, then return success.
Rust already supplies the durable queue. Do not add Celery, Redis queueing, or
another scheduler until measurements prove it necessary.

Graphiti-specific correctness cautions:

- The `add_episode` docstring explicitly says episodes should be added
  sequentially and awaited. Enforce ordering per room; process different rooms
  concurrently under one global LLM semaphore.
- The `uuid` argument is not a usable creation idempotency key in 0.29.3.
  `add_episode(uuid=fresh_uuid)` first calls `get_by_uuid` and raises
  `NodeNotFoundError`; an [official open issue documents the
  defect](https://github.com/getzep/graphiti/issues/1646). For new episodes, do
  not pass a UUID. Use a deterministic application ingestion key/name, retain
  the UUID returned by Graphiti, and look up that exact key in the room before
  retrying. Passing an existing UUID also re-runs extraction, so it is unsafe as
  a blind retry.
- `add_episode_bulk` is documented for initial empty graphs and does not perform
  edge invalidation. Do not use it for ordinary live chat updates.
- `remove_episode` removes facts created by that episode and entities mentioned
  only by that episode, but does not recompute every shared entity summary or
  reverse every temporal invalidation. Treat edits/recalls as room-dirty events
  and provide a full room rebuild path. For a recall or privacy-sensitive
  deletion, rebuilding the room from currently visible messages is the
  correctness baseline.
- The official REST sample's episode-delete route does not even call
  `remove_episode`; it calls `EpisodicNode.delete` directly and can leave
  derived entities/facts behind. Do not reuse that route.
- `group_id` is a logical namespace, not an authorization system. Rust must
  authorize before calling the sidecar, pass exactly one room group, and
  revalidate returned provenance against current room/message state.
- Graphiti is a derived index. PostgreSQL/SQLite remains authoritative. If
  Neo4j is unavailable, chat writes must succeed and graph retrieval must fall
  back to current Qdrant/recent-history behavior.

Official sources:

- [Graphiti license](https://github.com/getzep/graphiti/blob/v0.29.3/LICENSE)
- [Adding episodes and bulk-ingestion warning](https://help.getzep.com/graphiti/core-concepts/adding-episodes)
- [Official episode deletion caveats for shared summaries and invalidated facts](https://help.getzep.com/deleting-data-from-the-graph)
- [Graph namespacing](https://help.getzep.com/graphiti/core-concepts/graph-namespacing)
- [`add_episode`, ordering note, search, and `remove_episode` source](https://github.com/getzep/graphiti/blob/v0.29.3/graphiti_core/graphiti.py)
- [0.29.3 pre-assigned UUID defect](https://github.com/getzep/graphiti/issues/1646)
- [Graphiti search recipes](https://help.getzep.com/graphiti/working-with-data/searching)
- [Official FastAPI service README and image](https://github.com/getzep/graphiti/blob/v0.29.3/server/README.md)
- [Official service's in-memory ingestion queue](https://github.com/getzep/graphiti/blob/v0.29.3/server/graph_service/routers/ingest.py)
- [Official sample's direct episode deletion implementation](https://github.com/getzep/graphiti/blob/v0.29.3/server/graph_service/zep_graphiti.py)
- [Official service search contract](https://github.com/getzep/graphiti/blob/v0.29.3/server/graph_service/routers/retrieve.py)
- [Official MCP deployment; Neo4j production recommendation](https://github.com/getzep/graphiti/blob/v0.29.3/mcp_server/README.md)

## Recommended architecture

```text
browser / desktop client
        |
        v
Rust chat-room API ------------------------------+
  | source of truth + room ACL                   |
  |                                              |
  +--> messages transaction                      |
  |       +--> message_index_outbox --> Qdrant   | raw message evidence
  |       +--> graph_index_outbox ---------------+
  |                                              v
  +--> AI retrieval -----> internal Graph API --> Graphiti core --> FalkorDB
          |                     (one room_id)        facts, time, provenance
          +--> Qdrant message retrieval
          +--> DB membership/recall recheck
          +--> bounded combined context --> existing AI model stream
```

### Ownership boundaries

Rust owns:

- user and room authorization;
- message and room lifecycle;
- durable graph outbox, retry policy, dead-letter state, and backfill cursor;
- graph feature flags and degradation;
- the final RAG prompt, source labels, and answer streaming;
- final revalidation of every graph source episode/message.

The Python sidecar owns:

- Graphiti client construction and pinned model/embedder adapters;
- Graphiti indices/constraints and Neo4j access;
- per-room sequential execution;
- idempotent episode-version application;
- room graph search and bounded subgraph serialization;
- no end-user sessions, no room membership database, and no final answer
  generation.

Neo4j stores only derived graph state. Qdrant remains the raw-message semantic
index. The relational database remains the only authority.

### Graph model

| Chat concept | Graphiti concept | Required identity/scope |
| --- | --- | --- |
| Room | `group_id` | Exact room UUID string; required on every write/search |
| Message version | `Episodic` node with `EpisodeType.message` | Deterministic application ingestion key mapped to Graphiti's returned UUID |
| Sender mention and domain concept | Entity node | Resolved only inside the room group |
| Extracted assertion | Temporal entity edge/fact | Include validity and contributing episode UUIDs |
| Message timestamp | `reference_time` | Original message `created_at`; edit event time for corrections |
| Message ID/generation | episode name/source metadata plus wrapper receipt | Enables retry, provenance, edit, recall, and rebuild |

Do not intentionally merge an entity across rooms in the first version. Two
rooms discussing the same person should receive distinct graph entities. That
is the correct privacy default. A future cross-room view must be a separate,
explicitly authorized aggregation feature.

### Internal API contract

Expose only an internal, authenticated API on the Compose network:

```text
PUT    /v1/rooms/{room_id}/events/{ingestion_key}
DELETE /v1/rooms/{room_id}/events/{ingestion_key}
POST   /v1/rooms/{room_id}/search
GET    /v1/rooms/{room_id}/subgraph
DELETE /v1/rooms/{room_id}
POST   /v1/rooms/{room_id}/rebuild
GET    /healthz
GET    /readyz
```

`PUT` must return success only after `add_episode` completes. Its payload should
include `message_id`, `generation`, `content_hash`, sender display/ID, content,
`created_at`, optional `edited_at`, and the previous Graphiti episode UUID when
known. The path key is deterministic, for example a hash of room ID, message ID,
and generation, and is also used as an exact, bounded episode name/source key.

Because 0.29.3 cannot create an episode with a caller-supplied UUID, the sidecar
must not pass `uuid` for a new episode. It first resolves the ingestion key in a
durable mapping/receipt and performs an exact room-scoped episode lookup. If the
same key and content hash already map to a completed Graphiti episode, return
that UUID without re-extraction. A hash mismatch is `409 Conflict`. Otherwise,
call `add_episode` without `uuid`, then durably record the returned UUID. A
timeout/crash between graph write and receipt write is an explicit
reconciliation case: retry performs exact room/key lookup before creating
anything. Run one sidecar writer initially and retain the Rust lease/per-room
lock so two requests cannot create the same key concurrently.

`search` accepts a query and bounded limit, but no arbitrary `group_ids` array.
The path room is the sole group. Return structured facts rather than a generated
answer:

```json
{
  "facts": [
    {
      "id": "edge-uuid",
      "fact": "...",
      "source_message_ids": ["message-uuid"],
      "valid_at": "2026-08-26T00:00:00Z",
      "invalid_at": null,
      "score": 0.82
    }
  ]
}
```

The custom response must retain episode/message provenance. The upstream sample
`FactResult` omits episode UUIDs, which is insufficient for this application's
authorization and citation checks.

### Outbox and ordering

Create `graph_index_outbox`; do not reuse `message_index_outbox`. It needs at
least:

- event/job ID, room ID, message ID, operation, and monotonic generation;
- created/message reference time and content hash;
- attempt count, next attempt time, last error, and updated time;
- a claim lease/owner so multiple Rust replicas cannot process the same
  expensive Graphiti operation concurrently;
- terminal dead-letter status after a configured retry ceiling;
- a uniqueness rule that makes re-enqueueing the same desired generation safe.

Process events in chronological/generation order within one room. Different
rooms may run concurrently, bounded by a global limit and the extraction
provider's rate limit. PostgreSQL workers should claim work without holding a
transaction open over the HTTP/LLM call; use a lease and generation check. A
SQLite deployment should use one graph worker.

Normal append flow:

1. Commit the message and graph outbox event in the same relational transaction.
2. Claim the event and derive its deterministic ingestion key.
3. Call the sidecar `PUT`; it awaits Graphiti and returns an idempotent result.
4. Persist the returned Graphiti episode UUID in the receipt and
   delete/complete only the exact claimed generation.
5. On timeout or `5xx`, release with exponential backoff plus jitter. Chat is
   unaffected.

Edit/recall flow:

1. The trigger/event increments generation.
2. An edit removes older message-version episodes and inserts the current
   version, or schedules a room rebuild when exact cleanup cannot be proven.
3. A recall/delete removes all known versions and marks the room dirty.
4. A room rebuild deletes the Graphiti group and replays every current,
   non-recalled, non-empty message in chronological order.
5. Room soft deletion immediately disables retrieval and enqueues group purge.

The rebuild path is mandatory, not an administrative afterthought. It is the
repair mechanism for model changes, schema changes, missed events, recalls,
Graphiti upgrades, and consistency audits.

### Retrieval composition

For an AI question in one room:

1. Authorize active room membership in Rust before any retrieval.
2. Run existing Qdrant message retrieval and Graphiti fact search concurrently,
   each with a short independent timeout.
3. Translate returned Graphiti episode UUIDs to message IDs and batch recheck
   room ID, membership, recall state, and room deletion in the relational DB.
4. Discard a fact if no currently visible source message remains.
5. Allocate separate bounded budgets, for example up to six raw message hits
   and up to eight graph facts, deduplicated against the recent transcript.
6. Render graph facts as untrusted evidence with stable `G1`, `G2`, ... labels
   and message citations. Keep existing `S1`, `S2`, ... labels for Qdrant hits.
7. Generate the final answer with the existing Rust AI provider/streaming path.
8. If Graphiti or Neo4j fails, continue with Qdrant/recent history and record the
   degraded state.

This deliberately avoids calling an LLM again inside a generic GraphRAG answer
endpoint. Graphiti's search produces context; the existing chat server remains
responsible for the answer.

### Deployment and security

Add two services to the Compose stack:

- a custom `knowledge-graph` image containing the pinned Python lockfile and
  `graphiti-core==0.29.3`;
- an explicitly pinned, compatible Neo4j image with `/data` on a named volume.

Operational rules:

- Never use `latest`; pin package versions, container tags, and production
  image digests.
- Do not publish the sidecar, Bolt, or Neo4j Browser ports to non-loopback host
  interfaces. The browser/client never calls them directly.
- Require a service credential on every sidecar endpoint even on the internal
  network; rotate it through an environment-secret mechanism.
- Configure Neo4j authentication and production memory explicitly. Neo4j's
  official docs say the image defaults are intentionally small and not suitable
  as production settings.
- Treat chat text and graph output as untrusted prompt input. Use a fixed
  extraction ontology/instruction, schema validation, length limits, and no
  tool execution from extracted text.
- Disable Graphiti anonymous telemetry where required by policy.
- Use a dedicated extraction model only after it passes structured-output
  contract tests. An OpenAI-compatible endpoint is not automatically equivalent
  to reliable JSON-schema support.
- Track LLM token/cost budgets per room and rate-limit backfill separately from
  live messages.
- Treat Neo4j as rebuildable derived state. Document a full rebuild procedure;
  add Neo4j-native backups only if recovery-time requirements make replay too
  slow. The existing PostgreSQL/attachment export is not a Neo4j backup.

## Execution plan

### Phase 0: freeze contracts and versions

1. Record this decision in the implementation PR and pin
   `graphiti-core==0.29.3` plus a tested Neo4j image tag/digest.
2. Define the extraction ontology for chat: at minimum Person, Organization,
   Project, Topic, Decision, Task, Event, and Resource; keep additional types
   disabled initially to reduce hallucinated structure.
3. Define the internal API and error taxonomy: validation/conflict (`4xx`),
   retryable provider/database (`5xx`), and terminal unsupported content.
4. Define feature flags: service enabled, ingestion enabled, retrieval enabled,
   and per-room rebuild enabled. All default off outside Compose-enabled
   environments.
5. Set explicit privacy rules for recalls, deleted rooms, and retention before
   backfilling any production data.

Exit gate: API schema, room isolation invariant, deletion behavior, version
pins, and license choice are documented and testable.

### Phase 1: build the isolated Python sidecar

1. Add a small FastAPI/uv project with a locked dependency graph, multi-stage
   Docker build, non-root runtime, and read-only filesystem except temporary
   paths.
2. Construct Graphiti with Neo4j, the chosen structured-output extraction
   client, and the chosen embedder. Build indices/constraints during readiness
   initialization with bounded retries.
3. Implement synchronous `PUT`/`DELETE` episode handlers, deterministic
   ingestion keys, returned-UUID receipts, exact room/key/content-hash
   preflight, and per-room async locks. Add reconciliation for the graph-write /
   receipt-write crash window; do not pass a fresh UUID to Graphiti 0.29.3.
4. Implement bounded `search`, provenance-preserving response serialization,
   group purge, subgraph, and rebuild primitives.
5. Add service auth, request/body limits, connect/total timeouts, structured
   logs, metrics, liveness, and readiness.
6. Add unit tests with fake LLM/embedder and Neo4j integration tests in Compose.

Exit gate: retries do not duplicate an episode, every search is hard-scoped to
one room, and no endpoint is anonymously reachable from outside the service
network.

### Phase 2: add durable Rust synchronization

1. Add equivalent PostgreSQL and SQLite migrations for the graph outbox,
   triggers/backfill, indexes, leases, generation checks, and dead-letter state.
2. Add `[knowledge_graph]` configuration and environment overrides without
   reading secrets into persisted settings or admin responses.
3. Add a typed `reqwest` client with strict response parsing and bounded
   connect/request timeouts.
4. Add a background worker with per-room ordering, cross-room bounded
   concurrency, lease renewal, exponential backoff/jitter, and graceful
   shutdown.
5. Handle room soft deletion and add an admin-triggered room/full rebuild path.
6. Expose queue depth, retry/dead-letter count, oldest lag, last error, sidecar
   health, Neo4j readiness, and graph counts in admin diagnostics.

Exit gate: a sidecar outage never blocks message send/edit/recall; the queue
later drains exactly to the desired message generations.

### Phase 3: integrate GraphRAG retrieval

1. Add a room-scoped graph retriever beside the current Qdrant retriever.
2. Fetch Qdrant and graph candidates concurrently under separate deadlines.
3. Resolve graph provenance and reauthorize it in the relational database.
4. Add bounded `G*` evidence rendering and citations to the existing AI context
   without exposing raw Graphiti objects or chain-of-thought.
5. Preserve current fallback behavior when graph retrieval is disabled,
   unhealthy, empty, or timed out.
6. Add an authenticated room graph endpoint for nodes, edges, temporal fields,
   and source messages. Keep graph visualization separate from ingestion and
   retrieval correctness.

Exit gate: no cross-room result can be produced, a recalled source disappears,
and AI answers still work when Neo4j is stopped.

### Phase 4: backfill, reconcile, and roll out

1. Start with synthetic and staging rooms; tune extraction prompts against
   Chinese and mixed-language chat before production backfill.
2. Backfill room by room in message chronological order with a separate global
   concurrency/cost cap. Do not use `add_episode_bulk` where contradiction
   invalidation matters.
3. Compare relational visible message counts, applied episode receipts, queue
   state, and sampled graph provenance. Add an audit that can mark a room dirty
   and enqueue rebuild.
4. Enable ingestion first, then read-only graph inspection, then GraphRAG
   retrieval for a small allowlist of rooms, then general availability.
5. Record baseline p50/p95 ingest lag, search latency, graph size per room,
   extraction failure rate, rebuild time, provider tokens/cost, and fallback
   rate. Define alerts before broad rollout.
6. Test a version upgrade by rebuilding a copied room graph before changing the
   production pin. Never roll old and new Graphiti writers concurrently against
   the same derived graph without a compatibility test.

Exit gate: the backfill is auditable, recalls/deleted rooms are absent after
rebuild, operational limits are known, and rollback is only a feature-flag
change plus optional derived-index deletion.

## Required tests and acceptance criteria

Correctness:

- Insert one message, drain the outbox, and retrieve its fact with source
  message provenance.
- Retry after simulated response loss and after graph success/receipt failure;
  exact key lookup recovers the returned UUID, and only one episode version and
  one logical set of facts exists.
- Process two updates to one message and prove generation ordering.
- Recall/edit a message, rebuild, and prove old raw text and facts are absent
  from current search/subgraph results.
- Soft-delete a room and prove graph retrieval is immediately denied and the
  group is eventually purged.
- Crash the worker between sidecar success and outbox completion; restart and
  converge without duplicate extraction.

Isolation and security:

- Put the same entity name and distinct secrets in two rooms. Search each room
  and prove neither nodes, facts, episode IDs, nor summaries cross the boundary.
- Verify outsider, former member, and deleted-room requests are rejected before
  the sidecar call.
- Verify the sidecar rejects a missing/invalid service credential and cannot
  accept multiple arbitrary group IDs.
- Verify returned episode IDs are rechecked against current room and recall
  state before prompt injection.
- Fuzz long/malformed message content and LLM schema responses; no unbounded
  body, prompt, or graph property is accepted.

Resilience:

- Stop Neo4j and the sidecar independently. Chat remains writable; GraphRAG
  falls back; metrics identify the failed dependency; retries recover.
- Exercise PostgreSQL multi-replica claiming and SQLite single-worker behavior.
- Rate-limit the extraction provider and prove jittered retries do not form a
  hot loop.
- Fill the retry ceiling and prove the job becomes visible dead-letter state
  rather than retrying forever.
- Rebuild a large room under the configured cost/concurrency limits while live
  messages continue to queue and later converge.

Performance acceptance should be measured, not guessed. Establish budgets for
Graphiti search timeout, maximum graph evidence tokens, maximum nodes/edges in
the subgraph response, acceptable live-ingestion lag, and rebuild cost before
enabling all rooms.

## Production risks and mitigations

| Risk | Consequence | Required mitigation |
| --- | --- | --- |
| Graphiti is `0.x` and self-managed | API/schema behavior can change; no OSS SLA | Exact dependency/image pins, contract tests, staged upgrades, managed Zep evaluation if SLA becomes mandatory |
| Extraction LLM returns invalid or hallucinated structure | Wrong facts or failed jobs | Structured-output capable model, prescribed ontology, schema validation, provenance in UI/answers, rebuild capability |
| LLM cost and rate limits during backfill | High spend and long lag | Per-room/global quotas, separate backfill concurrency, token/cost metrics, pause/resume |
| At-least-once delivery around HTTP/graph commit | Duplicate extraction or inconsistent graph | Deterministic ingestion keys, returned-UUID receipts, exact key/content preflight, sidecar awaits commit, leased generation-checked outbox, reconciliation |
| Out-of-order events within a room | Incorrect temporal invalidation | Strict per-room serial ordering and chronological backfill |
| Recall/edit leaves derived summaries | Privacy/correctness leak | Purge versions, mark room dirty, full group rebuild from current visible messages |
| Logical `group_id` omitted or broadened | Cross-room data leak | Room ID in path, no multi-group public contract, Rust authorization and provenance recheck, isolation tests |
| FalkorDB unavailable | Retrieval and indexing degrade | Derived-index semantics, durable outbox, circuit breaker, Qdrant/recent-history fallback |
| Single-node FalkorDB lacks HA | Longer recovery during node failure | Accept rebuild-based RTO for initial rollout; evaluate FalkorDB Cloud or another supported topology when HA becomes mandatory |
| Two derived indexes disagree | Confusing evidence | Keep Qdrant and Graphiti roles distinct, cite provenance, DB recheck both, expose lag/health separately |
| Demo service's in-memory queue/auth model is reused | Lost jobs and exposed graph mutation | Custom thin synchronous sidecar; Rust owns durable queue and all user auth |

## Rejected shortcuts

- Do not expose Neo4j Browser, Bolt, LightRAG, Graphiti demo, or MCP endpoints to
  chat clients.
- Do not let the graph database decide authorization.
- Do not index all rooms into one unscoped graph and filter only after search.
- Do not share the Qdrant outbox between two consumers.
- Do not return `202 Accepted` to the Rust outbox worker before graph commit
  unless a second durable queue and status protocol are implemented.
- Do not delete Qdrant in the first release. Graph facts and raw message
  retrieval solve different evidence needs, and the existing path is already
  authorization-hardened.
- Do not use `latest` package or container tags.
- Do not treat a successful health endpoint as evidence that backfill,
  provenance, recall cleanup, or room isolation is correct.

## Final recommendation

Build the service with Graphiti 0.29.3 and FalkorDB behind a small private FastAPI
contract. Feed it from a new leased, per-room ordered relational outbox. Use
Graphiti only for derived temporal facts and graph structure; keep Qdrant for
raw message retrieval and keep all authorization, source validation, answer
generation, and fallback in Rust.

This is the smallest architecture that fits continuously changing chat without
discarding the repository's already-correct message RAG and permission model.
It also leaves two clean escape routes: replace the Graphiti sidecar with
managed Zep if operational support becomes more valuable than self-hosting, or
replace Graphiti extraction with a custom Neo4j GraphRAG pipeline later without
changing the Rust-facing service contract.
