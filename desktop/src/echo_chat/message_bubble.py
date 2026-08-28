from __future__ import annotations

from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QMenu, QPushButton, QVBoxLayout, QWidget

from .models import Message
from .ui_common import clear_layout, format_time

REACTION_CHOICES = ("👍", "❤️", "😂", "😮", "😢", "👏")


class MessageBubble(QFrame):
    reaction_requested = Signal(str, str, bool)
    reply_requested = Signal(object)
    edit_requested = Signal(str, str)
    recall_requested = Signal(str)
    forward_requested = Signal(str)
    attachment_requested = Signal(object)
    favorite_requested = Signal(str)
    context_toggle_requested = Signal(str)

    def __init__(
        self,
        message: Message,
        current_user_id: str,
        parent: QWidget | None = None,
    ) -> None:
        own = bool(message.sender_id and message.sender_id == current_user_id)
        super().__init__(parent)
        self.setObjectName("outgoingBubble" if own else "incomingBubble")
        self._message = message
        self._current_user_id = current_user_id
        self._own = own
        self._context_selected = False
        self.setMaximumWidth(620)
        self.setMinimumWidth(180)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.CustomContextMenu)
        self.customContextMenuRequested.connect(self._context_menu)
        self._layout = QVBoxLayout(self)
        self._layout.setContentsMargins(11, 8, 11, 8)
        self._layout.setSpacing(5)
        self._render()

    def update_message(self, message: Message) -> None:
        self._message = message
        self._render()

    def set_context_selected(self, selected: bool) -> None:
        self._context_selected = selected
        self.setProperty("contextSelected", selected)
        self.style().unpolish(self)
        self.style().polish(self)

    def _render(self) -> None:
        clear_layout(self._layout)
        message = self._message
        if not self._own:
            sender = QLabel(message.sender)
            sender.setStyleSheet("font-weight:650;color:#087f6b;")
            self._layout.addWidget(sender)
        self._add_forward_source(message)
        self._add_reply_preview(message)
        self._add_content(message)
        self._add_attachment(message)
        self._add_reactions(message)
        meta = (
            f"已编辑  {format_time(message.timestamp)}"
            if message.edited_at
            else format_time(message.timestamp)
        )
        stamp = QLabel(meta)
        stamp.setObjectName("muted")
        stamp.setAlignment(Qt.AlignmentFlag.AlignRight)
        self._layout.addWidget(stamp)

    def _add_forward_source(self, message: Message) -> None:
        if not message.forwarded_from:
            return
        source = message.forwarded_from
        forwarded = QLabel(f"转发自 {source.get('sender', '')} · {source.get('room_name', '')}")
        forwarded.setObjectName("muted")
        self._layout.addWidget(forwarded)

    def _add_reply_preview(self, message: Message) -> None:
        if not message.reply_to:
            return
        reply = message.reply_to
        preview = str(reply.get("content") or reply.get("attachment_file_name") or "消息已撤回")
        label = QLabel(f"回复 {reply.get('sender', '')}\n{preview}")
        label.setWordWrap(True)
        label.setStyleSheet("border-left:3px solid #087f6b;padding-left:7px;color:#52615d;")
        self._layout.addWidget(label)

    def _add_content(self, message: Message) -> None:
        label = QLabel("消息已撤回" if message.recalled else message.content)
        label.setWordWrap(True)
        if message.recalled:
            label.setStyleSheet("color:#697d78;font-style:italic;")
        else:
            label.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        if message.content or message.recalled:
            self._layout.addWidget(label)

    def _add_attachment(self, message: Message) -> None:
        if not message.attachment or message.recalled:
            return
        attachment = message.attachment
        file_name = str(attachment.get("file_name", "文件"))
        size = int(attachment.get("size_bytes", 0) or 0)
        button = QPushButton(f"↓  {file_name}  ·  {format_bytes(size)}")
        button.setProperty("attachment", True)
        button.setToolTip("下载附件")
        button.clicked.connect(lambda: self.attachment_requested.emit(attachment))
        self._layout.addWidget(button)

    def _add_reactions(self, message: Message) -> None:
        reactions = QHBoxLayout()
        reactions.setSpacing(4)
        for reaction in message.reactions:
            active = self._current_user_id in reaction.user_ids
            button = QPushButton(f"{reaction.emoji}  {len(reaction.user_ids)}")
            button.setCheckable(True)
            button.setChecked(active)
            button.setProperty("reaction", True)
            button.clicked.connect(
                lambda _, emoji=reaction.emoji, was_active=active: self.reaction_requested.emit(
                    message.message_id, emoji, not was_active
                )
            )
            reactions.addWidget(button)
        if not message.recalled:
            reactions.addWidget(self._reaction_menu(message))
        reactions.addStretch()
        self._layout.addLayout(reactions)

    def _reaction_menu(self, message: Message) -> QPushButton:
        button = QPushButton("＋")
        button.setProperty("reaction", True)
        button.setToolTip("添加表情回应")
        menu = QMenu(button)
        for emoji in REACTION_CHOICES:
            action = menu.addAction(emoji)
            active = next(
                (self._current_user_id in item.user_ids for item in message.reactions if item.emoji == emoji),
                False,
            )
            action.triggered.connect(
                lambda _, value=emoji, was_active=active: self.reaction_requested.emit(
                    message.message_id, value, not was_active
                )
            )
        button.setMenu(menu)
        return button

    def _context_menu(self) -> None:
        menu = QMenu(self)
        reply = menu.addAction("回复")
        reply.triggered.connect(lambda: self.reply_requested.emit(self._message))
        forward = menu.addAction("转发")
        forward.triggered.connect(lambda: self.forward_requested.emit(self._message.message_id))
        favorite = menu.addAction("收藏")
        favorite.triggered.connect(lambda: self.favorite_requested.emit(self._message.message_id))
        context = menu.addAction("移出 AI 上下文" if self._context_selected else "加入 AI 上下文")
        context.triggered.connect(lambda: self.context_toggle_requested.emit(self._message.message_id))
        if self._own and not self._message.recalled:
            edit = menu.addAction("编辑")
            edit.triggered.connect(
                lambda: self.edit_requested.emit(
                    self._message.message_id,
                    self._message.content,
                )
            )
            recall = menu.addAction("撤回")
            recall.triggered.connect(lambda: self.recall_requested.emit(self._message.message_id))
        menu.exec(self.mapToGlobal(self.rect().center()))


def format_bytes(size: int) -> str:
    if size < 1024:
        return f"{size} B"
    if size < 1024 * 1024:
        return f"{size / 1024:.1f} KB"
    return f"{size / (1024 * 1024):.1f} MB"
