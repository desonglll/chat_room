import asyncio
from collections import defaultdict
from contextlib import asynccontextmanager
from typing import Annotated
from uuid import UUID

from fastapi import Depends, FastAPI, Query, Request, Response, status
from fastapi.responses import JSONResponse

from .auth import require_bearer
from .engine import GraphEngine, GraphitiEngine
from .models import (
    EpisodeResult,
    EpisodeUpsert,
    GraphSnapshot,
    HealthResponse,
    SearchRequest,
    SearchResponse,
)
from .settings import Settings

Authorized = Annotated[None, Depends(require_bearer)]


class RoomOperations:
    def __init__(self, engine: GraphEngine, concurrency: int):
        self.engine = engine
        self.semaphore = asyncio.Semaphore(concurrency)
        self.locks: defaultdict[UUID, asyncio.Lock] = defaultdict(asyncio.Lock)

    @asynccontextmanager
    async def write(self, room_id: UUID):
        async with self.semaphore, self.locks[room_id]:
            yield


def create_app(settings: Settings, engine: GraphEngine | None = None) -> FastAPI:
    app = FastAPI(title='Chat Room Knowledge Graph', version='1.0.0')
    app.state.settings = settings
    app.state.operations = RoomOperations(
        engine or GraphitiEngine(settings), settings.max_concurrent_writes
    )

    @app.get('/healthz', response_model=HealthResponse)
    async def health(response: Response) -> HealthResponse:
        try:
            await app.state.operations.engine.health()
            return HealthResponse(status='ready')
        except Exception:
            response.status_code = status.HTTP_503_SERVICE_UNAVAILABLE
            return HealthResponse(status='unavailable')

    @app.put('/v1/episodes/{message_id}', response_model=EpisodeResult)
    async def upsert_episode(
        message_id: UUID, episode: EpisodeUpsert, _authorized: Authorized
    ) -> EpisodeResult:
        async with app.state.operations.write(episode.room_id):
            result = await app.state.operations.engine.upsert(message_id, episode)
        return EpisodeResult(message_id=message_id, status=result)

    @app.delete('/v1/episodes/{message_id}', response_model=EpisodeResult)
    async def delete_episode(
        message_id: UUID, room_id: UUID, _authorized: Authorized
    ) -> EpisodeResult:
        async with app.state.operations.write(room_id):
            result = await app.state.operations.engine.delete(room_id, message_id)
        return EpisodeResult(message_id=message_id, status=result)

    @app.post('/v1/search', response_model=SearchResponse)
    async def search(request: SearchRequest, _authorized: Authorized) -> SearchResponse:
        facts = await app.state.operations.engine.search(
            request.room_id, request.query, request.limit
        )
        return SearchResponse(facts=facts)

    @app.get('/v1/rooms/{room_id}/graph', response_model=GraphSnapshot)
    async def snapshot(
        room_id: UUID,
        _authorized: Authorized,
        limit: int = Query(default=settings.snapshot_limit, ge=1, le=1_000),
    ) -> GraphSnapshot:
        return await app.state.operations.engine.snapshot(room_id, limit)

    @app.exception_handler(ValueError)
    async def invalid_graph_data(_request: Request, _error: ValueError) -> JSONResponse:
        return JSONResponse(status_code=502, content={'detail': 'graph contains invalid data'})

    return app


def default_app() -> FastAPI:
    return create_app(Settings())  # pyright: ignore[reportCallIssue]
