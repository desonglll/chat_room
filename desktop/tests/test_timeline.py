from echo_chat.timeline import MessageTimeline


def broadcast(message_id: str = "message-1") -> dict:
    return {
        "type": "broadcast",
        "message_id": message_id,
        "sender_id": "user-1",
        "sender": "mike",
        "sender_avatar": "",
        "content": "hello",
        "timestamp": "2026-08-20T10:00:00Z",
        "reactions": [],
    }


def test_timeline_deduplicates_authoritative_messages() -> None:
    timeline = MessageTimeline()

    assert timeline.apply(broadcast()).kind == "added"
    replacement = {**broadcast(), "content": "updated"}
    assert timeline.apply(replacement).kind == "updated"

    assert len(timeline.messages) == 1
    assert timeline.messages[0].content == "updated"


def test_timeline_applies_edit_recall_and_reactions() -> None:
    timeline = MessageTimeline()
    timeline.apply(broadcast())

    timeline.apply(
        {
            "type": "reaction_changed",
            "message_id": "message-1",
            "emoji": "👍",
            "user_id": "user-2",
            "active": True,
        }
    )
    assert timeline.messages[0].reactions[0].user_ids == ("user-2",)

    timeline.apply(
        {"type": "message_edited", "message_id": "message-1", "content": "edited", "edited_at": "now"}
    )
    assert timeline.messages[0].content == "edited"

    timeline.apply({"type": "message_recalled", "message_id": "message-1", "recalled_at": "later"})
    assert timeline.messages[0].recalled
    assert timeline.messages[0].content == ""
