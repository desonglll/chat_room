from datetime import datetime
from uuid import UUID

from pydantic import BaseModel, Field


class EpisodeUpsert(BaseModel):
    room_id: UUID
    sender: str = Field(min_length=1, max_length=200)
    content: str = Field(min_length=1, max_length=20_000)
    created_at: datetime


class EpisodeResult(BaseModel):
    message_id: UUID
    status: str


class SearchRequest(BaseModel):
    room_id: UUID
    query: str = Field(min_length=1, max_length=4_000)
    limit: int = Field(default=8, ge=1, le=50)


class GraphNode(BaseModel):
    id: UUID
    name: str
    summary: str
    labels: list[str]


class GraphFact(BaseModel):
    id: UUID
    name: str
    fact: str
    source_node_id: UUID
    target_node_id: UUID
    episode_ids: list[UUID]
    valid_at: datetime | None = None
    invalid_at: datetime | None = None
    created_at: datetime
    expired_at: datetime | None = None


class SearchResponse(BaseModel):
    facts: list[GraphFact]


class GraphSnapshot(BaseModel):
    room_id: UUID
    nodes: list[GraphNode]
    facts: list[GraphFact]
    truncated: bool


class HealthResponse(BaseModel):
    status: str
