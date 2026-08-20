from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

JsonObject = dict[str, Any]


@dataclass(frozen=True, slots=True)
class User:
    id: str
    username: str
    avatar_emoji: str = ""
    display_name: str = ""
    signature: str = ""
    homepage: str = ""
    relationship: str = "none"

    @property
    def name(self) -> str:
        return self.display_name or self.username

    @classmethod
    def from_dict(cls, value: JsonObject) -> User:
        return cls(
            id=str(value.get("id", "")),
            username=str(value.get("username", "")),
            avatar_emoji=str(value.get("avatar_emoji", "")),
            display_name=str(value.get("display_name", "")),
            signature=str(value.get("signature", "")),
            homepage=str(value.get("homepage", "")),
            relationship=str(value.get("relationship", "none")),
        )


@dataclass(frozen=True, slots=True)
class Room:
    id: str
    name: str
    has_password: bool = False
    join_policy: str = "open"
    avatar_emoji: str = ""
    description: str = ""
    membership_status: str = ""
    membership_role: str = ""

    @property
    def can_open(self) -> bool:
        return self.membership_status == "active"

    @classmethod
    def from_dict(cls, value: JsonObject) -> Room:
        return cls(
            id=str(value.get("id", "")),
            name=str(value.get("name", "")),
            has_password=bool(value.get("has_password", False)),
            join_policy=str(value.get("join_policy", "open")),
            avatar_emoji=str(value.get("avatar_emoji", "")),
            description=str(value.get("description", "")),
            membership_status=str(value.get("membership_status") or ""),
            membership_role=str(value.get("membership_role") or ""),
        )


@dataclass(frozen=True, slots=True)
class Conversation:
    room_id: str
    kind: str
    title: str
    avatar_emoji: str = ""
    description: str = ""
    unread_count: int = 0
    pending_join_requests: int = 0
    last_activity_at: str = ""
    last_message: JsonObject | None = None
    peer: User | None = None
    has_password: bool = False
    membership_role: str = "member"

    @property
    def preview(self) -> str:
        if self.pending_join_requests:
            return f"{self.pending_join_requests} 个入群申请待处理"
        if not self.last_message:
            return self.description or "暂无消息"
        if self.last_message.get("recalled"):
            return "消息已撤回"
        content = str(self.last_message.get("content", "")).strip()
        attachment = self.last_message.get("attachment_file_name")
        return content or (f"文件：{attachment}" if attachment else "暂无消息")

    @classmethod
    def from_dict(cls, value: JsonObject) -> Conversation:
        group = value.get("group") or {}
        peer_value = value.get("peer")
        return cls(
            room_id=str(value.get("room_id", "")),
            kind=str(value.get("kind", "group")),
            title=str(value.get("title", "")),
            avatar_emoji=str(value.get("avatar_emoji", "")),
            description=str(value.get("description", "")),
            unread_count=int(value.get("unread_count", 0) or 0),
            pending_join_requests=int(value.get("pending_join_requests", 0) or 0),
            last_activity_at=str(value.get("last_activity_at", "")),
            last_message=value.get("last_message"),
            peer=User.from_dict(peer_value) if isinstance(peer_value, dict) else None,
            has_password=bool(group.get("has_password", False)),
            membership_role=str(group.get("membership_role") or "member"),
        )


@dataclass(frozen=True, slots=True)
class Reaction:
    emoji: str
    user_ids: tuple[str, ...] = ()

    @classmethod
    def from_dict(cls, value: JsonObject) -> Reaction:
        return cls(str(value.get("emoji", "")), tuple(map(str, value.get("user_ids", []))))


@dataclass(frozen=True, slots=True)
class Message:
    message_id: str
    sender_id: str | None
    sender: str
    sender_avatar: str
    content: str
    timestamp: str
    attachment: JsonObject | None = None
    reply_to: JsonObject | None = None
    recalled_at: str | None = None
    edited_at: str | None = None
    forwarded_from: JsonObject | None = None
    reactions: tuple[Reaction, ...] = field(default_factory=tuple)

    @property
    def recalled(self) -> bool:
        return bool(self.recalled_at)

    @classmethod
    def from_dict(cls, value: JsonObject) -> Message:
        return cls(
            message_id=str(value.get("message_id") or value.get("id") or ""),
            sender_id=str(value["sender_id"]) if value.get("sender_id") else None,
            sender=str(value.get("sender", "")),
            sender_avatar=str(value.get("sender_avatar", "")),
            content=str(value.get("content", "")),
            timestamp=str(value.get("timestamp") or value.get("created_at") or ""),
            attachment=value.get("attachment"),
            reply_to=value.get("reply_to"),
            recalled_at=value.get("recalled_at"),
            edited_at=value.get("edited_at"),
            forwarded_from=value.get("forwarded_from"),
            reactions=tuple(Reaction.from_dict(item) for item in value.get("reactions", [])),
        )


@dataclass(frozen=True, slots=True)
class FriendRequest:
    user: User
    direction: str
    created_at: str

    @classmethod
    def from_dict(cls, value: JsonObject) -> FriendRequest:
        return cls(
            user=User.from_dict(value.get("user") or {}),
            direction=str(value.get("direction", "")),
            created_at=str(value.get("created_at", "")),
        )
