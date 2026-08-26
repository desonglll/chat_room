from collections.abc import AsyncIterator
from contextlib import asynccontextmanager, suppress
from typing import Protocol
from uuid import UUID

from graphiti_core import Graphiti
from graphiti_core.cross_encoder.openai_reranker_client import OpenAIRerankerClient
from graphiti_core.driver.falkordb_driver import FalkorDriver
from graphiti_core.edges import EntityEdge
from graphiti_core.embedder import OpenAIEmbedder, OpenAIEmbedderConfig
from graphiti_core.errors import NodeNotFoundError
from graphiti_core.llm_client import LLMConfig, OpenAIClient
from graphiti_core.nodes import EntityNode, EpisodeType, EpisodicNode

from .models import EpisodeUpsert, GraphFact, GraphNode, GraphSnapshot
from .settings import Settings


class GraphEngine(Protocol):
    async def health(self) -> None: ...

    async def upsert(self, message_id: UUID, episode: EpisodeUpsert) -> str: ...

    async def delete(self, room_id: UUID, message_id: UUID) -> str: ...

    async def search(self, room_id: UUID, query: str, limit: int) -> list[GraphFact]: ...

    async def snapshot(self, room_id: UUID, limit: int) -> GraphSnapshot: ...


def room_graph_id(room_id: UUID) -> str:
    return f'room_{room_id.hex}'


class GraphitiEngine:
    def __init__(self, settings: Settings):
        self._settings = settings
        llm_config = LLMConfig(
            api_key=settings.llm_api_key.get_secret_value(),
            base_url=settings.llm_base_url,
            model=settings.llm_model,
            small_model=settings.llm_model,
            temperature=0,
        )
        self._llm = OpenAIClient(llm_config)
        self._cross_encoder = OpenAIRerankerClient(config=llm_config, client=self._llm)
        self._embedder = OpenAIEmbedder(
            OpenAIEmbedderConfig(
                api_key=settings.resolved_embedding_key,
                base_url=settings.embedding_base_url,
                embedding_model=settings.embedding_model,
                embedding_dim=settings.embedding_dimensions,
            )
        )

    def _driver(self, room_id: UUID) -> FalkorDriver:
        password = self._settings.falkordb_password
        return FalkorDriver(
            host=self._settings.falkordb_host,
            port=self._settings.falkordb_port,
            username=self._settings.falkordb_username,
            password=password.get_secret_value() if password else None,
            database=room_graph_id(room_id),
        )

    @asynccontextmanager
    async def _client(self, room_id: UUID) -> AsyncIterator[Graphiti]:
        graphiti = Graphiti(
            graph_driver=self._driver(room_id),
            llm_client=self._llm,
            embedder=self._embedder,
            cross_encoder=self._cross_encoder,
            store_raw_episode_content=True,
            max_coroutines=self._settings.max_concurrent_writes,
        )
        try:
            init_task = getattr(graphiti.driver, '_init_task', None)
            if init_task is not None:
                await init_task
            else:
                await graphiti.build_indices_and_constraints()
            yield graphiti
        finally:
            await graphiti.close()

    async def health(self) -> None:
        driver = self._driver(UUID(int=0))
        try:
            await driver.execute_query('RETURN 1 AS ok', routing_='r')
        finally:
            await driver.close()

    async def upsert(self, message_id: UUID, episode: EpisodeUpsert) -> str:
        body = f'{_actor(episode.sender)}: {episode.content}'
        async with self._client(episode.room_id) as graphiti:
            existing = await _episodes_for_message(graphiti, episode.room_id, message_id)
            if len(existing) == 1 and existing[0]['content'] == body:
                return 'unchanged'
            for stored in existing:
                await graphiti.remove_episode(stored['uuid'])
            await graphiti.add_episode(
                name=f'message:{message_id}',
                episode_body=body,
                source_description='chat room message',
                reference_time=episode.created_at,
                source=EpisodeType.message,
                group_id=room_graph_id(episode.room_id),
                update_communities=False,
            )
        return 'indexed'

    async def delete(self, room_id: UUID, message_id: UUID) -> str:
        async with self._client(room_id) as graphiti:
            existing = await _episodes_for_message(graphiti, room_id, message_id)
            if not existing:
                return 'absent'
            for stored in existing:
                with suppress(NodeNotFoundError):
                    await graphiti.remove_episode(stored['uuid'])
        return 'deleted'

    async def search(self, room_id: UUID, query: str, limit: int) -> list[GraphFact]:
        async with self._client(room_id) as graphiti:
            edges = await graphiti.search(
                query,
                group_ids=[room_graph_id(room_id)],
                num_results=limit,
            )
            message_ids = await _source_message_ids(graphiti, edges)
        return [_fact(edge, message_ids) for edge in edges]

    async def snapshot(self, room_id: UUID, limit: int) -> GraphSnapshot:
        graph_id = room_graph_id(room_id)
        async with self._client(room_id) as graphiti:
            nodes = await EntityNode.get_by_group_ids(graphiti.driver, [graph_id], limit=limit + 1)
            edges = await EntityEdge.get_by_group_ids(graphiti.driver, [graph_id], limit=limit + 1)
            message_ids = await _source_message_ids(graphiti, edges)
        truncated = len(nodes) > limit or len(edges) > limit
        return GraphSnapshot(
            room_id=room_id,
            nodes=[_node(node) for node in nodes[:limit]],
            facts=[_fact(edge, message_ids) for edge in edges[:limit]],
            truncated=truncated,
        )


def _actor(sender: str) -> str:
    return ' '.join(sender.split())[:120] or 'unknown'


def _node(node: EntityNode) -> GraphNode:
    return GraphNode(id=UUID(node.uuid), name=node.name, summary=node.summary, labels=node.labels)


async def _episodes_for_message(
    graphiti: Graphiti, room_id: UUID, message_id: UUID
) -> list[dict[str, str]]:
    records, _, _ = await graphiti.driver.execute_query(
        'MATCH (e:Episodic {name: $name, group_id: $group_id}) '
        'RETURN e.uuid AS uuid, e.content AS content ORDER BY e.created_at ASC',
        name=f'message:{message_id}',
        group_id=room_graph_id(room_id),
        routing_='r',
    )
    return [{'uuid': str(record['uuid']), 'content': str(record['content'])} for record in records]


async def _source_message_ids(graphiti: Graphiti, edges: list[EntityEdge]) -> dict[str, UUID]:
    episode_uuids = list(dict.fromkeys(value for edge in edges for value in edge.episodes))
    episodes = await EpisodicNode.get_by_uuids(graphiti.driver, episode_uuids)
    return {
        episode.uuid: message_id
        for episode in episodes
        if (message_id := _message_id(episode.name)) is not None
    }


def _message_id(name: str) -> UUID | None:
    prefix, separator, value = name.partition(':')
    if prefix != 'message' or not separator:
        return None
    try:
        return UUID(value)
    except ValueError:
        return None


def _fact(edge: EntityEdge, message_ids: dict[str, UUID]) -> GraphFact:
    return GraphFact(
        id=UUID(edge.uuid),
        name=edge.name,
        fact=edge.fact,
        source_node_id=UUID(edge.source_node_uuid),
        target_node_id=UUID(edge.target_node_uuid),
        episode_ids=[message_ids[value] for value in edge.episodes if value in message_ids],
        valid_at=edge.valid_at,
        invalid_at=edge.invalid_at,
        created_at=edge.created_at,
        expired_at=edge.expired_at,
    )
