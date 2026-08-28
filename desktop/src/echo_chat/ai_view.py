from __future__ import annotations

from PySide6.QtCore import Signal
from PySide6.QtWidgets import (
    QComboBox,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QPushButton,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from .feature_models import AiThread, AiThreadMessage


class AiView(QWidget):
    close_requested = Signal()
    new_thread_requested = Signal()
    thread_requested = Signal(str)
    question_requested = Signal(str, str, object)

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="featurePage")
        self._threads: list[AiThread] = []
        self._room_id = ""
        self._message_ids: list[str] = []
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(20, 16, 20, 20)
        header = QHBoxLayout()
        back = QPushButton("返回")
        back.clicked.connect(self.close_requested)
        header.addWidget(back)
        header.addWidget(QLabel("AI 助手", objectName="sectionTitle"))
        self._thread = QComboBox()
        self._thread.setMinimumWidth(220)
        self._thread.currentIndexChanged.connect(self._thread_changed)
        header.addWidget(self._thread, 1)
        new_thread = QPushButton("新对话")
        new_thread.clicked.connect(self.new_thread_requested)
        header.addWidget(new_thread)
        root.addLayout(header)
        self._status = QLabel("", objectName="muted")
        root.addWidget(self._status)
        self._messages = QListWidget(objectName="featureList")
        self._messages.setWordWrap(True)
        root.addWidget(self._messages, 1)
        self._context = QLabel("", objectName="selectedContext")
        self._context.setWordWrap(True)
        self._context.hide()
        root.addWidget(self._context)
        composer = QHBoxLayout()
        self._question = QTextEdit(placeholderText="向 AI 提问")
        self._question.setFixedHeight(76)
        composer.addWidget(self._question, 1)
        ask = QPushButton("发送")
        ask.setProperty("primary", True)
        ask.setFixedHeight(44)
        ask.clicked.connect(self._submit)
        composer.addWidget(ask)
        root.addLayout(composer)

    @property
    def current_thread_id(self) -> str:
        return str(self._thread.currentData() or "")

    def set_threads(self, payload: list[object], preferred_id: str = "") -> None:
        self._threads = [AiThread.from_dict(item) for item in payload if isinstance(item, dict)]
        current = preferred_id or self.current_thread_id
        self._thread.blockSignals(True)
        self._thread.clear()
        for thread in self._threads:
            self._thread.addItem(thread.title, thread.id)
        index = self._thread.findData(current)
        self._thread.setCurrentIndex(index if index >= 0 else (0 if self._threads else -1))
        self._thread.blockSignals(False)
        if self.current_thread_id:
            self.thread_requested.emit(self.current_thread_id)
        else:
            self._messages.clear()
            self._status.setText("创建一个 AI 对话后开始提问")

    def show_messages(self, payload: list[object]) -> None:
        messages = [AiThreadMessage.from_dict(item) for item in payload if isinstance(item, dict)]
        self._messages.clear()
        for message in messages:
            role = "你" if message.role == "user" else "AI"
            source_lines = []
            for source in message.sources:
                source_lines.append(f"来源：{source.get('sender', '')} · {source.get('excerpt', '')}")
            suffix = "\n" + "\n".join(source_lines) if source_lines else ""
            item = QListWidgetItem(f"{role}\n{message.content or '正在生成…'}{suffix}")
            if message.status == "failed":
                item.setToolTip("AI 运行失败")
            self._messages.addItem(item)
        if self._messages.count():
            self._messages.scrollToBottom()
        self._status.setText("")

    def set_context(self, room_id: str, message_ids: list[str]) -> None:
        self._room_id = room_id
        self._message_ids = list(dict.fromkeys(message_ids))[:50]
        if self._message_ids:
            self._context.setText(f"将使用所选 {len(self._message_ids)} 条消息作为上下文")
            self._context.show()
        else:
            self._context.hide()

    def set_loading(self, message: str) -> None:
        self._status.setText(message)

    def show_error(self, message: str) -> None:
        self._status.setText(message)

    def focus_question(self) -> None:
        self._question.setFocus()

    def clear_question(self) -> None:
        self._question.clear()

    def _thread_changed(self) -> None:
        if self.current_thread_id:
            self.thread_requested.emit(self.current_thread_id)

    def _submit(self) -> None:
        question = self._question.toPlainText().strip()
        if not question:
            self._status.setText("请输入问题")
            return
        self.question_requested.emit(self.current_thread_id, question, list(self._message_ids))
        self.set_loading("AI 正在处理…")
