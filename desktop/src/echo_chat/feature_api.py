from __future__ import annotations

from typing import Any
from urllib.parse import quote, urlencode


class FeatureApiMixin:
    """Frozen Desktop bindings for released server feature contracts."""

    def search_messages(
        self,
        query: str,
        room_id: str = "",
        content_type: str = "all",
        cursor: str = "",
    ) -> None:
        params: dict[str, str | int] = {"q": query.strip(), "limit": 50}
        if room_id:
            params["room_id"] = room_id
        if content_type != "all":
            params["content_type"] = content_type
        if cursor:
            params["cursor"] = cursor
        self.request_json("global-search", "GET", f"/api/messages/search?{urlencode(params)}")

    def message_context(
        self,
        room_id: str,
        message_id: str,
        room_password: str = "",
    ) -> None:
        room_path = quote(room_id, safe="")
        message_path = quote(message_id, safe="")
        self.request_json(
            f"message-context:{message_id}",
            "GET",
            f"/api/rooms/{room_path}/messages/{message_path}/context?limit=60",
            extra_headers={"x-room-password": room_password} if room_password else None,
        )

    def notifications(self, kind: str = "", cursor: str = "") -> None:
        params: dict[str, str | int] = {"limit": 50}
        if kind:
            params["kind"] = kind
        if cursor:
            params["cursor"] = cursor
        self.request_json("notification-list", "GET", f"/api/notifications?{urlencode(params)}")

    def notification_unread_count(self) -> None:
        self.request_json("notification-count", "GET", "/api/notifications/unread-count")

    def mark_notification_read(self, notification_id: str) -> None:
        path = quote(notification_id, safe="")
        self.request_json(f"notification-read:{notification_id}", "POST", f"/api/notifications/{path}/read")

    def mark_all_notifications_read(self) -> None:
        self.request_json("notification-read-all", "POST", "/api/notifications/read-all")

    def update_conversation_preferences(self, room_id: str, patch: dict[str, Any]) -> None:
        path = quote(room_id, safe="")
        self.request_json(
            f"conversation-preferences:{room_id}",
            "PATCH",
            f"/api/conversations/{path}/preferences",
            patch,
        )

    def favorites(self) -> None:
        self.request_json("favorite-list", "GET", "/api/favorites")

    def create_favorite(self, title: str, content: str) -> None:
        self.request_json("favorite-create", "POST", "/api/favorites", {"title": title, "content": content})

    def update_favorite(self, favorite_id: str, version: int, title: str, content: str) -> None:
        path = quote(favorite_id, safe="")
        self.request_json(
            f"favorite-update:{favorite_id}",
            "PUT",
            f"/api/favorites/{path}",
            {"version": version, "title": title, "content": content},
        )

    def delete_favorite(self, favorite_id: str) -> None:
        path = quote(favorite_id, safe="")
        self.request_json(f"favorite-delete:{favorite_id}", "DELETE", f"/api/favorites/{path}")

    def favorite_messages(self, message_ids: list[str]) -> None:
        self.request_json(
            "favorite-messages", "POST", "/api/favorites/messages", {"message_ids": message_ids}
        )

    def ai_threads(self) -> None:
        self.request_json("ai-threads", "GET", "/api/ai/threads")

    def create_ai_thread(self, room_id: str = "") -> None:
        payload = {"room_id": room_id} if room_id else {}
        self.request_json("ai-create-thread", "POST", "/api/ai/threads", payload)

    def ai_thread_messages(self, thread_id: str) -> None:
        path = quote(thread_id, safe="")
        self.request_json(f"ai-messages:{thread_id}", "GET", f"/api/ai/threads/{path}/messages")

    def create_ai_run(
        self,
        thread_id: str,
        question: str,
        room_id: str,
        client_request_id: str,
        message_ids: list[str],
        room_password: str = "",
    ) -> None:
        path = quote(thread_id, safe="")
        self.request_json(
            "ai-create-run",
            "POST",
            f"/api/ai/threads/{path}/runs",
            {
                "question": question,
                "room_id": room_id or None,
                "client_request_id": client_request_id,
                "model_option_id": None,
                "message_ids": message_ids,
            },
            extra_headers={"x-room-password": room_password} if room_password else None,
        )

    def ai_run(self, run_id: str) -> None:
        path = quote(run_id, safe="")
        self.request_json(f"ai-run:{run_id}", "GET", f"/api/ai/runs/{path}")
