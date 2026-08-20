from __future__ import annotations

from PySide6.QtCore import QEvent, Qt, QTimer, Signal
from PySide6.QtGui import QKeyEvent
from PySide6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from .message_bubble import MessageBubble
from .models import Conversation, JsonObject, Message
from .timeline import MessageTimeline
from .ui_common import clear_layout


class ComposerEdit(QTextEdit):
    submit_requested = Signal()

    def keyPressEvent(self, event: QKeyEvent) -> None:
        if event.key() in {Qt.Key_Return, Qt.Key_Enter} and not event.modifiers() & Qt.ShiftModifier:
            self.submit_requested.emit()
            event.accept()
            return
        super().keyPressEvent(event)


class ChatView(QWidget):
    send_requested = Signal(str, str)
    edit_requested = Signal(str, str)
    recall_requested = Signal(str)
    reaction_requested = Signal(str, str, bool)
    read_candidate = Signal(str)
    typing_requested = Signal(str)
    attachment_requested = Signal(str, str)
    download_requested = Signal(object)
    forward_requested = Signal(str)
    ai_requested = Signal()
    manage_members_requested = Signal()

    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent, objectName="chatPage")
        self._conversation: Conversation | None = None
        self._current_user_id = ""
        self._timeline = MessageTimeline()
        self._bubbles: dict[str, MessageBubble] = {}
        self._reply_to: Message | None = None
        self._ai_enabled = False
        self._online = False
        self._typing_timer = QTimer(self)
        self._typing_timer.setSingleShot(True)
        self._typing_timer.setInterval(100)
        self._typing_timer.timeout.connect(lambda: self.typing_requested.emit(self._composer.toPlainText()))
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)
        root.setSpacing(0)
        top = QFrame(objectName="topBar")
        header = QHBoxLayout(top)
        header.setContentsMargins(18, 10, 18, 10)
        titles = QVBoxLayout()
        titles.setSpacing(2)
        self._title = QLabel("选择一个会话", objectName="pageTitle")
        self._status = QLabel("未连接", objectName="muted")
        titles.addWidget(self._title)
        titles.addWidget(self._status)
        header.addLayout(titles, 1)
        self._members_button = QPushButton("成员")
        self._members_button.setToolTip("管理聊天室成员")
        self._members_button.clicked.connect(self.manage_members_requested)
        self._members_button.hide()
        header.addWidget(self._members_button)
        self._ai_button = QPushButton("AI")
        self._ai_button.setToolTip("生成回复建议")
        self._ai_button.clicked.connect(self.ai_requested)
        header.addWidget(self._ai_button)
        root.addWidget(top)
        self._scroll = QScrollArea(widgetResizable=True)
        self._message_host = QWidget()
        self._messages = QVBoxLayout(self._message_host)
        self._messages.setContentsMargins(18, 18, 18, 14)
        self._messages.setSpacing(8)
        self._messages.addStretch()
        self._scroll.setWidget(self._message_host)
        root.addWidget(self._scroll, 1)
        composer_panel = QFrame(objectName="topBar")
        composer_layout = QVBoxLayout(composer_panel)
        composer_layout.setContentsMargins(16, 8, 16, 14)
        composer_layout.setSpacing(7)
        self._reply_bar = QFrame()
        reply_layout = QHBoxLayout(self._reply_bar)
        reply_layout.setContentsMargins(8, 4, 4, 4)
        self._reply_label = QLabel()
        reply_layout.addWidget(self._reply_label, 1)
        cancel_reply = QPushButton("取消")
        cancel_reply.clicked.connect(self._cancel_reply)
        reply_layout.addWidget(cancel_reply)
        self._reply_bar.hide()
        composer_layout.addWidget(self._reply_bar)
        self._ai_panel = QFrame()
        self._ai_panel.setObjectName("suggestionPanel")
        ai_layout = QVBoxLayout(self._ai_panel)
        ai_layout.setContentsMargins(9, 7, 9, 7)
        self._ai_summary = QLabel()
        self._ai_summary.setWordWrap(True)
        self._ai_summary.setObjectName("muted")
        ai_layout.addWidget(self._ai_summary)
        self._ai_choices = QHBoxLayout()
        self._ai_choices.setSpacing(6)
        ai_layout.addLayout(self._ai_choices)
        self._ai_panel.hide()
        composer_layout.addWidget(self._ai_panel)
        row = QHBoxLayout()
        row.setSpacing(8)
        self._attachment_button = QPushButton("📎")
        self._attachment_button.setToolTip("发送附件")
        self._attachment_button.setFixedSize(42, 42)
        self._attachment_button.clicked.connect(self._request_attachment)
        row.addWidget(self._attachment_button)
        self._composer = ComposerEdit(placeholderText="输入消息…")
        self._composer.setFixedHeight(66)
        self._composer.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self._composer.submit_requested.connect(self._submit)
        self._composer.textChanged.connect(self._typing_timer.start)
        row.addWidget(self._composer, 1)
        send = QPushButton("发送")
        send.setProperty("primary", True)
        send.setFixedHeight(44)
        send.clicked.connect(self._submit)
        row.addWidget(send)
        composer_layout.addLayout(row)
        root.addWidget(composer_panel)
        self.set_enabled(False)

    def set_current_user(self, user_id: str) -> None:
        self._current_user_id = user_id

    def set_conversation(self, conversation: Conversation) -> None:
        self._conversation = conversation
        self._timeline.clear()
        self._bubbles.clear()
        clear_layout(self._messages)
        self._messages.addStretch()
        self._title.setText(conversation.title)
        self._status.setText("正在连接…")
        can_manage = conversation.kind == "group" and conversation.membership_role in {
            "owner",
            "admin",
        }
        self._members_button.setVisible(can_manage)
        self._cancel_reply()
        self.set_enabled(False)

    def set_status(self, status: str) -> None:
        labels = {
            "idle": "未连接",
            "connecting": "正在连接…",
            "reconnecting": "连接中断，正在重连…",
            "offline": "离线",
            "online": "已连接",
        }
        failed = status.startswith("failed:")
        self._status.setText(status.split(":", 1)[1] if failed else labels.get(status, status))
        self._status.setStyleSheet("color:#b9362b;" if failed else "")
        self.set_enabled(status == "online")

    def set_enabled(self, enabled: bool) -> None:
        self._online = enabled
        self._composer.setEnabled(enabled)
        self._attachment_button.setEnabled(enabled)
        self._ai_button.setEnabled(enabled and self._ai_enabled)
        self._composer.setPlaceholderText("输入消息…" if enabled else "连接聊天室后可以发送消息")

    def set_ai_enabled(self, enabled: bool) -> None:
        self._ai_enabled = enabled
        self._ai_button.setVisible(enabled)
        self._ai_button.setEnabled(enabled and self._online)

    def set_ai_loading(self) -> None:
        self._ai_button.setEnabled(False)
        self._ai_button.setText("生成中…")

    def show_ai_suggestions(self, payload: JsonObject) -> None:
        self._ai_button.setText("AI")
        self._ai_button.setEnabled(self._online and self._ai_enabled)
        self._ai_summary.setText(str(payload.get("summary", "")))
        clear_layout(self._ai_choices)
        for suggestion in payload.get("suggestions", []):
            text = str(suggestion).strip()
            if not text:
                continue
            button = QPushButton(text)
            button.clicked.connect(lambda _, value=text: self._use_suggestion(value))
            self._ai_choices.addWidget(button)
        self._ai_choices.addStretch()
        self._ai_panel.show()

    def show_ai_error(self) -> None:
        self._ai_button.setText("AI")
        self._ai_button.setEnabled(self._online and self._ai_enabled)

    def apply_event(self, event: JsonObject) -> None:
        kind = event.get("type")
        if kind in {"broadcast", "message_edited", "message_recalled", "reaction_changed"}:
            change = self._timeline.apply(event)
            if change.kind == "added" and change.message:
                self._add_message(change.message)
                if change.message.sender_id != self._current_user_id:
                    self.read_candidate.emit(change.message.message_id)
            elif change.kind == "updated" and change.message:
                if bubble := self._bubbles.get(change.message_id):
                    bubble.update_message(change.message)
            return
        if kind == "system":
            self._add_system(str(event.get("content", "")))
        elif kind == "typing":
            user_id = str(event.get("user_id", ""))
            if user_id and user_id != self._current_user_id and event.get("content"):
                self._status.setText(f"{event.get('username', '对方')} 正在输入…")
            elif self._status.text().endswith("正在输入…"):
                self._status.setText("已连接")
        elif kind == "history_complete":
            self._scroll_bottom()
            if self._timeline.latest_message_id:
                self.read_candidate.emit(self._timeline.latest_message_id)

    def message_sent(self) -> None:
        self._composer.clear()
        self._cancel_reply()
        self._ai_panel.hide()

    def show_send_error(self) -> None:
        self._status.setText("消息未发送，请等待连接恢复")

    def _add_message(self, message: Message) -> None:
        bubble = MessageBubble(message, self._current_user_id)
        bubble.reaction_requested.connect(self.reaction_requested)
        bubble.reply_requested.connect(self._begin_reply)
        bubble.edit_requested.connect(self._edit_message)
        bubble.recall_requested.connect(self.recall_requested)
        bubble.forward_requested.connect(self.forward_requested)
        bubble.attachment_requested.connect(self.download_requested)
        row = QWidget()
        layout = QHBoxLayout(row)
        layout.setContentsMargins(0, 0, 0, 0)
        if message.sender_id == self._current_user_id:
            layout.addStretch()
            layout.addWidget(bubble)
        else:
            layout.addWidget(bubble)
            layout.addStretch()
        self._messages.insertWidget(self._messages.count() - 1, row)
        self._bubbles[message.message_id] = bubble
        self._scroll_bottom()

    def _add_system(self, content: str) -> None:
        label = QLabel(content, objectName="muted", alignment=Qt.AlignCenter)
        label.setWordWrap(True)
        self._messages.insertWidget(self._messages.count() - 1, label)

    def _begin_reply(self, message: Message) -> None:
        self._reply_to = message
        preview = message.content or (message.attachment or {}).get("file_name", "消息")
        self._reply_label.setText(f"回复 {message.sender}：{preview[:80]}")
        self._reply_bar.show()
        self._composer.setFocus()

    def _cancel_reply(self) -> None:
        self._reply_to = None
        self._reply_bar.hide()

    def _edit_message(self, message_id: str, current: str) -> None:
        content, accepted = QInputDialog.getMultiLineText(self, "编辑消息", "消息内容", current)
        if accepted and content.strip():
            self.edit_requested.emit(message_id, content.strip())

    def _submit(self) -> None:
        content = self._composer.toPlainText().strip()
        if content:
            self.send_requested.emit(content, self._reply_to.message_id if self._reply_to else "")

    def _request_attachment(self) -> None:
        reply_to = self._reply_to.message_id if self._reply_to else ""
        self.attachment_requested.emit(self._composer.toPlainText().strip(), reply_to)

    def _use_suggestion(self, suggestion: str) -> None:
        self._composer.setPlainText(suggestion)
        self._composer.setFocus()
        self._ai_panel.hide()

    def _scroll_bottom(self) -> None:
        QTimer.singleShot(
            0,
            lambda: self._scroll.verticalScrollBar().setValue(self._scroll.verticalScrollBar().maximum()),
        )

    def event(self, event: QEvent) -> bool:
        if event.type() == QEvent.Type.Show and self._timeline.latest_message_id:
            self.read_candidate.emit(self._timeline.latest_message_id)
        return super().event(event)
