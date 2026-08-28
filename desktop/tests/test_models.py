from echo_chat.models import Conversation, Message, Room, User


def test_conversation_parses_direct_room_and_preview() -> None:
    conversation = Conversation.from_dict(
        {
            "room_id": "room-1",
            "kind": "direct",
            "title": "Shinoda",
            "avatar_emoji": "",
            "peer": {"id": "user-2", "username": "shinoda", "display_name": "Shinoda"},
            "last_message": {"content": "hello", "recalled": False},
            "unread_count": 2,
            "preferences": {
                "room_id": "room-1",
                "is_pinned": True,
                "is_archived": False,
                "notification_level": "mentions",
            },
        }
    )

    assert conversation.peer == User("user-2", "shinoda", display_name="Shinoda")
    assert conversation.preview == "hello"
    assert conversation.unread_count == 2
    assert conversation.preferences.is_pinned
    assert conversation.preferences.notification_level == "mentions"


def test_message_accepts_websocket_and_history_shapes() -> None:
    live = Message.from_dict(
        {
            "message_id": "message-1",
            "sender_id": "user-1",
            "sender": "mike",
            "sender_avatar": "",
            "content": "hello",
            "timestamp": "2026-08-20T10:00:00Z",
            "reactions": [{"emoji": "👍", "user_ids": ["user-2"]}],
        }
    )
    stored = Message.from_dict(
        {
            "id": "message-2",
            "sender": "mike",
            "content": "history",
            "created_at": "2026-08-20T09:00:00Z",
        }
    )

    assert live.reactions[0].user_ids == ("user-2",)
    assert stored.message_id == "message-2"
    assert stored.timestamp.endswith("Z")


def test_room_and_profile_keep_desktop_workflow_fields() -> None:
    room = Room.from_dict(
        {
            "id": "room-1",
            "name": "Product",
            "has_password": True,
            "join_policy": "approval",
            "membership_status": "active",
            "membership_role": "owner",
        }
    )
    user = User.from_dict(
        {
            "id": "user-1",
            "username": "mike",
            "homepage": "https://example.com",
        }
    )

    assert room.can_open
    assert room.membership_role == "owner"
    assert user.homepage == "https://example.com"
