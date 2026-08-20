from __future__ import annotations

import hashlib
from datetime import datetime

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QLabel, QLayout, QWidget

AVATAR_COLORS = ("#087f6b", "#db6b57", "#3273a8", "#8a5b00", "#7357a8", "#b34d78")


class AvatarLabel(QLabel):
    def __init__(
        self,
        identity: str,
        emoji: str,
        name: str,
        size: int = 42,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setFixedSize(size, size)
        self.setAlignment(Qt.AlignCenter)
        self.setText(emoji or (name[:1].upper() if name else "?"))
        color = avatar_color(identity)
        font_size = max(13, int(size * 0.38))
        self.setStyleSheet(
            f"background:{color};color:white;border-radius:{size // 2}px;"
            f"font-size:{font_size}px;font-weight:600;"
        )


def avatar_color(identity: str) -> str:
    digest = hashlib.sha256(identity.encode("utf-8")).digest()
    return AVATAR_COLORS[digest[0] % len(AVATAR_COLORS)]


def clear_layout(layout: QLayout) -> None:
    while layout.count():
        item = layout.takeAt(0)
        if child_layout := item.layout():
            clear_layout(child_layout)
            child_layout.deleteLater()
        if widget := item.widget():
            widget.deleteLater()


def format_time(value: str) -> str:
    if not value:
        return ""
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone()
    except ValueError:
        return ""
    today = datetime.now().astimezone().date()
    return parsed.strftime("%H:%M") if parsed.date() == today else parsed.strftime("%m/%d")
