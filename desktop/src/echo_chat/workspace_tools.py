from __future__ import annotations

from uuid import uuid4

from PySide6.QtWidgets import QMessageBox

from .feature_models import NotificationItem


class WorkspaceToolsMixin:
    def _open_search(self) -> None:
        self._search_view.set_conversations(list(self._conversations.values()))
        self._content.setCurrentWidget(self._search_view)
        self._search_view.focus_query()

    def _run_search(self, query: str, room_id: str, content_type: str) -> None:
        self._api.search_messages(query, room_id, content_type)

    def _open_notifications(self) -> None:
        self._content.setCurrentWidget(self._notifications_view)
        self._notifications_view.set_loading()
        self._api.notifications(self._notifications_view.kind)

    def _open_favorites(self) -> None:
        self._content.setCurrentWidget(self._favorites_view)
        self._favorites_view.set_loading()
        self._api.favorites()

    def _open_ai(self) -> None:
        self._ai_context_room_id = ""
        self._ai_view.set_context("", [])
        self._show_ai()

    def _show_ai(self) -> None:
        self._content.setCurrentWidget(self._ai_view)
        self._api.ai_threads()
        self._ai_view.focus_question()

    def _open_ai_context(self, message_ids: list[str]) -> None:
        self._ai_context_room_id = self._active_room_id
        self._ai_view.set_context(self._active_room_id, message_ids)
        self._show_ai()

    def _save_conversation_preferences(self, room_id: str, patch: dict[str, object]) -> None:
        self._api.update_conversation_preferences(room_id, patch)

    def _open_notification(self, notification: NotificationItem) -> None:
        if not notification.read_at:
            self._api.mark_notification_read(notification.id)
        if notification.run_id:
            self._open_ai()
            self._active_ai_run_id = notification.run_id
            self._api.ai_run(notification.run_id)
        elif notification.kind == "friend_request":
            self._contacts.set_section("requests")
            self._content.setCurrentWidget(self._contacts)
            self._refresh_contacts()
        elif notification.source_available and notification.room_id:
            self._open_message(notification.room_id, notification.message_id)

    def _delete_favorite(self, favorite_id: str) -> None:
        if QMessageBox.question(self, "删除收藏", "确定删除这条收藏吗？") == QMessageBox.Yes:
            self._api.delete_favorite(favorite_id)

    def _favorite_message(self, message_id: str) -> None:
        self._api.favorite_messages([message_id])

    def _open_message(self, room_id: str, message_id: str) -> None:
        self._pending_message_id = message_id
        if room_id == self._active_room_id and self._content.currentWidget() is self._chat:
            if message_id:
                self._locate_or_load_message(room_id, message_id)
            return
        if not self._sidebar.select_room(room_id):
            self._pending_room_id = room_id
            self._api.conversations()

    def _load_message_context(self, room_id: str, message_id: str) -> None:
        self._api.message_context(room_id, message_id, self._room_passwords.get(room_id, ""))

    def _locate_or_load_message(self, room_id: str, message_id: str) -> None:
        if self._chat.scroll_to_message(message_id):
            self._pending_message_id = ""
        else:
            self._load_message_context(room_id, message_id)

    def _new_ai_thread(self) -> None:
        self._api.create_ai_thread(self._ai_context_room_id or self._active_room_id)

    def _ask_ai(self, thread_id: str, question: str, message_ids: list[str]) -> None:
        room_id = self._ai_context_room_id or self._active_room_id
        if not thread_id:
            self._pending_ai_question = (question, room_id, message_ids)
            self._api.create_ai_thread(room_id)
            return
        self._submit_ai_run(thread_id, question, room_id, message_ids)

    def _submit_ai_run(
        self,
        thread_id: str,
        question: str,
        room_id: str,
        message_ids: list[str],
    ) -> None:
        self._api.create_ai_run(
            thread_id,
            question,
            room_id,
            str(uuid4()),
            message_ids,
            self._room_passwords.get(room_id, ""),
        )

    def _poll_ai_run(self) -> None:
        if self._active_ai_run_id:
            self._api.ai_run(self._active_ai_run_id)

    def _tool_api_completed(self, operation: str, payload: object) -> bool:
        if operation == "global-search" and isinstance(payload, dict):
            self._search_view.show_results(payload)
        elif operation.startswith("message-context:") and isinstance(payload, list):
            target = operation.split(":", 1)[1]
            if self._chat.show_message_context(payload, target):
                self._pending_message_id = ""
            else:
                self.statusBar().showMessage("消息已不可用或无权查看", 5000)
        elif operation == "notification-list" and isinstance(payload, dict):
            self._notifications_view.show_items(payload)
        elif operation == "notification-count" and isinstance(payload, dict):
            self._sidebar.set_notification_count(int(payload.get("unread_count", 0) or 0))
        elif operation.startswith("notification-read"):
            self._api.notification_unread_count()
            if self._content.currentWidget() is self._notifications_view:
                self._api.notifications(self._notifications_view.kind)
        elif operation.startswith("conversation-preferences:"):
            self.statusBar().showMessage("会话设置已保存", 2400)
            self._api.conversations()
        elif operation == "favorite-list" and isinstance(payload, list):
            self._favorites_view.show_items(payload)
        elif operation.startswith("favorite-"):
            self.statusBar().showMessage("收藏已更新", 2400)
            self._api.favorites()
        elif operation == "ai-threads" and isinstance(payload, list):
            self._ai_view.set_threads(payload, self._preferred_ai_thread_id)
            self._preferred_ai_thread_id = ""
        elif operation == "ai-create-thread" and isinstance(payload, dict):
            thread_id = str(payload.get("id", ""))
            self._preferred_ai_thread_id = thread_id
            if self._pending_ai_question:
                question, room_id, message_ids = self._pending_ai_question
                self._pending_ai_question = None
                self._submit_ai_run(thread_id, question, room_id, message_ids)
            self._api.ai_threads()
        elif operation.startswith("ai-messages:") and isinstance(payload, list):
            self._ai_view.show_messages(payload)
        elif operation == "ai-create-run" and isinstance(payload, dict):
            self._active_ai_run_id = str(payload.get("id", ""))
            self._ai_view.clear_question()
            self._ai_poll_timer.start()
        elif operation.startswith("ai-run:") and isinstance(payload, dict):
            if str(payload.get("status", "")) in {"completed", "failed"}:
                self._ai_poll_timer.stop()
                self._active_ai_run_id = ""
                thread_id = str(payload.get("thread_id", ""))
                if thread_id:
                    self._api.ai_thread_messages(thread_id)
            else:
                self._ai_poll_timer.start()
        else:
            return False
        return True

    def _tool_api_failed(self, operation: str, message: str) -> bool:
        if operation == "global-search":
            self._search_view.show_error(message)
        elif operation.startswith("message-context:"):
            self.statusBar().showMessage(message, 5000)
        elif operation.startswith("notification-"):
            self._notifications_view.show_error(message)
        elif operation.startswith("favorite-"):
            self._favorites_view.show_error(message)
        elif operation.startswith("ai-") and operation != "ai-suggestions":
            self._ai_poll_timer.stop()
            self._ai_view.show_error(message)
        elif operation.startswith("conversation-preferences:"):
            self.statusBar().showMessage(message, 5000)
            self._api.conversations()
        else:
            return False
        return True
