from echo_chat.feature_models import AiThreadMessage, FavoriteItem, NotificationItem, SearchResult


def test_feature_models_normalize_optional_server_fields() -> None:
    search = SearchResult.from_dict(
        {
            "message_id": "message-1",
            "room_id": "room-1",
            "conversation_title": "Release",
            "sender": "mike",
            "excerpt": "ship it",
            "attachment_file_name": None,
            "context_before": None,
        }
    )
    notification = NotificationItem.from_dict(
        {
            "id": "notification-1",
            "kind": "mention",
            "summary": "mentioned you",
            "room_id": None,
            "source_available": False,
        }
    )
    favorite = FavoriteItem.from_dict({"id": "favorite-1", "version": 0, "content": "note"})
    ai_message = AiThreadMessage.from_dict(
        {
            "id": "assistant-1",
            "role": "assistant",
            "content": "answer",
            "sources": [{"message_id": "message-1", "excerpt": "evidence"}, None],
        }
    )

    assert search.attachment_file_name == ""
    assert search.context_before == ""
    assert notification.room_id == ""
    assert not notification.source_available
    assert favorite.version == 1
    assert ai_message.sources == ({"message_id": "message-1", "excerpt": "evidence"},)
