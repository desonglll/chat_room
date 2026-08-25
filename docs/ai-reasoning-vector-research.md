# AI reasoning controls, streaming, and message-vector architecture

Research snapshot: 2026-08-25

Scope: SiliconFlow OpenAI-compatible Chat Completions, the repository's pinned
`genai = 0.6.5`, and a future retrieval index for all authorized chat messages.
Only first-party documentation, official source repositories, and standards are
cited. No credential or real secret was read or recorded.

## Executive recommendation

1. Model "fast / thinking" as a provider-and-model capability, not as one
   universal OpenAI parameter. For SiliconFlow models that explicitly support
   the switch, fast mode is the top-level request field
   `"enable_thinking": false`. `thinking_budget` limits reasoning length; it is
   not an off switch.
2. With `genai 0.6.5`, send SiliconFlow extensions through
   `ChatOptions::with_extra_body(json!(...))`. Do not use
   `ReasoningEffort::None` to disable SiliconFlow reasoning, and do not use
   `ReasoningEffort::Budget(n)` for `thinking_budget`: the OpenAI adapter maps
   the former to `reasoning_effort: "none"` and intentionally emits nothing for
   the latter.
3. Keep the upstream and browser legs streaming. Consume both
   `ChatStreamEvent::ReasoningChunk` and `ChatStreamEvent::Chunk`; the UI may
   hide reasoning text, but the server still needs to consume it and expose a
   bounded progress state. Use separate time-to-first-event, idle, and total
   deadlines instead of the current single absolute 60-second deadline.
4. Start message retrieval with PostgreSQL + pgvector, using the official
   `pgvector` Rust crate's SQLx feature. It keeps vectors, memberships, recalls,
   edits, room deletion, backup, and authorization in one transactional system.
   Put embedding and retrieval behind a small module interface so Qdrant can be
   introduced later without changing chat-domain code.
5. Index asynchronously through a durable PostgreSQL outbox. External embedding
   calls cannot participate in the message transaction, so the achievable
   contract is at-least-once processing plus idempotent, version-checked upsert.
   Retrieval must always re-check current room access and current message state;
   the vector index is never an authorization source of truth.

## 1. SiliconFlow reasoning controls

### 1.1 Correct request shapes

