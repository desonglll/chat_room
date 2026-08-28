from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QComboBox,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from .feature_models import NotificationItem


class NotificationsView(QWidget):
    close_requested = Signal()
    refresh_requested = Signal(str)
    read_all_requested = Signal()
    notification_requested = Signal(object)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="featurePage")
        self._items: list[NotificationItem] = []
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(20, 16, 20, 20)
        header = QHBoxLayout()
        back = QPushButton("返回")
        back.clicked.connect(self.close_requested)
        header.addWidget(back)
        header.addWidget(QLabel("通知中心", objectName="sectionTitle"), 1)
        self._kind = QComboBox()
        for label, value in (
            ("全部", ""),
            ("提及", "mention"),
            ("回复", "reply"),
            ("好友申请", "friend_request"),
            ("入群申请", "room_join_request"),
            ("AI 完成", "ai_run_completed"),
        ):
            self._kind.addItem(label, value)
        self._kind.currentIndexChanged.connect(self._refresh)
        header.addWidget(self._kind)
        read_all = QPushButton("全部已读")
        read_all.clicked.connect(self.read_all_requested)
        header.addWidget(read_all)
        root.addLayout(header)
        self._status = QLabel("", objectName="muted")
        root.addWidget(self._status)
        self._list = QListWidget(objectName="featureList")
        self._list.itemActivated.connect(self._open_item)
        root.addWidget(self._list, 1)

    @property
    def kind(self) -> str:
        return str(self._kind.currentData() or "")

    def set_loading(self) -> None:
        self._status.setText("正在读取通知…")

    def show_items(self, payload: dict[str, object]) -> None:
        raw_items = payload.get("items", [])
        self._items = [NotificationItem.from_dict(item) for item in raw_items if isinstance(item, dict)]
        self._list.clear()
        for index, notification in enumerate(self._items):
            marker = "" if notification.read_at else "未读 · "
            room = f" · {notification.room_name}" if notification.room_name else ""
            item = QListWidgetItem(f"{marker}{kind_label(notification.kind)}{room}\n{notification.summary}")
            item.setData(Qt.ItemDataRole.UserRole, index)
            if not notification.source_available:
                item.setToolTip("来源已失效；仍可标记已读")
            self._list.addItem(item)
        self._status.setText(f"{len(self._items)} 条通知" if self._items else "暂无通知")

    def show_error(self, message: str) -> None:
        self._status.setText(message)

    def _refresh(self) -> None:
        self.set_loading()
        self.refresh_requested.emit(self.kind)

    def _open_item(self, item: QListWidgetItem) -> None:
        self.notification_requested.emit(self._items[int(item.data(Qt.ItemDataRole.UserRole))])


def kind_label(kind: str) -> str:
    return {
        "mention": "提及",
        "reply": "回复",
        "friend_request": "好友申请",
        "room_join_request": "入群申请",
        "ai_run_completed": "AI 运行完成",
    }.get(kind, "通知")
