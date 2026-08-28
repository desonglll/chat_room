from __future__ import annotations

from .models import Conversation, FriendRequest, Room, User


class WorkspaceResponsesMixin:
    def _api_completed(self, operation: str, payload: object) -> None:
        if self._tool_api_completed(operation, payload):
            return
        if operation == "authenticate" and isinstance(payload, dict):
            self._complete_authentication(payload)
        elif operation == "conversations" and isinstance(payload, list):
            items = [Conversation.from_dict(item) for item in payload if isinstance(item, dict)]
            self._set_conversations(items)
        elif operation == "config" and isinstance(payload, dict):
            self._max_upload_bytes = int(payload.get("max_upload_bytes", self._max_upload_bytes))
            ai_enabled = bool(payload.get("ai_enabled", False))
            self._chat.set_ai_enabled(ai_enabled)
            self._sidebar.set_ai_enabled(ai_enabled)
        elif operation == "rooms" and isinstance(payload, list):
            rooms = [Room.from_dict(item) for item in payload if isinstance(item, dict)]
            self._show_room_directory(rooms)
        elif operation == "room-lookup" and isinstance(payload, dict):
            self._show_room_directory([Room.from_dict(payload)])
        elif operation == "create-room" and isinstance(payload, dict):
            self._room_created(Room.from_dict(payload))
        elif operation.startswith("join-room:") and isinstance(payload, dict):
            self._room_joined(operation.split(":", 1)[1], payload)
        elif operation == "friends" and isinstance(payload, list):
            self._friends = [User.from_dict(item) for item in payload if isinstance(item, dict)]
            self._update_contacts()
        elif operation == "requests:incoming" and isinstance(payload, list):
            self._incoming = [FriendRequest.from_dict(item) for item in payload if isinstance(item, dict)]
            self._update_contacts()
        elif operation == "requests:outgoing" and isinstance(payload, list):
            self._outgoing = [FriendRequest.from_dict(item) for item in payload if isinstance(item, dict)]
            self._update_contacts()
        elif operation == "blocks" and isinstance(payload, list):
            self._blocked = [User.from_dict(item) for item in payload if isinstance(item, dict)]
            self._update_contacts()
        elif operation == "user-search" and isinstance(payload, list):
            users = [User.from_dict(item) for item in payload if isinstance(item, dict)]
            self._contacts.show_search_results(users)
        elif operation == "start-direct" and isinstance(payload, dict):
            conversation = Conversation.from_dict(payload)
            self._conversations[conversation.room_id] = conversation
            self._render_conversations()
            self._sidebar.select_room(conversation.room_id)
        elif operation == "update-profile" and isinstance(payload, dict):
            self._current_user = User.from_dict(payload)
            self._sidebar.set_user(self._current_user)
            self.statusBar().showMessage("个人资料已保存", 2400)
        elif operation == "attachment-upload":
            self._chat.message_sent()
            self.statusBar().showMessage("附件已发送", 2400)
            self._api.conversations()
        elif operation == "attachment-download" and isinstance(payload, dict):
            self.statusBar().showMessage(f"附件已保存到 {payload.get('path', '')}", 5000)
        elif operation == "forward-messages" and isinstance(payload, list):
            succeeded = sum(not item.get("skipped_reason") for item in payload if isinstance(item, dict))
            self.statusBar().showMessage(f"已完成 {succeeded} 项转发", 3500)
            self._api.conversations()
        elif operation == "ai-suggestions" and isinstance(payload, dict):
            self._chat.show_ai_suggestions(payload)
        elif operation.startswith("room-members:") and isinstance(payload, list):
            if self._members_dialog:
                members = [item for item in payload if isinstance(item, dict)]
                self._members_dialog.set_members(members)
        elif operation.startswith(("member-action:", "invite-member:")):
            room_id = operation.split(":", 1)[1]
            self.statusBar().showMessage("成员状态已更新", 2400)
            self._api.room_members(room_id)
            self._api.conversations()
        elif operation.startswith("social:"):
            self.statusBar().showMessage("联系人状态已更新", 2400)
            self._refresh_contacts()
            self._api.conversations()
        elif operation == "logout":
            self._finish_logout()

    def _api_failed(self, operation: str, message: str, status: int) -> None:
        if self._tool_api_failed(operation, message):
            return
        if operation == "authenticate":
            self._login.set_loading(False)
            self._login.set_error(message)
            self._login.clear_password()
            return
        if operation == "user-search":
            self._contacts.show_notice(message)
        elif operation == "ai-suggestions":
            self._chat.show_ai_error()
            self.statusBar().showMessage(message, 5000)
        else:
            self.statusBar().showMessage(message, 5000)
        if operation == "create-room":
            self._pending_created_password = ""

    def _room_created(self, room: Room) -> None:
        if self._pending_created_password:
            self._room_passwords[room.id] = self._pending_created_password
        self._pending_created_password = ""
        self._pending_room_id = room.id
        self.statusBar().showMessage("聊天室已创建", 2400)
        self._api.conversations()

    def _room_joined(self, room_id: str, payload: dict[str, object]) -> None:
        if payload.get("status") == "active":
            self._pending_room_id = room_id
            self.statusBar().showMessage("已加入聊天室", 2400)
            self._api.conversations()
        else:
            self.statusBar().showMessage("加入申请已提交", 4000)
