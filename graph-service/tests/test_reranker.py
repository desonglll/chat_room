import pytest

from room_graph.reranker import EmbeddingRerankerClient


class FakeEmbedder:
    def __init__(self):
        self.calls = []

    async def create_batch(self, input_data_list: list[str]) -> list[list[float]]:
        self.calls.append(input_data_list)
        return [[1.0, 0.0], [0.8, 0.2], [0.0, 1.0]]


@pytest.mark.asyncio
async def test_embedding_reranker_orders_passages_by_cosine_similarity():
    embedder = FakeEmbedder()
    reranker = EmbeddingRerankerClient(embedder)

    results = await reranker.rank('query', ['related', 'unrelated'])

    assert embedder.calls == [['query', 'related', 'unrelated']]
    assert [passage for passage, _ in results] == ['related', 'unrelated']
    assert results[0][1] == pytest.approx(0.9701, abs=0.0001)
    assert results[1][1] == 0.0


@pytest.mark.asyncio
async def test_embedding_reranker_skips_empty_passage_lists():
    embedder = FakeEmbedder()
    reranker = EmbeddingRerankerClient(embedder)

    assert await reranker.rank('query', []) == []
    assert embedder.calls == []
