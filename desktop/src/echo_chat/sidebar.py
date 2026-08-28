from __future__ import annotations

from datetime import UTC, datetime, timedelta

from PySide6.QtCore import QSize, Qt, Signal
from PySide6.QtGui import QMouseEvent
from PySide6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMenu,
    QPushButton,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from .models import Conversation, User
from .ui_common import AvatarLabel, format_time


class ConversationRow(QFrame):
    preference_requested = Signal(str, object)

    def __init__(self, conversation: Conversation, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="conversationRow")
        self.conversation = conversation
        layout = QHBoxLayout(self)
        layout.setContentsMargins(8, 7, 8, 7)
        layout.setSpacing(10)
        layout.addWidget(AvatarLabel(conversation.room_id, conversation.avatar_emoji, conversation.title, 44))
        copy = QVBoxLayout()
        copy.setSpacing(3)
        heading = QHBoxLayout()
        title = QLabel(conversation.title)
        title.setStyleSheet("font-weight:650;")
        heading.addWidget(title, 1)
        if conversation.preferences.is_pinned:
            heading.addWidget(QLabel("置顶", objectName="muted"))
        if conversation.preferences.is_archived:
            heading.addWidget(QLabel("归档", objectName="muted"))
        time = QLabel(format_time(conversation.last_activity_at), objectName="muted")
        heading.addWidget(time)
        options = QToolButton()
        options.setText("⋯")
        options.setToolTip("会话设置")
        options.setPopupMode(QToolButton.ToolButtonPopupMode.InstantPopup)
        options.setMenu(self._preference_menu(options))
        heading.addWidget(options)
        copy.addLayout(heading)
        preview_row = QHBoxLayout()
        preview = QLabel(conversation.preview, objectName="muted")
        preview.setMaximumWidth(205)
        preview_row.addWidget(preview, 1)
        count = conversation.pending_join_requests or conversation.unread_count
        if count:
            badge = QLabel("99+" if count > 99 else str(count))
            badge.setObjectName("unreadBadge")
            badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
            badge.setFixedHeight(18)
            badge.setMinimumWidth(18)
            preview_row.addWidget(badge)
        copy.addLayout(preview_row)
        layout.addLayout(copy, 1)

    def _preference_menu(self, parent: QToolButton) -> QMenu:
        preferences = self.conversation.preferences
        menu = QMenu(parent)
        pin = menu.addAction("取消置顶" if preferences.is_pinned else "置顶会话")
        pin.triggered.connect(lambda: self._save({"is_pinned": not preferences.is_pinned}))
        archive = menu.addAction("移出归档" if preferences.is_archived else "归档会话")
        archive.triggered.connect(lambda: self._save({"is_archived": not preferences.is_archived}))
        notification = menu.addMenu("通知级别")
        for label, level in (("全部消息", "all"), ("仅提及", "mentions"), ("不通知", "none")):
            action = notification.addAction(label)
            action.setCheckable(True)
            action.setChecked(preferences.notification_level == level)
            action.triggered.connect(lambda _, value=level: self._save({"notification_level": value}))
        mute = menu.addMenu("静音")
        for label, hours in (("1 小时", 1), ("8 小时", 8), ("1 天", 24)):
            action = mute.addAction(label)
            action.triggered.connect(lambda _, value=hours: self._mute(value))
        clear = mute.addAction("取消静音")
        clear.setEnabled(bool(preferences.muted_until))
        clear.triggered.connect(lambda: self._save({"muted_until": None}))
        return menu

    def _mute(self, hours: int) -> None:
        until = (datetime.now(UTC) + timedelta(hours=hours)).isoformat().replace("+00:00", "Z")
        self._save({"muted_until": until})

    def _save(self, patch: dict[str, object]) -> None:
        self.preference_requested.emit(self.conversation.room_id, patch)

    def set_selected(self, selected: bool) -> None:
        self.setProperty("selected", selected)
        self.style().unpolish(self)
        self.style().polish(self)


class ConversationList(QListWidget):
    blank_clicked = Signal()

    def mousePressEvent(self, event: QMouseEvent) -> None:
        if self.itemAt(event.position().toPoint()) is None:
            self.clearSelection()
            self.setCurrentItem(None)
            self.blank_clicked.emit()
        super().mousePressEvent(event)


