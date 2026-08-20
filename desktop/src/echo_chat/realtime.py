from __future__ import annotations

import json
from typing import Any
from urllib.parse import urlsplit, urlunsplit

from PySide6.QtCore import QObject, QTimer, QUrl, Signal
from PySide6.QtNetwork import QAbstractSocket
from PySide6.QtWebSockets import QWebSocket


class RealtimeClient(QObject):
    """Owns account and active-room WebSockets, including authentication and reconnects."""

    account_event = Signal(object)
    room_event = Signal(object)
    account_status = Signal(str)
    room_status = Signal(str)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._account = QWebSocket(parent=self)
        self._room = QWebSocket(parent=self)
        self._account_timer = QTimer(self)
        self._account_timer.setSingleShot(True)
        self._account_timer.setInterval(1200)
        self._room_timer = QTimer(self)
        self._room_timer.setSingleShot(True)
        self._room_timer.setInterval(1200)
        self._server_url = ""
        self._token = ""
        self._room_id = ""
        self._room_password = ""
        self._room_has_password = False
        self._account_desired = False
        self._room_desired = False
        self._connect_signals()

    @property
    def room_id(self) -> str:
        return self._room_id

    def connect_account(self, server_url: str, token: str) -> None:
        self._server_url = server_url
        self._token = token
        self._account_desired = True
        self._account.abort()
        self._open_account()

    def open_room(self, room_id: str, password: str = "", has_password: bool = False) -> None:
        self._room_desired = False
        self._room.abort()
        self._room_id = room_id
        self._room_password = password
        self._room_has_password = has_password
        self._room_desired = True
        self.room_status.emit("connecting")
        self._open_room()

    def close_room(self) -> None:
        self._room_desired = False
        self._room_timer.stop()
        self._room.close()
        self._room_id = ""
        self.room_status.emit("idle")

    def shutdown(self) -> None:
        self._account_desired = False
        self._room_desired = False
        self._account_timer.stop()
        self._room_timer.stop()
        self._account.close()
        self._room.close()

    def send_message(self, content: str, reply_to: str = "") -> bool:
        frame: dict[str, Any] = {"type": "message", "content": content}
        if reply_to:
            frame["reply_to"] = reply_to
        return self._send_room(frame)

    def edit_message(self, message_id: str, content: str) -> bool:
        return self._send_room({"type": "edit", "message_id": message_id, "content": content})

    def recall_message(self, message_id: str) -> bool:
        return self._send_room({"type": "recall", "message_id": message_id})

    def react(self, message_id: str, emoji: str, active: bool) -> bool:
        return self._send_room(
            {"type": "reaction", "message_id": message_id, "emoji": emoji, "active": active}
        )

    def mark_read(self, message_id: str) -> bool:
        return self._send_room({"type": "read", "message_id": message_id})

    def send_typing(self, content: str) -> bool:
        return self._send_room({"type": "typing", "content": content[:512]})

    def _connect_signals(self) -> None:
        self._account.connected.connect(self._account_connected)
        self._account.disconnected.connect(self._account_disconnected)
        self._account.textMessageReceived.connect(self._account_message)
        self._account.errorOccurred.connect(lambda _: self.account_status.emit("offline"))
        self._room.connected.connect(self._room_connected)
        self._room.disconnected.connect(self._room_disconnected)
        self._room.textMessageReceived.connect(self._room_message)
        self._room.errorOccurred.connect(lambda _: self.room_status.emit("offline"))
        self._account_timer.timeout.connect(self._open_account)
        self._room_timer.timeout.connect(self._open_room)

    def _open_account(self) -> None:
        if self._account_desired and self._server_url:
            self.account_status.emit("connecting")
            self._account.open(QUrl(websocket_url(self._server_url, "/ws/account")))

    def _open_room(self) -> None:
        if self._room_desired and self._server_url and self._room_id:
            self._room.open(QUrl(websocket_url(self._server_url, f"/ws/{self._room_id}")))

    def _account_connected(self) -> None:
        self.account_status.emit("online")
        self._send(self._account, {"token": self._token})

    def _room_connected(self) -> None:
        if self._room_has_password:
            frame = {"type": "auth", "token": self._token, "password": self._room_password}
        else:
            frame = {"type": "join", "token": self._token}
        self._send(self._room, frame)

    def _account_disconnected(self) -> None:
        self.account_status.emit("offline")
        if self._account_desired:
            self._account_timer.start()

    def _room_disconnected(self) -> None:
        if not self._room_desired:
            return
        self.room_status.emit("reconnecting")
        self._room_timer.start()

    def _account_message(self, raw: str) -> None:
        event = parse_json(raw)
        if event is not None:
            self.account_event.emit(event)

    def _room_message(self, raw: str) -> None:
        event = parse_json(raw)
        if event is None:
            return
        if event.get("type") == "auth_fail":
            self._room_desired = False
            self.room_status.emit(f"failed:{event.get('reason', '无法连接聊天室')}")
            self._room.close()
            return
        if event.get("type") == "auth_ok":
            self.room_status.emit("online")
        self.room_event.emit(event)

    def _send_room(self, payload: dict[str, Any]) -> bool:
        if self._room.state() != QAbstractSocket.SocketState.ConnectedState:
            return False
        self._send(self._room, payload)
        return True

    @staticmethod
    def _send(socket: QWebSocket, payload: dict[str, Any]) -> None:
        socket.sendTextMessage(json.dumps(payload, ensure_ascii=False, separators=(",", ":")))


def websocket_url(server_url: str, path: str) -> str:
    parts = urlsplit(server_url)
    scheme = "wss" if parts.scheme == "https" else "ws"
    prefix = parts.path.rstrip("/")
    return urlunsplit((scheme, parts.netloc, f"{prefix}{path}", "", ""))


def parse_json(raw: str) -> dict[str, Any] | None:
    try:
        value = json.loads(raw)
    except json.JSONDecodeError:
        return None
    return value if isinstance(value, dict) else None
