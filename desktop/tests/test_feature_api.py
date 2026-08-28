from typing import Any

from echo_chat.feature_api import FeatureApiMixin


class FakeJsonAdapter(FeatureApiMixin):
    def __init__(self) -> None:
        self.requests: list[tuple[str, str, str, dict[str, Any] | None, bool, dict[str, str] | None]] = []

    def request_json(
        self,
        operation: str,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
        authenticated: bool = True,
        extra_headers: dict[str, str] | None = None,
    ) -> None:
        self.requests.append((operation, method, path, payload, authenticated, extra_headers))


def test_search_notifications_and_preferences_use_frozen_contracts() -> None:
    api = FakeJsonAdapter()

    api.search_messages(" release plan ", "room/one", "file", "cursor value")
    api.notifications("mention", "next cursor")
    api.update_conversation_preferences("room/one", {"is_pinned": True, "muted_until": None})
    api.message_context("room/one", "message/one", "room-password")

    assert api.requests[0][1:3] == (
        "GET",
        "/api/messages/search?q=release+plan&limit=50&room_id=room%2Fone&content_type=file&cursor=cursor+value",
    )
    assert api.requests[1][2] == "/api/notifications?limit=50&kind=mention&cursor=next+cursor"
    assert api.requests[2][1:4] == (
        "PATCH",
        "/api/conversations/room%2Fone/preferences",
        {"is_pinned": True, "muted_until": None},
    )
    assert api.requests[3][1:3] == (
        "GET",
        "/api/rooms/room%2Fone/messages/message%2Fone/context?limit=60",
    )
    assert api.requests[3][5] == {"x-room-password": "room-password"}


def test_favorites_and_selected_message_ai_payloads_are_server_owned() -> None:
    api = FakeJsonAdapter()

    api.favorite_messages(["message-1", "message-2"])
    api.create_ai_run(
        "thread/one",
        "What changed?",
        "room-1",
        "request-1",
        ["message-1", "message-2"],
        "room-password",
    )

    assert api.requests[0][1:4] == (
        "POST",
        "/api/favorites/messages",
        {"message_ids": ["message-1", "message-2"]},
    )
    assert api.requests[1][1:4] == (
        "POST",
        "/api/ai/threads/thread%2Fone/runs",
        {
            "question": "What changed?",
            "room_id": "room-1",
            "client_request_id": "request-1",
            "model_option_id": None,
            "message_ids": ["message-1", "message-2"],
        },
    )
    assert api.requests[1][5] == {"x-room-password": "room-password"}
