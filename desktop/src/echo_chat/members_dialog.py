from __future__ import annotations

from PySide6.QtCore import Signal
from PySide6.QtWidgets import (
    QDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMenu,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from .models import JsonObject
from .ui_common import AvatarLabel, clear_layout


class MemberRow(QFrame):
    action_requested = Signal(str, str, str)

    def __init__(
        self,
        member: JsonObject,
        current_user_id: str,
        current_role: str,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setObjectName("contactRow")
        user_id = str(member.get("user_id", ""))
        username = str(member.get("username", ""))
        layout = QHBoxLayout(self)
        layout.setContentsMargins(10, 8, 10, 8)
        layout.addWidget(AvatarLabel(user_id, str(member.get("avatar_emoji", "")), username, 38))
        copy = QVBoxLayout()
        name = str(member.get("nickname") or username)
        copy.addWidget(QLabel(name))
        status = str(member.get("status", ""))
        role = str(member.get("role", "member"))
        detail = QLabel(member_label(status, role), objectName="muted")
        copy.addWidget(detail)
        layout.addLayout(copy, 1)
        if user_id == current_user_id:
            return
        if status == "pending":
            approve = QPushButton("通过")
            approve.setProperty("primary", True)
            approve.clicked.connect(lambda: self.action_requested.emit("approve", user_id, ""))
            layout.addWidget(approve)
            reject = QPushButton("拒绝")
            reject.clicked.connect(lambda: self.action_requested.emit("reject", user_id, ""))
            layout.addWidget(reject)
            return
        if status != "active" or current_role not in {"owner", "admin"}:
            return
        more = QPushButton("更多")
        menu = QMenu(more)
        if current_role == "owner" and role != "owner":
            target_role = "member" if role == "admin" else "admin"
            role_label = "设为成员" if target_role == "member" else "设为管理员"
            role_action = menu.addAction(role_label)
            role_action.triggered.connect(
                lambda: self.action_requested.emit("set_role", user_id, target_role)
            )
        remove = menu.addAction("移出聊天室")
        remove.triggered.connect(lambda: self.action_requested.emit("remove", user_id, ""))
        more.setMenu(menu)
        layout.addWidget(more)


class RoomMembersDialog(QDialog):
    action_requested = Signal(str, str, str)
    invite_requested = Signal(str)
    refresh_requested = Signal()

    def __init__(
        self,
        room_name: str,
        current_user_id: str,
        current_role: str,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._current_user_id = current_user_id
        self._current_role = current_role
        self.setWindowTitle(f"{room_name} · 成员")
        self.resize(540, 600)
        root = QVBoxLayout(self)
        root.setContentsMargins(18, 16, 18, 16)
        heading = QHBoxLayout()
        heading.addWidget(QLabel("聊天室成员", objectName="dialogTitle"), 1)
        self._count = QLabel(objectName="muted")
        heading.addWidget(self._count)
        root.addLayout(heading)
        invite = QHBoxLayout()
        self._username = QLineEdit(placeholderText="输入用户名邀请")
        self._username.returnPressed.connect(self._invite)
        invite.addWidget(self._username, 1)
        button = QPushButton("邀请")
        button.clicked.connect(self._invite)
        invite.addWidget(button)
        root.addLayout(invite)
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        host = QWidget()
        self._rows = QVBoxLayout(host)
        self._rows.setContentsMargins(0, 0, 0, 0)
        scroll.setWidget(host)
        root.addWidget(scroll, 1)

    def set_members(self, members: list[JsonObject]) -> None:
        clear_layout(self._rows)
        ordered = sorted(
            members,
            key=lambda item: (item.get("status") != "pending", item.get("username", "")),
        )
        for member in ordered:
            row = MemberRow(member, self._current_user_id, self._current_role)
            row.action_requested.connect(self.action_requested)
            self._rows.addWidget(row)
        self._rows.addStretch()
        pending = sum(item.get("status") == "pending" for item in members)
        self._count.setText(f"{len(members)} 人" + (f" · {pending} 条申请" if pending else ""))

    def show_notice(self, message: str) -> None:
        self.setWindowTitle(message)

    def _invite(self) -> None:
        username = self._username.text().strip()
        if username:
            self.invite_requested.emit(username)
            self._username.clear()


def member_label(status: str, role: str) -> str:
    if status == "pending":
        return "申请加入"
    if status == "invited":
        return "等待接受邀请"
    return {"owner": "所有者", "admin": "管理员", "member": "成员"}.get(role, role)
