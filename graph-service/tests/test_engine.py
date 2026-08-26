import json
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from types import SimpleNamespace
from unittest.mock import AsyncMock
from uuid import uuid4

import httpx2 as httpx
import pytest
from graphiti_core.prompts import Message
from openai import AsyncOpenAI
from pydantic import BaseModel, SecretStr

import room_graph.engine as engine_module
from room_graph.engine import GraphitiEngine
from room_graph.models import EpisodeUpsert
from room_graph.settings import Settings


class ExtractionResult(BaseModel):
    entities: list[str]


@pytest.mark.asyncio
async def test_upsert_rebuilds_an_unmarked_partial_episode(monkeypatch):
    settings = Settings(
        api_token=SecretStr('test-token-at-least-16'),
        llm_api_key=SecretStr('graph-llm-key'),
    )
    engine = GraphitiEngine(settings)
    graphiti = SimpleNamespace(
        remove_episode=AsyncMock(),
        add_episode=AsyncMock(),
        driver=SimpleNamespace(execute_query=AsyncMock(return_value=([], None, None))),
    )

    @asynccontextmanager
    async def client(_room_id):
        yield graphiti

    monkeypatch.setattr(engine, '_client', client)
    monkeypatch.setattr(
        engine_module,
        '_episodes_for_message',
        AsyncMock(
            return_value=[{'uuid': 'partial-episode', 'content': 'Ada: Hello', 'indexed': False}]
        ),
    )
    message_id = uuid4()
    episode = EpisodeUpsert(
        room_id=uuid4(),
        sender='Ada',
        content='Hello',
        created_at=datetime(2026, 8, 26, tzinfo=UTC),
    )

    result = await engine.upsert(message_id, episode)

    assert result == 'indexed'
    graphiti.remove_episode.assert_awaited_once_with('partial-episode')
    graphiti.add_episode.assert_awaited_once()
    marker_query = graphiti.driver.execute_query.await_args
    assert 'chat_room_indexed' in marker_query.args[0]
    assert marker_query.kwargs['name'] == f'message:{message_id}'


@pytest.mark.asyncio
async def test_graph_llm_uses_chat_completions_for_openai_compatible_providers():
    requests = []

    def respond(request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content)
        requests.append((request.url.path, body))
        if request.url.path != '/v1/chat/completions':
            return httpx.Response(404, json={'error': {'message': 'Not Found'}})
        return httpx.Response(
            200,
            json={
                'id': 'chatcmpl-test',
                'object': 'chat.completion',
                'created': 0,
                'model': 'graph-model',
                'choices': [
                    {
                        'index': 0,
                        'message': {'role': 'assistant', 'content': '{"entities": []}'},
                        'finish_reason': 'stop',
                    }
                ],
                'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2},
            },
        )

    settings = Settings(
        api_token=SecretStr('test-token-at-least-16'),
        llm_api_key=SecretStr('graph-llm-key'),
        llm_base_url='https://llm.example/v1',
        llm_model='graph-model',
    )
    engine = GraphitiEngine(settings)
    http_client = httpx.AsyncClient(transport=httpx.MockTransport(respond))
    engine._llm.client = AsyncOpenAI(
        api_key='graph-llm-key',
        base_url='https://llm.example/v1',
        http_client=http_client,
    )

    try:
        result = await engine._llm._generate_response(
            [
                Message(role='system', content='Extract entities'),
                Message(role='user', content='Hi'),
            ],
            response_model=ExtractionResult,
        )
    finally:
        await engine._llm.client.close()

    assert result == {'entities': []}
    assert [path for path, _ in requests] == ['/v1/chat/completions']
    assert requests[0][1]['model'] == 'graph-model'
    assert requests[0][1]['temperature'] == 0
    assert requests[0][1]['response_format'] == {'type': 'json_object'}


@pytest.mark.asyncio
async def test_graphiti_uses_the_configured_embedding_reranker(monkeypatch):
    captured = {}

    class FakeGraphiti:
        def __init__(self, **kwargs):
            captured.update(kwargs)
            self.driver = object()

        async def build_indices_and_constraints(self):
            pass

        async def close(self):
            pass

    settings = Settings(
        api_token=SecretStr('test-token-at-least-16'),
        llm_api_key=SecretStr('graph-llm-key'),
        llm_base_url='https://llm.example/v1',
        llm_model='graph-model',
    )
    engine = GraphitiEngine(settings)
    monkeypatch.setattr(engine_module, 'Graphiti', FakeGraphiti)
    monkeypatch.setattr(engine, '_driver', lambda _room_id: object())

    async with engine._client(uuid4()):
        pass

    reranker = captured['cross_encoder']
    assert reranker.embedder is engine._embedder
    assert engine._embedder.config.api_key == 'graph-llm-key'