SiliconFlow's endpoint is
`POST https://api.siliconflow.cn/v1/chat/completions`. Its OpenAI-compatible
schema defines `enable_thinking`, `thinking_budget`, and the ordinary `stream`
field as top-level JSON properties. `stream: true` returns SSE as tokens become
available and ends with `data: [DONE]`.
[SiliconFlow Chat Completions API](https://docs.siliconflow.cn/cn/api-reference/chat-completions/chat-completions)

Fast mode for a switchable model:

```json
{
  "model": "Qwen/Qwen3-8B",
  "messages": [{ "role": "user", "content": "..." }],
  "stream": true,
  "enable_thinking": false
}
```

Bounded thinking mode:

```json
{
  "model": "Qwen/Qwen3-8B",
  "messages": [{ "role": "user", "content": "..." }],
  "stream": true,
  "enable_thinking": true,
  "thinking_budget": 1024
}
```

`thinking_budget` accepts 128 through 32768 in the current schema. SiliconFlow
also warns that only Qwen3 natively forces reasoning to stop at the budget;
other reasoning models may continue. It therefore cannot serve as a reliable
cross-model latency cap. Final text is `content`; reasoning is the sibling
`reasoning_content`. In a stream these arrive as
`choices[0].delta.content` and `choices[0].delta.reasoning_content`.
[SiliconFlow reasoning guide](https://docs.siliconflow.cn/cn/userguide/capabilities/reasoning)

Do not send `enable_thinking` indiscriminately. As of this snapshot, the API
schema explicitly lists these switchable families/IDs:

- `Pro/zai-org/GLM-5`, `Pro/zai-org/GLM-4.7`, `zai-org/GLM-4.6`,
  `zai-org/GLM-4.5V`
- `deepseek-ai/DeepSeek-V3.2`, `Pro/deepseek-ai/DeepSeek-V3.2`,
  `deepseek-ai/DeepSeek-V3.1-Terminus`, and its `Pro/` variant
- `Qwen/Qwen3-8B`, `Qwen/Qwen3-14B`, `Qwen/Qwen3-32B`,
  `Qwen/Qwen3-30B-A3B`
- `Qwen/Qwen3.5-397B-A17B`, `122B-A10B`, `35B-A3B`, `27B`, `9B`, and
  `4B`
- `tencent/Hunyuan-A13B-Instruct`

This is a parameter-support list, not a guarantee that every ID remains
available to an account. SiliconFlow says models can be brought online/offline
or have capabilities changed. Validate the configured ID using the model-list
endpoint at deployment/startup, and fail configuration clearly rather than
silently changing semantics.
[SiliconFlow model list API](https://docs.siliconflow.cn/cn/api-reference/models/get-model-list)

`reasoning_effort` is not a portable substitute. SiliconFlow's current schema
limits it to `deepseek-ai/DeepSeek-V4-Flash` and documents only `high` and
`max`. A capability table should therefore be keyed by both provider and exact
model (or an intentionally reviewed model family).

Suggested application-level modes:

| UI mode | SiliconFlow payload | Unsupported-model behavior |
| --- | --- | --- |
| Fast | `enable_thinking: false` | Reject configuration or choose a documented non-thinking model; do not silently send an unknown field |
| Thinking | `enable_thinking: true`, optional validated `thinking_budget` | Omit unsupported budget; explain that latency is model-controlled |
| Provider default | omit both | Use only as an explicit admin/default choice |

### 1.2 Provider differences that must not leak across adapters

DeepSeek's own API uses a different extension for its current hybrid models:
`"thinking": {"type": "disabled"}`. Its official guide also documents
separate `reasoning_content`; when a request includes tools, subsequent turns
must preserve the assistant's reasoning content as required by that provider.
These are DeepSeek-direct rules, not SiliconFlow request syntax.
[DeepSeek thinking-mode guide](https://api-docs.deepseek.com/guides/thinking_mode/)
[DeepSeek Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion)

Qwen's official vLLM deployment example uses
`chat_template_kwargs: {"enable_thinking": false}`. Qwen explicitly identifies
this as a non-standard, framework-dependent extension. SiliconFlow instead
exposes `enable_thinking` at the request top level. Qwen model variants also
differ: hybrid Qwen3 models support switching, `Instruct-2507` is non-thinking,
and `Thinking-2507` is thinking-only. Do not infer capability from the string
`Qwen` alone.
[Qwen3 quick start](https://github.com/QwenLM/Qwen3/blob/main/docs/source/getting_started/quickstart.md)
[Qwen3 vLLM deployment](https://github.com/QwenLM/Qwen3/blob/main/docs/source/deployment/vllm.md)

The safe abstraction is consequently:

```text
ReasoningMode::Fast | Thinking { budget? } | ProviderDefault
    + ProviderKind
    + exact configured model
    -> validated provider-specific JSON fields
```

It is not `bool -> one universal OpenAI field`.

## 2. What `genai 0.6.5` actually supports

The repository pins `genai = "0.6.5"` in `Cargo.toml`. The published API gives
`ChatOptions` both provider-neutral reasoning hints and the low-level
`extra_body: Option<serde_json::Value>`, documented for non-standard fields on
OpenAI-compatible providers.
[genai 0.6.5 ChatOptions](https://docs.rs/genai/0.6.5/genai/chat/struct.ChatOptions.html)

For SiliconFlow, the appropriate per-call option is:

```rust
use genai::chat::ChatOptions;
use serde_json::json;

let options = ChatOptions::default().with_extra_body(json!({
    "enable_thinking": false
}));

client
    .exec_chat_stream(model, request, Some(&options))
    .await?;
```

When thinking is enabled and the exact model supports a budget:

```rust
let options = ChatOptions::default().with_extra_body(json!({
    "enable_thinking": true,
    "thinking_budget": 1024
}));
```

The locally installed official crate source confirms all relevant wire
behavior for version 0.6.5:

- `ChatOptions::with_extra_body` stores arbitrary JSON. The OpenAI adapter
  merges that object last into the top-level payload, so it can also override
  previously generated fields.
- The crate's own OpenAI-adapter test uses
  `json!({"temperature": 0.7, "enable_thinking": false})` and asserts that
  `enable_thinking` is top-level.
- `with_reasoning_effort(ReasoningEffort::None)` serializes the standard-ish
  field `reasoning_effort: "none"`. SiliconFlow does not document this as its
  off switch.
- `ReasoningEffort::Budget(u32)` is deliberately ignored by the OpenAI
  adapter; it does not become `thinking_budget`. Use `extra_body` for the
  SiliconFlow budget.
- The OpenAI stream parser recognizes both `delta.reasoning_content` and
  `delta.reasoning`, and emits `ChatStreamEvent::ReasoningChunk`. Ordinary
  answer text is emitted as `ChatStreamEvent::Chunk`.

Official published source/API:
[genai 0.6.5 source](https://docs.rs/crate/genai/0.6.5/source/)
[ChatStreamEvent](https://docs.rs/genai/0.6.5/genai/chat/enum.ChatStreamEvent.html)

`capture_reasoning_content` is only for concatenating reasoning into the final
`StreamEnd.captured_reasoning_content`; it is not required merely to receive
progressive `ReasoningChunk` events.

### 2.1 Why the current stream appears stuck

This conclusion is based on the repository code plus the documented upstream
protocol:

- `src/ai/mod.rs::answer_stream` calls `exec_chat_stream(..., None)`, so no
  provider extension currently disables thinking.
- Its loop returns only `ChatStreamEvent::Chunk`; every other successful event,
  including `ReasoningChunk`, is consumed and discarded.
- It establishes one absolute deadline before opening the upstream stream and
  applies that same deadline to every later read. Reasoning activity does not
  extend it.
- Axum returns HTTP 200 as soon as the downstream SSE response is established.
  The observed 57-170 ms HTTP completion therefore measures response-header/SSE
  establishment, not model completion.

The reported behavior is thus consistent with an upstream model spending the
first minute emitting only `reasoning_content`: the browser receives no answer
delta, then the server's absolute 60-second timeout fires. This is a strong
protocol-level explanation, not proof about a particular provider request;
provider trace IDs and event-level timing should be logged to confirm it.

Recommended streaming contract:

```text
upstream reasoning chunk -> consume immediately -> optional downstream status event
upstream content chunk   -> downstream delta immediately -> browser renders incrementally
upstream end             -> downstream done
upstream/provider error  -> bounded generic downstream error; detailed server log
```

Do not expose raw chain of thought by default. A coarse `status: reasoning`
event is enough to show liveness. Track at least:

- connection setup deadline;
- time to first upstream event;
- time to first visible content token;
- idle timeout reset by any valid upstream event;
- larger hard total deadline;
- cancellation when the browser disconnects.

## 3. Recommended vector architecture

### 3.1 Default: pgvector beside the source messages

Use PostgreSQL plus pgvector first. pgvector supports exact nearest-neighbor
search, HNSW and IVFFlat approximate indexes, cosine/inner-product/L2 distance,
normal PostgreSQL joins, ACID transactions, point-in-time recovery, and hybrid
use with PostgreSQL full-text search. HNSW generally gives a better
speed/recall trade-off than IVFFlat but takes more memory and builds more
slowly. pgvector recommends building indexes after the initial load and using
`CREATE INDEX CONCURRENTLY` in production.
[official pgvector README](https://github.com/pgvector/pgvector)

This project already uses PostgreSQL through SQLx 0.8. The official
`pgvector` Rust crate supports SQLx 0.8 with:

```toml
pgvector = { version = "0.4", features = ["sqlx"] }
```

It provides `pgvector::Vector` for SQLx bind/decode while leaving SQL and
transactions under application control.
[official pgvector-rust README](https://github.com/pgvector/pgvector-rust)

This is a better initial fit than a separate service because current access is
relational and mutable:

- messages belong to rooms;
- readable rooms require an active `room_memberships` row;
- recalled messages must disappear;
- edited content invalidates its prior vector;
- rooms are soft-deleted;
- direct-conversation membership can be removed and later restored;
- password-protected rooms currently require a valid room password even for an
  active member.

One SQL query can combine vector distance with those current facts. The vector
row must never grant access by itself.

### 3.2 Module boundary

Keep framework types out of `messages`, `rooms`, and HTTP DTOs. A narrow
`knowledge` module can own these ports:

```rust
trait EmbeddingProvider {
    async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn profile(&self) -> EmbeddingProfile; // provider, model, revision, dimensions
}

trait MessageIndex {
    async fn apply(&self, job: IndexJob) -> Result<(), IndexError>;
    async fn search(&self, query: AuthorizedSearch) -> Result<Vec<Hit>, IndexError>;
}
```

`AuthorizedSearch` should contain an authenticated user ID and explicit room
scope/unlock capabilities, not arbitrary caller-supplied room IDs. The
PostgreSQL implementation owns SQLx/pgvector; an eventual Qdrant implementation
owns Qdrant payloads and mandatory PostgreSQL rehydration.

Swiftide is the most relevant current Rust framework if a composable indexing
pipeline is desired: its official project provides streaming indexing/query
pipelines and feature-gated pgvector and Qdrant integrations. It can sit behind
the two ports for chunking, batching, embedding, and retry orchestration.
However, it is a broad 0.x framework and must not own this application's room
authorization or message lifecycle. Start with the smaller official
`pgvector` + SQLx integration; adopt Swiftide when its pipeline removes measured
complexity rather than as a prerequisite.
[Swiftide official repository](https://github.com/bosun-ai/swiftide)
[Swiftide API docs](https://docs.rs/swiftide/latest/swiftide/)

### 3.3 Proposed storage shape

Use one vector per message for ordinary short chat messages. For messages over
a chosen embedding-token limit, split deterministically and store
`chunk_index`; retrieve adjacent source messages after semantic search rather
than embedding arbitrary multi-message windows. That keeps edit/recall
invalidation exact.

Conceptual PostgreSQL schema (not a committed migration):

```sql
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE message_embeddings_v1 (
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content_sha256 BYTEA NOT NULL,
    embedding_model TEXT NOT NULL,
    embedding_revision TEXT NOT NULL,
    source_edited_at TIMESTAMPTZ,
    embedding vector(1024) NOT NULL,
    embedded_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (message_id, chunk_index)
);

-- Add after backfill and recall measurement.
CREATE INDEX CONCURRENTLY message_embeddings_v1_hnsw_cosine
    ON message_embeddings_v1
    USING hnsw (embedding vector_cosine_ops);
```

The 1024 dimension is an example that fits BAAI's official BGE-M3 model card;
the actual migration must be tied to the selected embedding profile.
[BAAI BGE-M3 model card](https://huggingface.co/BAAI/bge-m3)
[SiliconFlow Embeddings API](https://docs.siliconflow.cn/cn/api-reference/embeddings/create-embeddings)

Do not mix vectors from different models/revisions in one similarity index.
Dimension equality is not semantic compatibility. A model change should create
a shadow `message_embeddings_v2` (or an equally explicit profile-specific
table), backfill it, compare retrieval quality, then switch reads. Retain the
old table for rollback before later removal.

Do not duplicate raw message text or room ACL metadata in the in-database
vector table. Join through `message_id` to the current source rows. An external
vector store needs minimal filter metadata such as IDs and version hashes in
its payload, but PostgreSQL rehydration remains authoritative.

### 3.4 Durable asynchronous indexing

The existing `src/work_queue.rs` is an in-memory semaphore for admission
control. It is useful for concurrency limits but is not a durable indexing
queue. Add a PostgreSQL outbox/job table in the same transaction as each
message lifecycle change:

```text
message INSERT -> enqueue UPSERT(message_id, desired_content_hash)
message EDIT   -> immediately make old embedding ineligible; enqueue UPSERT(new hash)
message RECALL -> immediately make row ineligible; enqueue DELETE
hard DELETE    -> foreign-key cascade plus idempotent DELETE for an external store
room soft delete / membership removal -> no per-vector rewrite; query-time ACL blocks immediately
```

Workers claim small batches with `FOR UPDATE SKIP LOCKED`, which PostgreSQL
documents as suitable for multiple consumers of a queue-like table. Calls to
the embedding provider occur outside the claim transaction. Completion uses a
conditional upsert only if the source message is still unrecalled and its
current hash/version equals the job's desired version. Retries use exponential
backoff, a maximum-attempt/dead-letter state, and idempotent keys such as
`(message_id, chunk_index, embedding_profile)`.
[PostgreSQL locking clauses](https://www.postgresql.org/docs/current/sql-select.html#SQL-FOR-UPDATE-SHARE)

This is deliberately at-least-once. A database transaction cannot atomically
commit with an external embedding API. Idempotency and current-version checks
make duplicate or late completions harmless.

Database triggers are an option if messages can be changed by paths outside the
Rust service; PostgreSQL supports row-level triggers on insert/update/delete.
For this repository, explicit outbox writes in the owning message transaction
are easier to test and make domain intent clearer. A trigger can be added as a
defense against future out-of-band writers.
[PostgreSQL CREATE TRIGGER](https://www.postgresql.org/docs/current/sql-createtrigger.html)

### 3.5 Retrieval and permission filtering

Every pgvector search should enforce, in the same SQL statement or transaction:

```sql
WHERE memberships.user_id = $viewer_id
  AND memberships.status = 'active'
  AND rooms.deleted_at IS NULL
  AND messages.recalled_at IS NULL
  AND embedding.content_sha256 = <hash of the current indexed message version>
  AND <explicit allowed-room/password-unlock scope>
ORDER BY embedding.embedding <=> $query_vector
LIMIT $k
```

The exact implementation may avoid recomputing hashes by storing a current
message version and making stale embeddings ineligible transactionally. The
invariant matters: an edit or recall must block stale text synchronously even
if re-embedding/deletion is delayed.

Password-protected rooms need special treatment. Current AI endpoints require
`x-room-password` in addition to active membership. A cross-conversation RAG
query cannot safely weaken that to membership alone. Initial choices are:

1. Exclude password-protected rooms from global retrieval by default.
2. Search only explicitly selected rooms whose passwords were validated for
   this request.
3. Later issue short-lived server-side room-unlock capabilities and pass only
   their room IDs into retrieval.

Never store room passwords or password hashes in vector metadata.

PostgreSQL row-level security can add defense in depth, but the current service
uses a shared application connection pool. RLS is only effective if each
transaction sets a trustworthy viewer identity, policies join against active
membership, and the runtime role cannot bypass RLS. PostgreSQL notes that table
owners normally bypass policies unless `FORCE ROW LEVEL SECURITY` is used.
[PostgreSQL row security](https://www.postgresql.org/docs/current/ddl-rowsecurity.html)

Approximate indexes plus selective room filters require measurement. pgvector
documents that HNSW/IVFFlat filtering occurs after the approximate index scan
and can under-return; pgvector 0.8+ iterative scans can continue until enough
filtered results are found. Begin with exact search while the corpus is small,
then introduce HNSW, enable an appropriate iterative-scan mode, and monitor
recall against exact queries.

### 3.6 Migration and rollout

1. Confirm the production PostgreSQL service supports the pgvector extension;
   install/enable it in a separate migration. Keep SQLite development/tests on
   a no-op or test index implementation unless vector parity is explicitly
   required.
2. Choose and freeze an embedding profile: provider, exact model/revision,
   dimensions, distance metric, input normalization, chunking version, and
   privacy policy.
3. Add the profile-specific vector table and durable jobs/outbox table. Start
   dual-writing jobs for new insert/edit/recall events while retrieval remains
   disabled.
4. Capture a high-water mark and backfill existing unrecalled messages in
   stable `(created_at, id)` batches. Rate-limit and checkpoint each batch;
   ordinary chat writes must not wait for embedding.
5. Reconcile repeatedly: eligible source count vs indexed-current count,
   missing/stale rows, dead letters, provider latency, and age of the oldest
   pending job.
6. Build HNSW concurrently after the large backfill. Compare sampled HNSW
   results with exact search and tune only from measured recall/latency.
7. Run shadow retrieval first. Verify that every hit survives PostgreSQL
   authorization and current-version rehydration, including membership removal,
   recalls, edits, direct-chat deactivation, room soft deletion, and password
   protection.
8. Enable RAG for a small cohort with source citations back to message IDs and
   timestamps. Keep the bounded recent transcript as a fallback while
   retrieval quality is evaluated.

### 3.7 When Qdrant becomes justified

Qdrant is a reasonable second implementation when the vector workload needs
independent horizontal scaling, dedicated vector operations, richer payload
filtering, or operational isolation from the transactional database. Its
official Rust client supports create/upsert/query/delete operations, and Qdrant
supports indexed payload filters and tenant-marked payload fields.
[Qdrant Rust client](https://docs.rs/qdrant-client/latest/qdrant_client/)
[Qdrant filtering](https://qdrant.tech/documentation/search/filtering/)
[Qdrant multitenancy](https://qdrant.tech/documentation/manage-data/multitenancy/)

Trade-offs for this application:

| Concern | pgvector | Qdrant |
| --- | --- | --- |
| Source/vector consistency | Same PostgreSQL transaction for state + outbox; current-state joins are direct | Separate system; upsert/delete is eventually consistent with PostgreSQL |
| Authorization | Join current memberships/messages/rooms at query time | Payload filters help performance, but PostgreSQL must remain authoritative and results must be rehydrated/rechecked |
| Operations | Existing database, migrations, PITR, monitoring | Additional service, network, credentials, backup/snapshot and recovery procedure |
| Scale/isolation | Strong starting point; partition/replica options exist | Purpose-built vector scaling, sharding, payload indexes and tenant placement |
| Backend portability | SQLx + small `MessageIndex` implementation | Official `qdrant-client`; same application port if IDs and semantics are kept neutral |

If Qdrant is adopted, use stable deterministic point IDs, include `room_id`,
`message_id`, `chunk_index`, profile, and content version in payload, and create
payload indexes for mandatory filters. Do not put raw content in payload.
Over-fetch candidates, then rehydrate and authorize them in PostgreSQL; a
payload copy can be stale. Qdrant snapshots/backups must be added to disaster
recovery rather than assuming PostgreSQL backup covers the index.
[Qdrant snapshots](https://qdrant.tech/documentation/snapshots/)

## 4. Privacy and security requirements

Indexing every message through a hosted embedding API is a continuous external
transfer of private conversation content, unlike the current explicit
question-on-one-room flow. This requires a visible product policy and an admin
choice, not merely a backend dependency:

- default to no global indexing until the deployment has selected and disclosed
  an embedding provider and retention policy;
- support an entirely local embedding provider for private deployments;
- allow room-level exclusion and apply it before jobs are created;
- never embed credentials, room passwords, access tokens, attachment URLs,
  local paths, deleted or recalled content, or hidden attachment data;
- treat vectors, provider logs, job error payloads, and backups as sensitive
  derived conversation data; encrypt transport and storage and restrict
  operator access;
- keep provider errors bounded and scrubbed; do not log request bodies or
  embedding inputs;
- make account/room hard deletion produce a verifiable index purge, including
  dead-letter jobs and any external-store snapshots according to the declared
  retention policy;
- use retrieved chat text as untrusted data in the model prompt. Retrieval does
  not remove prompt-injection risk.

## 5. Acceptance tests and observability

Reasoning/streaming:

- exact payload tests per configured provider/model for Fast, Thinking, and
  ProviderDefault;
- an upstream fixture that emits only `reasoning_content` for several events
  before `content`; verify no buffering and no false "empty response";
- fragmented SSE frames, keep-alives, `[DONE]`, provider error frames, idle
  timeout, hard timeout, and browser cancellation;
- metrics for time to response headers, first upstream event, first reasoning
  event, first visible token, completion, timeout phase, model, and mode;
- never include API keys, authorization headers, prompts, or raw message text in
  metrics/logs.

Vector/RAG:

- insert, edit-during-embed, repeated edit, recall, hard delete, room soft
  delete, membership removal/restoration, and password-room access;
- duplicate jobs and worker crash after provider success but before database
  commit;
- backfill concurrent with live writes and a model-profile shadow migration;
- exact-search vs ANN recall fixtures, mandatory filter selectivity, and fewer
  than `k` authorized hits;
- authorization property: every returned message is independently readable by
  the viewer at response time;
- purge/reconciliation reports contain IDs and counts, not message text.

## Source snapshot

Primary sources used:

- SiliconFlow Chat Completions, reasoning, embeddings, and model-list API docs
- DeepSeek official API and thinking-mode guide
- QwenLM/Qwen3 official repository documentation
- `genai` 0.6.5 published API/source and the locally installed 0.6.5 crate source
- pgvector and pgvector-rust official repositories
- PostgreSQL official row-security, locking, and trigger documentation
- Swiftide official repository/API docs
- Qdrant official documentation and official Rust client docs
- BAAI's official BGE-M3 model card
