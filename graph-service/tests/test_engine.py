from uuid import uuid4

import pytest
from pydantic import SecretStr

import room_graph.engine as engine_module
from room_graph.engine import GraphitiEngine
from room_graph.settings import Settings


@pytest.mark.asyncio
async def test_graphiti_uses_graph_llm_settings_for_the_reranker(monkeypatch):
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
    assert reranker.config.api_key == 'graph-llm-key'
    assert reranker.config.base_url == 'https://llm.example/v1'
    assert reranker.config.model == 'graph-model'
