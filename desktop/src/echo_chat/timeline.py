from __future__ import annotations

from dataclasses import dataclass, replace

from .models import JsonObject, Message, Reaction


@dataclass(frozen=True, slots=True)
class TimelineChange:
    kind: str
    message_id: str = ""
    message: Message | None = None


class MessageTimeline:
    """Owns deduplication and room-event reconciliation for one open conversation."""

    def __init__(self) -> None:
        self._messages: list[Message] = []
        self._indexes: dict[str, int] = {}

    @property
    def messages(self) -> tuple[Message, ...]:
        return tuple(self._messages)

    @property
    def latest_message_id(self) -> str:
        return self._messages[-1].message_id if self._messages else ""

    def clear(self) -> None:
        self._messages.clear()
        self._indexes.clear()

    def apply(self, event: JsonObject) -> TimelineChange:
        kind = str(event.get("type", ""))
        if kind == "broadcast":
            return self._upsert(Message.from_dict(event))
        message_id = str(event.get("message_id", ""))
        index = self._indexes.get(message_id)
        if index is None:
            return TimelineChange("ignored", message_id)
        current = self._messages[index]
        if kind == "message_edited":
            updated = replace(
                current,
                content=str(event.get("content", "")),
                edited_at=event.get("edited_at"),
            )
        elif kind == "message_recalled":
            updated = replace(current, content="", attachment=None, recalled_at=event.get("recalled_at"))
        elif kind == "reaction_changed":
            updated = self._apply_reaction(current, event)
        else:
            return TimelineChange("ignored", message_id)
        self._messages[index] = updated
        return TimelineChange("updated", message_id, updated)

    def _upsert(self, message: Message) -> TimelineChange:
        index = self._indexes.get(message.message_id)
        if index is None:
            self._indexes[message.message_id] = len(self._messages)
            self._messages.append(message)
            return TimelineChange("added", message.message_id, message)
        self._messages[index] = message
        return TimelineChange("updated", message.message_id, message)

    @staticmethod
    def _apply_reaction(message: Message, event: JsonObject) -> Message:
        emoji = str(event.get("emoji", ""))
        user_id = str(event.get("user_id", ""))
        active = bool(event.get("active"))
        reactions = list(message.reactions)
        position = next((index for index, item in enumerate(reactions) if item.emoji == emoji), None)
        if position is None and active:
            reactions.append(Reaction(emoji, (user_id,)))
        elif position is not None:
            users = list(reactions[position].user_ids)
            if active and user_id not in users:
                users.append(user_id)
            elif not active and user_id in users:
                users.remove(user_id)
            if users:
                reactions[position] = Reaction(emoji, tuple(users))
            else:
                reactions.pop(position)
        return replace(message, reactions=tuple(reactions))
