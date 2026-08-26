# Use Graphiti for isolated room knowledge graphs

The application will build one temporal graph per room with Graphiti 0.29.3 and FalkorDB 4.18.8, accessed through a small authenticated service owned by this repository. Graphiti fits changing conversations because it incrementally extracts entities and temporal facts with episode provenance; room-specific FalkorDB graph names prevent knowledge from different authorization boundaries from being merged.

## Considered Options

- Microsoft GraphRAG is optimized for batch corpus indexing and community reports rather than message-by-message conversation updates.
- LightRAG has broad storage support, but its API server does not yet provide request-scoped workspace isolation suitable for many private rooms.
- Neo4j GraphRAG provides mature graph retrieval primitives, but would require this project to design more of the temporal ingestion and episode lifecycle itself.
- Graphiti's example server acknowledges ingestion before an in-memory queue finishes. This project instead exposes a synchronous adapter so the existing SQL outbox only completes a job after Graphiti has committed it.

## Consequences

Graph updates are eventually consistent and consume LLM capacity. The service pins its dependencies, reconciles Graphiti's generated Episode UUIDs through deterministic message names, serializes writes within each room, requires a bearer token, and keeps all user authorization in the Rust application. FalkorDB is licensed under SSPLv1, so deployment owners must confirm that license is acceptable; replacing it later changes the graph persistence layer but not the application-facing service contract.
