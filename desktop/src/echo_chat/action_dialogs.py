from __future__ import annotations

from PySide6.QtCore import QSize, Qt
from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from .models import Conversation, User


class ForwardDialog(QDialog):
    def __init__(self, conversations: list[Conversation], parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("转发消息")
        self.resize(430, 500)
        root = QVBoxLayout(self)
        root.setContentsMargins(20, 18, 20, 18)
        root.addWidget(QLabel("转发消息", objectName="dialogTitle"))
        self._search = QLineEdit(placeholderText="搜索会话")
        self._search.setClearButtonEnabled(True)
        root.addWidget(self._search)
        self._list = QListWidget()
        self._list.setSpacing(2)
        root.addWidget(self._list, 1)
        self._checks: dict[str, QCheckBox] = {}
        for conversation in conversations:
            check = QCheckBox(conversation.title)
            check.setProperty("searchText", f"{conversation.title} {conversation.description}".casefold())
            item = QListWidgetItem()
            item.setData(Qt.ItemDataRole.UserRole, conversation.room_id)
            item.setSizeHint(QSize(check.sizeHint().width() + 18, 44))
            self._list.addItem(item)
            self._list.setItemWidget(item, check)
            self._checks[conversation.room_id] = check
        self._search.textChanged.connect(self._filter)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Cancel | QDialogButtonBox.StandardButton.Ok
        )
        buttons.button(QDialogButtonBox.StandardButton.Ok).setText("转发")
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText("取消")
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)

    @property
    def selected_room_ids(self) -> list[str]:
        return [room_id for room_id, check in self._checks.items() if check.isChecked()]

    def _filter(self, query: str) -> None:
        needle = query.strip().casefold()
        for index in range(self._list.count()):
            item = self._list.item(index)
            check = self._list.itemWidget(item)
            search_text = str(check.property("searchText")) if check else ""
            item.setHidden(bool(needle and needle not in search_text))


class ProfileDialog(QDialog):
    def __init__(self, user: User, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("个人资料")
        self.setMinimumWidth(460)
        root = QVBoxLayout(self)
        root.setContentsMargins(22, 20, 22, 18)
        root.addWidget(QLabel("个人资料", objectName="dialogTitle"))
        handle = QLabel(f"@{user.username}", objectName="muted")
        root.addWidget(handle)
        form = QFormLayout()
        form.setVerticalSpacing(13)
        self._emoji = QLineEdit(user.avatar_emoji)
        self._emoji.setMaxLength(8)
        self._emoji.setPlaceholderText("例如：🙂")
        form.addRow("头像", self._emoji)
        self._name = QLineEdit(user.display_name)
        self._name.setMaxLength(80)
        form.addRow("显示名称", self._name)
        self._signature = QTextEdit(user.signature)
        self._signature.setMaximumHeight(76)
        form.addRow("签名", self._signature)
        self._homepage = QLineEdit(user.homepage)
        self._homepage.setPlaceholderText("https://example.com")
        form.addRow("个人主页", self._homepage)
        root.addLayout(form)
        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Cancel | QDialogButtonBox.StandardButton.Save
        )
        buttons.button(QDialogButtonBox.StandardButton.Save).setText("保存")
        buttons.button(QDialogButtonBox.StandardButton.Cancel).setText("取消")
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)

    @property
    def values(self) -> dict[str, str]:
        return {
            "avatar_emoji": self._emoji.text().strip(),
            "display_name": self._name.text().strip(),
            "signature": self._signature.toPlainText().strip(),
            "homepage": self._homepage.text().strip(),
        }
