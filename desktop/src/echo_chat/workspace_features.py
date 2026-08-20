from __future__ import annotations

from PySide6.QtWidgets import QMessageBox

from .members_dialog import RoomMembersDialog
from .models import Conversation, JsonObject, Room
from .workspace_actions import (
    choose_download,
    choose_upload,
    create_room_values,
    discover_room,
    forward_targets,
    profile_values,
    room_identifier,
)


class WorkspaceFeaturesMixin:
    def _set_conversations(self, conversations: list[Conversation]) -> None:
        self._conversations = {item.room_id: item for item in conversations}
        self._render_conversations()
        if self._pending_room_id:
            room_id, self._pending_room_id = self._pending_room_id, ""
            self._sidebar.select_room(room_id)

    def _render_conversations(self) -> None:
        ordered = sorted(
            self._conversations.values(),
            key=lambda item: item.last_activity_at,
            reverse=True,
        )
        self._sidebar.set_conversations(ordered)

    def _refresh_contacts(self) -> None:
        self._api.friends()
        self._api.friend_requests("incoming")
        self._api.friend_requests("outgoing")
        self._api.blocks()

    def _update_contacts(self) -> None:
        self._contacts.set_data(self._friends, self._incoming, self._outgoing, self._blocked)

    def _contact_action(self, action: str, user_id: str) -> None:
        if action == "message":
            self._api.start_direct(user_id)
            return
        if action in {"remove", "block"}:
            label = "删除这位好友" if action == "remove" else "将这位用户加入黑名单"
            if QMessageBox.question(self, "确认操作", f"确定要{label}吗？") != QMessageBox.Yes:
                return
        self._api.social_action(action, user_id)

    def _create_room(self) -> None:
        values = create_room_values(self)
        if not values:
            return
        name, password, policy, emoji, description = values
        self._pending_created_password = password
        self._api.create_room(name, password, policy, emoji, description)

    def _show_room_directory(self, rooms: list[Room]) -> None:
        selected = discover_room(self, rooms)
        if not selected:
            return
        room, password = selected
        if room.can_open:
            if not self._sidebar.select_room(room.id):
                self._pending_room_id = room.id
                self._api.conversations()
            return
        if password:
            self._room_passwords[room.id] = password
        self._api.join_room(room.id, password)

    def _lookup_room(self) -> None:
        room_id = room_identifier(self)
        if room_id:
            self._api.get_room(room_id)

    def _edit_profile(self) -> None:
        if not self._current_user:
            return
        values = profile_values(self, self._current_user)
        if values:
            self._api.update_profile(values)

    def _upload_attachment(self, content: str, reply_to: str) -> None:
        if not self._active_room_id:
            return
        file_path = choose_upload(self, self._max_upload_bytes)
        if not file_path:
            return
        self.statusBar().showMessage("正在上传附件…")
        self._api.upload_attachment(
            self._active_room_id,
            file_path,
            self._active_room_password,
            content,
            reply_to,
        )

    def _download_attachment(self, attachment: JsonObject) -> None:
        selected = choose_download(self, attachment)
        if selected:
            self._api.download_attachment(*selected)

    def _forward_message(self, message_id: str) -> None:
        targets = forward_targets(self, list(self._conversations.values()))
        if targets:
            self._api.forward_messages([message_id], targets)

    def _request_ai_suggestions(self) -> None:
        if self._active_room_id:
            self._chat.set_ai_loading()
            self._api.ai_suggestions(self._active_room_id)

    def _manage_room_members(self) -> None:
        conversation = self._conversations.get(self._active_room_id)
        if not conversation or not self._current_user:
            return
        self._members_dialog = RoomMembersDialog(
            conversation.title,
            self._current_user.id,
            conversation.membership_role,
            self,
        )
        room_id = conversation.room_id
        self._members_dialog.action_requested.connect(
            lambda action, user_id, role: self._api.update_room_member(
                room_id,
                user_id,
                action,
                role,
            )
        )
        self._members_dialog.invite_requested.connect(
            lambda username: self._api.invite_room_member(room_id, username)
        )
        self._members_dialog.show()
        self._api.room_members(room_id)
