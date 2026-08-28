from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QComboBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from .feature_models import SearchResult
from .models import Conversation


class SearchView(QWidget):
    close_requested = Signal()
    search_requested = Signal(str, str, str)
    result_requested = Signal(str, str)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="featurePage")
        self._results: list[SearchResult] = []
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(20, 16, 20, 20)
        header = QHBoxLayout()
        back = QPushButton("返回")
        back.clicked.connect(self.close_requested)
        header.addWidget(back)
        header.addWidget(QLabel("全局搜索", objectName="sectionTitle"), 1)
        root.addLayout(header)
        filters = QHBoxLayout()
        self._query = QLineEdit(placeholderText="搜索所有可见会话中的消息")
        self._query.setClearButtonEnabled(True)
        self._query.returnPressed.connect(self._submit)
        filters.addWidget(self._query, 1)
        self._room = QComboBox()
        self._room.setMinimumWidth(160)
        filters.addWidget(self._room)
        self._kind = QComboBox()
        for label, value in (
            ("全部", "all"),
            ("文字", "text"),
            ("文件", "file"),
            ("图片", "image"),
            ("视频", "video"),
            ("音频", "audio"),
        ):
            self._kind.addItem(label, value)
        filters.addWidget(self._kind)
        search = QPushButton("搜索")
        search.setProperty("primary", True)
        search.clicked.connect(self._submit)
        filters.addWidget(search)
        root.addLayout(filters)
        self._status = QLabel("输入关键词开始搜索", objectName="muted")
        root.addWidget(self._status)
        self._list = QListWidget(objectName="featureList")
        self._list.itemActivated.connect(self._open_item)
        root.addWidget(self._list, 1)

    def set_conversations(self, conversations: list[Conversation]) -> None:
        selected = self._room.currentData()
        self._room.clear()
        self._room.addItem("所有会话", "")
        for conversation in conversations:
            self._room.addItem(conversation.title, conversation.room_id)
        index = self._room.findData(selected)
        self._room.setCurrentIndex(max(index, 0))

    def set_loading(self) -> None:
        self._status.setText("正在搜索…")

    def show_results(self, payload: dict[str, object]) -> None:
        raw_items = payload.get("items", [])
        self._results = [SearchResult.from_dict(item) for item in raw_items if isinstance(item, dict)]
        self._list.clear()
        for index, result in enumerate(self._results):
            attachment = f" · {result.attachment_file_name}" if result.attachment_file_name else ""
            item = QListWidgetItem(
                f"{result.conversation_title} · {result.sender}{attachment}\n{result.excerpt}"
            )
            item.setData(Qt.ItemDataRole.UserRole, index)
            item.setToolTip("打开消息位置")
            self._list.addItem(item)
        self._status.setText(f"找到 {len(self._results)} 条消息" if self._results else "没有匹配消息")

    def show_error(self, message: str) -> None:
        self._status.setText(message)

    def focus_query(self) -> None:
        self._query.setFocus()

    def _submit(self) -> None:
        query = self._query.text().strip()
        if not query:
            self._status.setText("请输入关键词")
            return
        self.set_loading()
        self.search_requested.emit(query, str(self._room.currentData() or ""), str(self._kind.currentData()))

    def _open_item(self, item: QListWidgetItem) -> None:
        index = int(item.data(Qt.ItemDataRole.UserRole))
        result = self._results[index]
        self.result_requested.emit(result.room_id, result.message_id)
