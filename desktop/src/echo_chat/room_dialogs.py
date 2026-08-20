from __future__ import annotations

from PySide6.QtCore import QSize
from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMessageBox,
    QPushButton,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from .models import Room


class CreateRoomDialog(QDialog):
    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("新建聊天室")
        self.setMinimumWidth(440)
        root = QVBoxLayout(self)
        root.setContentsMargins(22, 20, 22, 18)
        title = QLabel("新建聊天室", objectName="dialogTitle")
        root.addWidget(title)
        form = QFormLayout()
        form.setVerticalSpacing(13)
        self._name = QLineEdit(placeholderText="例如：产品讨论")
        self._name.setMaxLength(80)
        form.addRow("名称", self._name)
        self._emoji = QLineEdit(placeholderText="例如：💬")
        self._emoji.setMaxLength(8)
        form.addRow("头像", self._emoji)
        self._description = QTextEdit(placeholderText="简介（可选）")
        self._description.setMaximumHeight(76)
        form.addRow("简介", self._description)
        self._policy = QComboBox()
        self._policy.addItem("直接加入", "open")
        self._policy.addItem("需要审核", "approval")
        form.addRow("加入方式", self._policy)
        self._password = QLineEdit()
        self._password.setEchoMode(QLineEdit.EchoMode.Password)
        self._password.setPlaceholderText("留空表示无需密码")
        form.addRow("访问密码", self._password)
        root.addLayout(form)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Cancel | QDialogButtonBox.StandardButton.Ok
        )
        buttons.button(QDialogButtonBox.StandardButton.Ok).setText("创建")
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText("取消")
        buttons.accepted.connect(self._validate)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)
        self._name.setFocus()

    @property
    def values(self) -> tuple[str, str, str, str, str]:
        return (
            self._name.text().strip(),
            self._password.text(),
            str(self._policy.currentData()),
            self._emoji.text().strip(),
            self._description.toPlainText().strip(),
        )

    def _validate(self) -> None:
        if not self._name.text().strip():
            QMessageBox.warning(self, "无法创建", "请输入聊天室名称。")
            self._name.setFocus()
            return
        self.accept()


class DiscoverRoomsDialog(QDialog):
    def __init__(self, rooms: list[Room], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._rooms = rooms
        self._visible_rooms: list[Room] = []
        self.setWindowTitle("发现聊天室")
        self.resize(520, 540)
        root = QVBoxLayout(self)
        root.setContentsMargins(20, 18, 20, 18)
        heading = QHBoxLayout()
        heading.addWidget(QLabel("发现聊天室", objectName="dialogTitle"), 1)
        self._count = QLabel(objectName="muted")
        heading.addWidget(self._count)
        root.addLayout(heading)
        self._search = QLineEdit(placeholderText="搜索聊天室")
        self._search.setClearButtonEnabled(True)
        self._search.textChanged.connect(self._render)
        root.addWidget(self._search)
        self._list = QListWidget()
        self._list.setSpacing(3)
        self._list.itemDoubleClicked.connect(lambda _: self._choose())
        root.addWidget(self._list, 1)
        self._detail = QLabel(objectName="muted")
        self._detail.setWordWrap(True)
        self._list.currentRowChanged.connect(self._show_detail)
        root.addWidget(self._detail)
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Cancel)
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText("取消")
        self._open = QPushButton("加入")
        self._open.setProperty("primary", True)
        self._open.clicked.connect(self._choose)
        buttons.addButton(self._open, QDialogButtonBox.ButtonRole.AcceptRole)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)
        self._render()

    @property
    def selected_room(self) -> Room | None:
        row = self._list.currentRow()
        return self._visible_rooms[row] if 0 <= row < len(self._visible_rooms) else None

    def _render(self) -> None:
        needle = self._search.text().strip().casefold()
        self._visible_rooms = [
            room
            for room in self._rooms
            if not needle or needle in f"{room.name} {room.description}".casefold()
        ]
        self._list.clear()
        for room in self._visible_rooms:
            status = {
                "active": "已加入",
                "pending": "审核中",
                "invited": "已邀请",
            }.get(room.membership_status, "")
            lock = "  🔒" if room.has_password else ""
            suffix = f"  ·  {status}" if status else ""
            item = QListWidgetItem(f"{room.avatar_emoji or '💬'}  {room.name}{lock}{suffix}")
            item.setSizeHint(QSize(0, 48))
            self._list.addItem(item)
        self._count.setText(f"{len(self._visible_rooms)} 个")
        if self._visible_rooms:
            self._list.setCurrentRow(0)
        else:
            self._detail.setText("没有匹配的聊天室")
            self._open.setEnabled(False)

    def _show_detail(self, row: int) -> None:
        room = self._visible_rooms[row] if 0 <= row < len(self._visible_rooms) else None
        self._open.setEnabled(bool(room and room.membership_status != "pending"))
        if not room:
            self._detail.clear()
            return
        policy = "需要管理员审核" if room.join_policy == "approval" else "可直接加入"
        self._detail.setText("  ·  ".join(filter(None, (room.description, policy))))
        self._open.setText("打开" if room.can_open else "加入")

    def _choose(self) -> None:
        room = self.selected_room
        if room and room.membership_status != "pending":
            self.accept()
