from __future__ import annotations

from dataclasses import dataclass

from .models import JsonObject


@dataclass(frozen=True, slots=True)
class SearchResult:
    message_id: str
    room_id: str
    conversation_title: str
    sender: str
    excerpt: str
    content_type: str
    attachment_file_name: str
    context_before: str
    context_after: str
    created_at: str

    @classmethod
    def from_dict(cls, value: JsonObject) -> SearchResult:
        return cls(
            message_id=str(value.get("message_id", "")),
            room_id=str(value.get("room_id", "")),
            conversation_title=str(value.get("conversation_title", "")),
            sender=str(value.get("sender", "")),
            excerpt=str(value.get("excerpt", "")),
            content_type=str(value.get("content_type", "text")),
            attachment_file_name=str(value.get("attachment_file_name") or ""),
            context_before=str(value.get("context_before") or ""),
            context_after=str(value.get("context_after") or ""),
            created_at=str(value.get("created_at", "")),
        )


@dataclass(frozen=True, slots=True)
class NotificationItem:
    id: str
    kind: str
    summary: str
    room_id: str
    room_name: str
    message_id: str
    run_id: str
    source_available: bool
    created_at: str
    read_at: str

    @classmethod
    def from_dict(cls, value: JsonObject) -> NotificationItem:
        return cls(
            id=str(value.get("id", "")),
            kind=str(value.get("kind", "")),
            summary=str(value.get("summary", "")),
            room_id=str(value.get("room_id") or ""),
            room_name=str(value.get("room_name") or ""),
            message_id=str(value.get("message_id") or ""),
            run_id=str(value.get("run_id") or ""),
            source_available=bool(value.get("source_available", False)),
            created_at=str(value.get("created_at", "")),
            read_at=str(value.get("read_at") or ""),
        )


@dataclass(frozen=True, slots=True)
class FavoriteItem:
    id: str
    title: str
    content: str
    kind: str
    access: str
    version: int
    source_room_id: str
    source_message_id: str
    source_room_name: str
    source_sender: str
    updated_at: str

    @classmethod
    def from_dict(cls, value: JsonObject) -> FavoriteItem:
        return cls(
            id=str(value.get("id", "")),
            title=str(value.get("title", "")),
            content=str(value.get("content", "")),
            kind=str(value.get("kind", "manual")),
            access=str(value.get("access", "owner")),
            version=int(value.get("version", 1) or 1),
            source_room_id=str(value.get("source_room_id") or ""),
            source_message_id=str(value.get("source_message_id") or ""),
            source_room_name=str(value.get("source_room_name") or ""),
            source_sender=str(value.get("source_sender") or ""),
            updated_at=str(value.get("updated_at", "")),
        )


@dataclass(frozen=True, slots=True)
class AiThread:
    id: str
    title: str
    room_id: str
    updated_at: str

    @classmethod
    def from_dict(cls, value: JsonObject) -> AiThread:
        return cls(
            id=str(value.get("id", "")),
            title=str(value.get("title", "新对话")),
            room_id=str(value.get("room_id") or ""),
            updated_at=str(value.get("updated_at", "")),
        )


@dataclass(frozen=True, slots=True)
class AiThreadMessage:
    id: str
    role: str
    content: str
    status: str
    sources: tuple[JsonObject, ...]

    @classmethod
    def from_dict(cls, value: JsonObject) -> AiThreadMessage:
        sources = tuple(item for item in value.get("sources", []) if isinstance(item, dict))
        return cls(
            id=str(value.get("id", "")),
            role=str(value.get("role", "assistant")),
            content=str(value.get("content", "")),
            status=str(value.get("status", "completed")),
            sources=sources,
        )
