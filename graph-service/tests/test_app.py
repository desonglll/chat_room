from datetime import UTC, datetime
from uuid import UUID, uuid4

import pytest
from fastapi.testclient import TestClient
from pydantic import SecretStr

from room_graph.app import create_app
from room_graph.models import GraphFact, GraphSnapshot
from room_graph.settings import Settings


class FakeEngine:
    def __init__(self):
        self.calls = []
        self.fail_health = False

    async def health(self):
        if self.fail_health:
            raise ConnectionError('offline')

    async def upsert(self, message_id, episode):
        self.calls.append(('upsert', message_id, episode.room_id))
        return 'indexed'

    async def delete(self, room_id, message_id):
        self.calls.append(('delete', message_id, room_id))
        return 'absent'

    async def search(self, room_id, query, limit):
        self.calls.append(('search', room_id, query, limit))
        return [fact()]

    async def snapshot(self, room_id, limit):
        self.calls.append(('snapshot', room_id, limit))
        return GraphSnapshot(room_id=room_id, nodes=[], facts=[fact()], truncated=False)


@pytest.fixture
def service():
    engine = FakeEngine()
    settings = Settings(api_token=SecretStr('test-token-at-least-16'), llm_api_key=SecretStr('key'))
    return TestClient(create_app(settings, engine)), engine


def test_v1_routes_require_the_internal_bearer_token(service):
    client, _ = service
    room_id = uuid4()

    response = client.post(
        '/v1/search', json={'room_id': str(room_id), 'query': 'release', 'limit': 3}
    )

    assert response.status_code == 401
    assert response.headers['www-authenticate'] == 'Bearer'


def test_episode_lifecycle_uses_message_and_room_ids(service):
    client, engine = service
    room_id, message_id = uuid4(), uuid4()
    headers = {'Authorization': 'Bearer test-token-at-least-16'}
    payload = {
        'room_id': str(room_id),
        'sender': 'Ada',
        'content': 'The launch is Friday',
        'created_at': '2026-08-26T09:00:00Z',
    }

    put = client.put(f'/v1/episodes/{message_id}', headers=headers, json=payload)
    delete = client.delete(f'/v1/episodes/{message_id}?room_id={room_id}', headers=headers)

    assert put.status_code == 200
    assert put.json() == {'message_id': str(message_id), 'status': 'indexed'}
    assert delete.json() == {'message_id': str(message_id), 'status': 'absent'}
    assert engine.calls == [
        ('upsert', message_id, room_id),
        ('delete', message_id, room_id),
    ]


def test_search_and_snapshot_return_episode_provenance(service):
    client, engine = service
    room_id = uuid4()
    headers = {'Authorization': 'Bearer test-token-at-least-16'}

    search = client.post(
        '/v1/search',
        headers=headers,
        json={'room_id': str(room_id), 'query': 'deadline', 'limit': 5},
    )
    snapshot = client.get(f'/v1/rooms/{room_id}/graph?limit=20', headers=headers)

    assert search.status_code == 200
    assert search.json()['facts'][0]['episode_ids'] == ['aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa']
    assert snapshot.status_code == 200
    assert snapshot.json()['room_id'] == str(room_id)
    assert engine.calls[-2:] == [
        ('search', room_id, 'deadline', 5),
        ('snapshot', room_id, 20),
    ]


def test_health_reports_dependency_failure_without_details(service):
    client, engine = service
    engine.fail_health = True

    response = client.get('/healthz')

    assert response.status_code == 503
    assert response.json() == {'status': 'unavailable'}


def test_invalid_room_and_oversized_limits_are_rejected(service):
    client, _ = service
    headers = {'Authorization': 'Bearer test-token-at-least-16'}

    invalid_room = client.post(
        '/v1/search', headers=headers, json={'room_id': 'room-one', 'query': 'x'}
    )
    invalid_limit = client.get(f'/v1/rooms/{uuid4()}/graph?limit=1001', headers=headers)

    assert invalid_room.status_code == 422
    assert invalid_limit.status_code == 422


def fact() -> GraphFact:
    return GraphFact(
        id=UUID('bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'),
        name='scheduled_for',
        fact='Launch is scheduled for Friday',
        source_node_id=UUID('cccccccc-cccc-4ccc-8ccc-cccccccccccc'),
        target_node_id=UUID('dddddddd-dddd-4ddd-8ddd-dddddddddddd'),
        episode_ids=[UUID('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa')],
        created_at=datetime(2026, 8, 26, tzinfo=UTC),
    )
