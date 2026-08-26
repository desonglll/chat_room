from math import sqrt
from typing import Protocol

from graphiti_core.cross_encoder.client import CrossEncoderClient


class BatchEmbedder(Protocol):
    async def create_batch(self, input_data_list: list[str]) -> list[list[float]]: ...


class EmbeddingRerankerClient(CrossEncoderClient):
    def __init__(self, embedder: BatchEmbedder):
        self.embedder = embedder

    async def rank(self, query: str, passages: list[str]) -> list[tuple[str, float]]:
        if not passages:
            return []
        query_vector, *passage_vectors = await self.embedder.create_batch([query, *passages])
        ranked = [
            (passage, _cosine_similarity(query_vector, vector))
            for passage, vector in zip(passages, passage_vectors, strict=True)
        ]
        ranked.sort(key=lambda result: result[1], reverse=True)
        return ranked


def _cosine_similarity(left: list[float], right: list[float]) -> float:
    denominator = sqrt(sum(value * value for value in left)) * sqrt(
        sum(value * value for value in right)
    )
    if denominator == 0:
        return 0.0
    similarity = sum(a * b for a, b in zip(left, right, strict=True)) / denominator
    return min(1.0, max(0.0, similarity))
