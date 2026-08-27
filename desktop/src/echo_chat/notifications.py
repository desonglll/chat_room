from __future__ import annotations

from PySide6.QtCore import QObject, QSettings, Signal
from PySide6.QtGui import QAction, QIcon
from PySide6.QtWidgets import QMenu, QSystemTrayIcon, QWidget


class NotificationManager(QObject):
    conversation_requested = Signal(str)
    open_requested = Signal()
    quit_requested = Signal()

    def __init__(self, icon: QIcon, parent: QWidget) -> None:
        super().__init__(parent)
        self._settings = QSettings()
        self._enabled = self._settings.value("notifications/enabled", True, bool)
        self._latest_room_id = ""
        self._tray = QSystemTrayIcon(icon, parent)
        self._tray.setToolTip("Echo Gate")
        self._tray.messageClicked.connect(self._message_clicked)
        self._tray.activated.connect(self._tray_activated)
        menu = QMenu(parent)
        open_action = QAction("打开 Echo Gate", menu)
        open_action.triggered.connect(self.open_requested)
        menu.addAction(open_action)
        self._notification_action = QAction("桌面通知", menu)
        self._notification_action.setCheckable(True)
        self._notification_action.setChecked(self._enabled)
        self._notification_action.toggled.connect(self.set_enabled)
        menu.addAction(self._notification_action)
        menu.addSeparator()
        quit_action = QAction("退出", menu)
        quit_action.triggered.connect(self.quit_requested)
        menu.addAction(quit_action)
        self._tray.setContextMenu(menu)
        if self.available:
            self._tray.show()

    @property
    def available(self) -> bool:
        return QSystemTrayIcon.isSystemTrayAvailable()

    @property
    def enabled(self) -> bool:
        return self._enabled

    def set_enabled(self, enabled: bool) -> None:
        self._enabled = enabled
        self._settings.setValue("notifications/enabled", enabled)
        if self._notification_action.isChecked() != enabled:
            self._notification_action.setChecked(enabled)

    def show_message(self, room_id: str, title: str, body: str) -> bool:
        if not self.available or not self._enabled or not QSystemTrayIcon.supportsMessages():
            return False
        self._latest_room_id = room_id
        self._tray.showMessage(
            title,
            body or "收到一条新消息",
            QSystemTrayIcon.MessageIcon.Information,
            6000,
        )
        return True

    def show_background_hint(self) -> None:
        if self.available and self._enabled:
            self._tray.showMessage(
                "Echo Gate 在后台运行",
                "新消息会继续通过系统通知提醒你。",
                QSystemTrayIcon.MessageIcon.Information,
                3500,
            )

    def hide(self) -> None:
        self._tray.hide()

    def _message_clicked(self) -> None:
        if self._latest_room_id:
            self.conversation_requested.emit(self._latest_room_id)
        else:
            self.open_requested.emit()

    def _tray_activated(self, reason: QSystemTrayIcon.ActivationReason) -> None:
        if reason in {QSystemTrayIcon.Trigger, QSystemTrayIcon.DoubleClick}:
            self.open_requested.emit()
