from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from .feature_models import FavoriteItem


class FavoriteEditor(QDialog):
    def __init__(self, item: FavoriteItem | None = None, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("编辑收藏" if item else "新建收藏")
        self.resize(520, 340)
        root = QVBoxLayout(self)
        form = QFormLayout()
        self._title = QLineEdit(item.title if item else "")
        self._content = QTextEdit()
        self._content.setPlainText(item.content if item else "")
        form.addRow("标题", self._title)
        form.addRow("内容", self._content)
        root.addLayout(form)
        buttons = QDialogButtonBox(QDialogButtonBox.Save | QDialogButtonBox.Cancel)
        buttons.accepted.connect(self._save)
        buttons.rejected.connect(self.reject)
        root.addWidget(buttons)

    @property
    def values(self) -> tuple[str, str]:
        return self._title.text().strip(), self._content.toPlainText().strip()

    def _save(self) -> None:
        if any(self.values):
            self.accept()


class FavoritesView(QWidget):
    close_requested = Signal()
    refresh_requested = Signal()
    create_requested = Signal(str, str)
    update_requested = Signal(str, int, str, str)
    delete_requested = Signal(str)
    source_requested = Signal(str, str)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="featurePage")
        self._items: list[FavoriteItem] = []
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(20, 16, 20, 20)
        header = QHBoxLayout()
        back = QPushButton("返回")
        back.clicked.connect(self.close_requested)
        header.addWidget(back)
        header.addWidget(QLabel("收藏", objectName="sectionTitle"), 1)
        create = QPushButton("新建")
        create.setProperty("primary", True)
        create.clicked.connect(self._create)
        header.addWidget(create)
        root.addLayout(header)
        self._status = QLabel("", objectName="muted")
        root.addWidget(self._status)
        body = QHBoxLayout()
        self._list = QListWidget(objectName="featureList")
        self._list.currentItemChanged.connect(self._selection_changed)
        body.addWidget(self._list, 1)
        detail = QVBoxLayout()
        self._title = QLabel("选择一条收藏", objectName="pageTitle")
        detail.addWidget(self._title)
        self._meta = QLabel("", objectName="muted")
        detail.addWidget(self._meta)
        self._content = QLabel("", objectName="favoriteContent")
        self._content.setWordWrap(True)
        self._content.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop)
        self._content.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        detail.addWidget(self._content, 1)
        actions = QHBoxLayout()
        self._source = QPushButton("打开来源")
        self._source.clicked.connect(self._open_source)
        actions.addWidget(self._source)
        self._edit = QPushButton("编辑")
        self._edit.clicked.connect(self._edit_selected)
        actions.addWidget(self._edit)
        self._delete = QPushButton("删除")
        self._delete.setProperty("danger", True)
        self._delete.clicked.connect(self._delete_selected)
        actions.addWidget(self._delete)
        actions.addStretch()
        detail.addLayout(actions)
        body.addLayout(detail, 2)
        root.addLayout(body, 1)
        self._set_actions(False)

    def set_loading(self) -> None:
        self._status.setText("正在读取收藏…")

    def show_items(self, payload: list[object]) -> None:
        self._items = [FavoriteItem.from_dict(item) for item in payload if isinstance(item, dict)]
        self._list.clear()
        for index, favorite in enumerate(self._items):
            title = favorite.title or favorite.content[:40] or "未命名收藏"
            item = QListWidgetItem(f"{title}\n{favorite.source_room_name or '手动收藏'}")
            item.setData(Qt.ItemDataRole.UserRole, index)
            self._list.addItem(item)
        self._status.setText(f"{len(self._items)} 条收藏" if self._items else "暂无收藏")
        if self._items:
            self._list.setCurrentRow(0)
        else:
            self._clear_detail()

    def show_error(self, message: str) -> None:
        self._status.setText(message)

    def _selected(self) -> FavoriteItem | None:
        item = self._list.currentItem()
        return self._items[int(item.data(Qt.ItemDataRole.UserRole))] if item else None

    def _selection_changed(self) -> None:
        favorite = self._selected()
        if not favorite:
            self._clear_detail()
            return
        self._title.setText(favorite.title or "未命名收藏")
        source = "手动收藏"
        if favorite.source_room_name:
            source = f"{favorite.source_room_name} · {favorite.source_sender}"
        self._meta.setText(f"{source} · {favorite.access}")
        self._content.setText(favorite.content or "（无文字内容）")
        self._source.setVisible(bool(favorite.source_room_id and favorite.source_message_id))
        self._edit.setEnabled(favorite.access in {"owner", "editor"})
        self._delete.setEnabled(favorite.access == "owner")
        self._set_actions(True)

    def _create(self) -> None:
        editor = FavoriteEditor(parent=self)
        if editor.exec() == QDialog.DialogCode.Accepted:
            self.create_requested.emit(*editor.values)

    def _edit_selected(self) -> None:
        favorite = self._selected()
        if not favorite:
            return
        editor = FavoriteEditor(favorite, self)
        if editor.exec() == QDialog.DialogCode.Accepted:
            self.update_requested.emit(favorite.id, favorite.version, *editor.values)

    def _delete_selected(self) -> None:
        favorite = self._selected()
        if favorite:
            self.delete_requested.emit(favorite.id)

    def _open_source(self) -> None:
        favorite = self._selected()
        if favorite:
            self.source_requested.emit(favorite.source_room_id, favorite.source_message_id)

    def _clear_detail(self) -> None:
        self._title.setText("选择一条收藏")
        self._meta.clear()
        self._content.clear()
        self._set_actions(False)

    def _set_actions(self, visible: bool) -> None:
        self._source.setVisible(visible)
        self._edit.setVisible(visible)
        self._delete.setVisible(visible)
