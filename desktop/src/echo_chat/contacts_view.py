from __future__ import annotations

from PySide6.QtCore import Qt, QTimer, Signal
from PySide6.QtGui import QAction
from PySide6.QtWidgets import (
    QButtonGroup,
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

from .models import FriendRequest, User
from .ui_common import AvatarLabel, clear_layout


class ContactRow(QFrame):
    action_requested = Signal(str, str)

    def __init__(self, user: User, kind: str, subtitle: str, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="contactRow")
        self._user = user
        layout = QHBoxLayout(self)
        layout.setContentsMargins(12, 10, 12, 10)
        layout.setSpacing(11)
        layout.addWidget(AvatarLabel(user.id, user.avatar_emoji, user.name, 44))
        copy = QVBoxLayout()
        copy.setSpacing(3)
        name = QLabel(user.name)
        name.setStyleSheet("font-weight:650;")
        copy.addWidget(name)
        detail = QLabel(subtitle or f"@{user.username}", objectName="muted")
        copy.addWidget(detail)
        layout.addLayout(copy, 1)
        self._add_actions(layout, kind)

    def _button(self, label: str, action: str, primary: bool = False) -> QPushButton:
        button = QPushButton(label)
        if primary:
            button.setProperty("primary", True)
        button.clicked.connect(lambda: self.action_requested.emit(action, self._user.id))
        return button

    def _add_actions(self, layout: QHBoxLayout, kind: str) -> None:
        if kind == "friend":
            layout.addWidget(self._button("发消息", "message", True))
            more = QPushButton("更多")
            menu = QMenu(more)
            remove = QAction("删除好友", menu)
            remove.triggered.connect(lambda: self.action_requested.emit("remove", self._user.id))
            menu.addAction(remove)
            block = QAction("加入黑名单", menu)
            block.triggered.connect(lambda: self.action_requested.emit("block", self._user.id))
            menu.addAction(block)
            more.setMenu(menu)
            layout.addWidget(more)
        elif kind == "incoming":
            layout.addWidget(self._button("接受", "accept", True))
            layout.addWidget(self._button("拒绝", "decline"))
        elif kind == "outgoing":
            layout.addWidget(self._button("撤回", "cancel"))
        elif kind == "blocked":
            layout.addWidget(self._button("解除拉黑", "unblock", True))
        elif kind == "none":
            layout.addWidget(self._button("添加", "request", True))
        elif kind == "search-friend":
            layout.addWidget(self._button("发消息", "message", True))
        else:
            text = {"incoming-search": "等待处理", "outgoing-search": "申请已发送"}.get(
                kind,
                "不可添加",
            )
            status = QLabel(text)
            status.setObjectName("muted")
            layout.addWidget(status)


class ContactsView(QWidget):
    close_requested = Signal()
    action_requested = Signal(str, str)
    search_requested = Signal(str)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="contactsPage")
        self._friends: list[User] = []
        self._incoming: list[FriendRequest] = []
        self._outgoing: list[FriendRequest] = []
        self._blocked: list[User] = []
        self._search_results: list[User] = []
        self._section = "friends"
        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.setInterval(350)
        self._search_timer.timeout.connect(self._run_search)
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)
        root.setSpacing(0)
        top = QFrame(objectName="topBar")
        top_layout = QHBoxLayout(top)
        top_layout.setContentsMargins(14, 10, 16, 10)
        back = QPushButton("←")
        back.setFixedSize(38, 36)
        back.setToolTip("返回会话")
        back.setAccessibleName("返回会话")
        back.clicked.connect(self.close_requested)
        top_layout.addWidget(back)
        top_layout.addWidget(QLabel("联系人", objectName="pageTitle"), 1)
        self._add_button = QPushButton("添加好友")
        self._add_button.setProperty("primary", True)
        self._add_button.clicked.connect(lambda: self.set_section("search"))
        top_layout.addWidget(self._add_button)
        root.addWidget(top)

        body = QWidget()
        body_layout = QVBoxLayout(body)
        body_layout.setContentsMargins(24, 18, 24, 20)
        body_layout.setSpacing(12)
        tabs = QHBoxLayout()
        tabs.setSpacing(4)
        group = QButtonGroup(self)
        group.setExclusive(True)
        self._tab_buttons: dict[str, QPushButton] = {}
        for section, label in (("friends", "全部好友"), ("requests", "申请"), ("blocked", "黑名单")):
            button = QPushButton(label, checkable=True, checked=section == "friends")
            button.setProperty("segment", True)
            button.clicked.connect(lambda _, value=section: self.set_section(value))
            group.addButton(button)
            self._tab_buttons[section] = button
            tabs.addWidget(button)
        tabs.addStretch()
        body_layout.addLayout(tabs)
        heading = QHBoxLayout()
        self._title = QLabel("全部好友", objectName="sectionTitle")
        heading.addWidget(self._title, 1)
        self._search = QLineEdit(placeholderText="搜索好友…")
        self._search.setClearButtonEnabled(True)
        self._search.setMaximumWidth(330)
        self._search.textChanged.connect(self._query_changed)
        heading.addWidget(self._search)
        body_layout.addLayout(heading)
        self._notice = QLabel(objectName="error", wordWrap=True)
        self._notice.hide()
        body_layout.addWidget(self._notice)
        scroll = QScrollArea(widgetResizable=True)
        self._content = QWidget()
        self._rows = QVBoxLayout(self._content)
        self._rows.setContentsMargins(0, 0, 0, 0)
        self._rows.setSpacing(0)
        self._rows.addStretch()
        scroll.setWidget(self._content)
        body_layout.addWidget(scroll, 1)
        root.addWidget(body, 1)

    def set_data(
        self,
        friends: list[User],
        incoming: list[FriendRequest],
        outgoing: list[FriendRequest],
        blocked: list[User],
    ) -> None:
        self._friends = friends
        self._incoming = incoming
        self._outgoing = outgoing
        self._blocked = blocked
        self._tab_buttons["requests"].setText(f"申请  {len(incoming)}" if incoming else "申请")
        if self._section != "search":
            self._render()

    def set_section(self, section: str) -> None:
        self._section = section
        self._notice.hide()
        if section in self._tab_buttons:
            self._tab_buttons[section].setChecked(True)
        else:
            for button in self._tab_buttons.values():
                button.setChecked(False)
        labels = {"friends": "全部好友", "requests": "好友申请", "blocked": "黑名单", "search": "添加好友"}
        self._title.setText(labels[section])
        self._search.blockSignals(True)
        self._search.clear()
        self._search.setPlaceholderText("输入至少 2 个字符…" if section == "search" else "搜索联系人…")
        self._search.blockSignals(False)
        self._add_button.setVisible(section != "search")
        self._render()
        if section == "search":
            self._search.setFocus()

    def show_search_results(self, users: list[User]) -> None:
        self._search_results = users
        if self._section == "search":
            self._render()

    def show_notice(self, message: str) -> None:
        self._notice.setText(message)
        self._notice.setVisible(bool(message))

    def _query_changed(self) -> None:
        self._search_timer.stop()
        if self._section == "search" and len(self._search.text().strip()) >= 2:
            self._search_timer.start()
        self._render()

    def _run_search(self) -> None:
        self.search_requested.emit(self._search.text().strip())

    def _render(self) -> None:
        clear_layout(self._rows)
        query = self._search.text().strip().casefold()
        rows: list[tuple[User, str, str]] = []
        if self._section == "friends":
            rows = [(user, "friend", user.signature) for user in self._friends]
        elif self._section == "blocked":
            rows = [(user, "blocked", user.signature) for user in self._blocked]
        elif self._section == "requests":
            rows = [(item.user, "incoming", "希望添加你为好友") for item in self._incoming]
            rows += [(item.user, "outgoing", "等待对方接受") for item in self._outgoing]
        elif len(query) >= 2:
            rows = [(user, self._search_kind(user), user.signature) for user in self._search_results]
        if query and self._section != "search":
            rows = [row for row in rows if query in f"{row[0].name} {row[0].username} {row[2]}".casefold()]
        for user, kind, subtitle in rows:
            row = ContactRow(user, kind, subtitle)
            row.action_requested.connect(self.action_requested)
            self._rows.addWidget(row)
        if not rows:
            needs_query = self._section == "search" and len(query) < 2
            message = "输入用户名开始搜索" if needs_query else "这里暂时没有内容"
            empty = QLabel(message, objectName="muted", alignment=Qt.AlignCenter)
            empty.setMinimumHeight(180)
            self._rows.addWidget(empty)
        self._rows.addStretch()

    @staticmethod
    def _search_kind(user: User) -> str:
        return {
            "friend": "search-friend",
            "incoming": "incoming-search",
            "outgoing": "outgoing-search",
            "blocked": "blocked-search",
        }.get(user.relationship, "none")
