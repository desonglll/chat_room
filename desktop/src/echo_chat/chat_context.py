from __future__ import annotations

from PySide6.QtCore import QTimer

from .message_bubble import MessageBubble
from .models import Message
from .ui_common import clear_layout


class ChatContextMixin:
    def show_message_context(self, payload: list[object], target_message_id: str) -> bool:
        self._timeline.clear()
        self._bubbles.clear()
        clear_layout(self._messages)
        self._messages.addStretch()
        for item in payload:
            if isinstance(item, dict):
                self._add_message(Message.from_dict(item))
        return self.scroll_to_message(target_message_id)

    def clear_ai_context(self) -> None:
        for message_id in self._selected_ids:
            if bubble := self._bubbles.get(message_id):
                bubble.set_context_selected(False)
        self._selected_ids.clear()
        self._context_bar.hide()

    def scroll_to_message(self, message_id: str) -> bool:
        bubble = self._bubbles.get(message_id)
        if not bubble:
            return False
        self._scroll.ensureWidgetVisible(bubble, 24, 80)
        bubble.setProperty("jumpTarget", True)
        bubble.style().unpolish(bubble)
        bubble.style().polish(bubble)
        QTimer.singleShot(2400, lambda: self._clear_jump_target(bubble))
        return True

    def _toggle_ai_context(self, message_id: str) -> None:
        if message_id in self._selected_ids:
            self._selected_ids.remove(message_id)
            selected = False
        elif len(self._selected_ids) < 50:
            self._selected_ids.append(message_id)
            selected = True
        else:
            return
        if bubble := self._bubbles.get(message_id):
            bubble.set_context_selected(selected)
        self._context_label.setText(f"已选择 {len(self._selected_ids)} 条消息作为 AI 上下文")
        self._context_bar.setVisible(bool(self._selected_ids))

    @staticmethod
    def _clear_jump_target(bubble: MessageBubble) -> None:
        bubble.setProperty("jumpTarget", False)
        bubble.style().unpolish(bubble)
        bubble.style().polish(bubble)