class Sidebar(QWidget):
    conversation_selected = Signal(str)
    selection_cleared = Signal()
    contacts_requested = Signal()
    create_room_requested = Signal()
    discover_rooms_requested = Signal()
    join_room_requested = Signal()
    profile_requested = Signal()
    logout_requested = Signal()
    search_requested = Signal()
    notifications_requested = Signal()
    favorites_requested = Signal()
    ai_requested = Signal()
    preference_requested = Signal(str, object)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="sidebar")
        self.setMinimumWidth(270)
        self.setMaximumWidth(390)
        self._conversations: list[Conversation] = []
        self._rows: dict[str, ConversationRow] = {}
        self._build()

    def _build(self) -> None:
        layout = QVBoxLayout(self)
        layout.setContentsMargins(10, 10, 10, 10)
        layout.setSpacing(9)
        header = QHBoxLayout()
        self._identity_slot = QHBoxLayout()
        header.addLayout(self._identity_slot, 1)
        create = QToolButton()
        create.setText("＋")
        create.setToolTip("新建")
        create.setFixedSize(36, 36)
        create.setPopupMode(QToolButton.ToolButtonPopupMode.InstantPopup)
        menu = QMenu(create)
        create_room = menu.addAction("新建聊天室")
        create_room.triggered.connect(self.create_room_requested)
        discover = menu.addAction("发现聊天室")
        discover.triggered.connect(self.discover_rooms_requested)
        join_room = menu.addAction("通过 ID 加入")
        join_room.triggered.connect(self.join_room_requested)
        menu.addSeparator()
        contacts = menu.addAction("联系人")
        contacts.triggered.connect(self.contacts_requested)
        create.setMenu(menu)
        header.addWidget(create)
        layout.addLayout(header)
        self._search = QLineEdit(placeholderText="搜索会话…")
        self._search.setClearButtonEnabled(True)
        self._search.textChanged.connect(self._filter)
        layout.addWidget(self._search)
        tools = QHBoxLayout()
        self._search_button = self._tool_button("搜索", "搜索所有消息", self.search_requested)
        tools.addWidget(self._search_button)
        self._notifications_button = self._tool_button("通知", "打开通知中心", self.notifications_requested)
        tools.addWidget(self._notifications_button)
        self._favorites_button = self._tool_button("收藏", "打开收藏", self.favorites_requested)
        tools.addWidget(self._favorites_button)
        self._ai_button = self._tool_button("AI", "打开 AI 助手", self.ai_requested)
        self._ai_button.hide()
        tools.addWidget(self._ai_button)
        layout.addLayout(tools)
        self._list = ConversationList()
        self._list.setSpacing(2)
        self._list.currentItemChanged.connect(self._selection_changed)
        self._list.itemClicked.connect(
            lambda item: self.conversation_selected.emit(str(item.data(Qt.UserRole)))
        )
        self._list.blank_clicked.connect(self.selection_cleared)
        layout.addWidget(self._list, 1)
        footer = QHBoxLayout()
        self._account_button = QPushButton()
        self._account_button.setToolTip("个人资料")
        self._account_button.clicked.connect(self.profile_requested)
        footer.addWidget(self._account_button, 1)
        logout = QPushButton("退出")
        logout.setToolTip("退出登录")
        logout.clicked.connect(self.logout_requested)
        footer.addWidget(logout)
        layout.addLayout(footer)

    def set_user(self, user: User) -> None:
        while self._identity_slot.count():
            item = self._identity_slot.takeAt(0)
            if item.widget():
                item.widget().deleteLater()
        self._identity_slot.addWidget(AvatarLabel(user.id, user.avatar_emoji, user.name, 36))
        label = QLabel("消息", objectName="brandTitle")
        self._identity_slot.addWidget(label)
        self._account_button.setText(user.name)

    def set_conversations(self, conversations: list[Conversation]) -> None:
        selected = self.current_room_id
        self._conversations = conversations
        self._list.clear()
        self._rows.clear()
        for conversation in conversations:
            item = QListWidgetItem()
            item.setData(Qt.UserRole, conversation.room_id)
            item.setSizeHint(QSize(0, 68))
            self._list.addItem(item)
            row = ConversationRow(conversation)
            row.preference_requested.connect(self.preference_requested)
            self._rows[conversation.room_id] = row
            self._list.setItemWidget(item, row)
            if conversation.room_id == selected:
                self._list.setCurrentItem(item)
        self._filter(self._search.text())

    def set_notification_count(self, count: int) -> None:
        self._notifications_button.setText("通知" if count <= 0 else f"通知 {min(count, 99)}")

    def set_ai_enabled(self, enabled: bool) -> None:
        self._ai_button.setVisible(enabled)

    @property
    def current_room_id(self) -> str:
        item = self._list.currentItem()
        return str(item.data(Qt.UserRole)) if item else ""

    def select_room(self, room_id: str) -> bool:
        for index in range(self._list.count()):
            item = self._list.item(index)
            if str(item.data(Qt.UserRole)) == room_id:
                self._list.setCurrentItem(item)
                self._list.scrollToItem(item)
                self.conversation_selected.emit(room_id)
                return True
        return False

    def clear_selection(self) -> None:
        self._list.clearSelection()
        self._list.setCurrentItem(None)

    def _filter(self, query: str) -> None:
        needle = query.strip().casefold()
        for index, conversation in enumerate(self._conversations):
            haystack = f"{conversation.title} {conversation.preview}".casefold()
            self._list.item(index).setHidden(bool(needle and needle not in haystack))

    def _selection_changed(self, current: QListWidgetItem | None, previous: QListWidgetItem | None) -> None:
        if previous:
            room_id = str(previous.data(Qt.UserRole))
            if row := self._rows.get(room_id):
                row.set_selected(False)
        if current:
            room_id = str(current.data(Qt.UserRole))
            if row := self._rows.get(room_id):
                row.set_selected(True)

    @staticmethod
    def _tool_button(label: str, tooltip: str, signal: Signal) -> QPushButton:
        button = QPushButton(label)
        button.setToolTip(tooltip)
        button.setFixedHeight(32)
        button.clicked.connect(signal)
        return button
