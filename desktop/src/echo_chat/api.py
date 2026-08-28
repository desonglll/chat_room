from __future__ import annotations

import json
from pathlib import Path
from typing import Any
from urllib.parse import quote, urlencode, urlsplit, urlunsplit

from PySide6.QtCore import QByteArray, QFile, QIODevice, QMimeDatabase, QObject, QSaveFile, QUrl, Signal
from PySide6.QtNetwork import (
    QHttpMultiPart,
    QHttpPart,
    QNetworkAccessManager,
    QNetworkReply,
    QNetworkRequest,
)

from .feature_api import FeatureApiMixin


class ApiClient(FeatureApiMixin, QObject):
    """Qt-native HTTP adapter for the server operations used by the desktop client."""

    completed = Signal(str, object)
    failed = Signal(str, str, int)
    upload_progress = Signal(int, int)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._manager = QNetworkAccessManager(self)
        self._server_url = "http://127.0.0.1:3000"
        self._token = ""

    @property
    def server_url(self) -> str:
        return self._server_url

    @property
    def token(self) -> str:
        return self._token

    def set_session(self, server_url: str, token: str) -> None:
        self._server_url = normalize_server_url(server_url)
        self._token = token

    def authenticate(self, mode: str, server_url: str, username: str, password: str) -> None:
        self._server_url = normalize_server_url(server_url)
        endpoint = "register" if mode == "register" else "login"
        self.request_json(
            "authenticate",
            "POST",
            f"/api/users/{endpoint}",
            {"username": username, "password": password},
            False,
        )

    def logout(self) -> None:
        self.request_json("logout", "POST", "/api/users/logout")

    def public_config(self) -> None:
        self.request_json("config", "GET", "/api/config", authenticated=False)

    def conversations(self) -> None:
        self.request_json("conversations", "GET", "/api/conversations")

    def rooms(self) -> None:
        self.request_json("rooms", "GET", "/api/rooms")

    def get_room(self, room_id: str) -> None:
        room_path = quote(room_id.strip(), safe="")
        self.request_json("room-lookup", "GET", f"/api/rooms/{room_path}")

    def create_room(
        self,
        name: str,
        password: str,
        join_policy: str,
        avatar_emoji: str,
        description: str,
    ) -> None:
        self.request_json(
            "create-room",
            "POST",
            "/api/rooms",
            {
                "name": name,
                "password": password or None,
                "join_policy": join_policy,
                "avatar_emoji": avatar_emoji,
                "description": description,
            },
        )

    def join_room(self, room_id: str, password: str) -> None:
        room_path = quote(room_id, safe="")
        self.request_json(
            f"join-room:{room_id}",
            "POST",
            f"/api/rooms/{room_path}/join-requests",
            {"password": password or None},
        )

    def update_profile(self, payload: dict[str, str]) -> None:
        self.request_json("update-profile", "PATCH", "/api/users/me", payload)

    def forward_messages(self, message_ids: list[str], target_room_ids: list[str]) -> None:
        self.request_json(
            "forward-messages",
            "POST",
            "/api/messages/forward",
            {"message_ids": message_ids, "target_room_ids": target_room_ids},
        )

    def ai_suggestions(self, room_id: str) -> None:
        room_path = quote(room_id, safe="")
        self.request_json("ai-suggestions", "POST", f"/api/rooms/{room_path}/ai/suggest")

    def room_members(self, room_id: str) -> None:
        room_path = quote(room_id, safe="")
        self.request_json(f"room-members:{room_id}", "GET", f"/api/rooms/{room_path}/members")

    def invite_room_member(self, room_id: str, username: str) -> None:
        room_path = quote(room_id, safe="")
        self.request_json(
            f"invite-member:{room_id}",
            "POST",
            f"/api/rooms/{room_path}/invitations",
            {"username": username},
        )

    def update_room_member(self, room_id: str, user_id: str, action: str, role: str = "") -> None:
        room_path = quote(room_id, safe="")
        user_path = quote(user_id, safe="")
        payload: dict[str, str] = {"action": action}
        if role:
            payload["role"] = role
        self.request_json(
            f"member-action:{room_id}",
            "PATCH",
            f"/api/rooms/{room_path}/members/{user_path}",
            payload,
        )

    def friends(self) -> None:
        self.request_json("friends", "GET", "/api/friends")

    def friend_requests(self, direction: str) -> None:
        query = urlencode({"direction": direction})
        self.request_json(f"requests:{direction}", "GET", f"/api/friend-requests?{query}")

    def blocks(self) -> None:
        self.request_json("blocks", "GET", "/api/blocks")

    def search_users(self, query: str) -> None:
        encoded = urlencode({"q": query, "limit": 30})
        self.request_json("user-search", "GET", f"/api/users/search?{encoded}")

    def start_direct(self, user_id: str) -> None:
        self.request_json("start-direct", "POST", "/api/direct-chats", {"user_id": user_id})

    def social_action(self, action: str, user_id: str) -> None:
        user_path = quote(user_id, safe="")
        mapping: dict[str, tuple[str, str, dict[str, str] | None]] = {
            "request": ("POST", "/api/friend-requests", {"user_id": user_id}),
            "accept": ("PATCH", f"/api/friend-requests/{user_path}", {"action": "accept"}),
            "decline": ("PATCH", f"/api/friend-requests/{user_path}", {"action": "decline"}),
            "cancel": ("DELETE", f"/api/friend-requests/{user_path}", None),
            "remove": ("DELETE", f"/api/friends/{user_path}", None),
            "block": ("PUT", f"/api/blocks/{user_path}", None),
            "unblock": ("DELETE", f"/api/blocks/{user_path}", None),
        }
        if action not in mapping:
            raise ValueError(f"unsupported social action: {action}")
        method, path, payload = mapping[action]
        self.request_json(f"social:{action}", method, path, payload)

    def upload_attachment(
        self,
        room_id: str,
        file_path: str,
        room_password: str = "",
        content: str = "",
        reply_to: str = "",
    ) -> None:
        source = QFile(file_path)
        if not source.open(QIODevice.OpenModeFlag.ReadOnly):
            self.failed.emit("attachment-upload", "无法读取所选文件", 0)
            return
        request = self._request_headers(
            f"/api/rooms/{quote(room_id, safe='')}/attachments",
            authenticated=True,
        )
        if room_password:
            request.setRawHeader(b"x-room-password", room_password.encode("utf-8"))
        multipart = QHttpMultiPart(QHttpMultiPart.ContentType.FormDataType)
        if content:
            multipart.append(text_part("content", content))
        if reply_to:
            multipart.append(text_part("reply_to", reply_to))
        file_part = QHttpPart()
        file_name = Path(file_path).name.replace('"', "") or "file"
        disposition = f'form-data; name="file"; filename="{file_name}"'.encode()
        file_part.setRawHeader(b"Content-Disposition", disposition)
        mime_type = QMimeDatabase().mimeTypeForFile(file_path).name() or "application/octet-stream"
        file_part.setRawHeader(b"Content-Type", mime_type.encode("ascii", errors="replace"))
        file_part.setBodyDevice(source)
        source.setParent(multipart)
        multipart.append(file_part)
        reply = self._manager.post(request, multipart)
        multipart.setParent(reply)
        reply.setProperty("operation", "attachment-upload")
        reply.uploadProgress.connect(lambda sent, total: self.upload_progress.emit(int(sent), int(total)))
        reply.finished.connect(lambda active=reply: self._finish(active))

    def download_attachment(self, download_url: str, destination: str) -> None:
        target = QSaveFile(destination)
        if not target.open(QIODevice.OpenModeFlag.WriteOnly):
            self.failed.emit("attachment-download", "无法写入所选位置", 0)
            return
        url = QUrl(download_url)
        if url.isRelative():
            url = QUrl(f"{self._server_url}{download_url}")
        request = QNetworkRequest(url)
        request.setRawHeader(b"Accept", b"application/octet-stream")
        reply = self._manager.get(request)
        target.setParent(reply)
        reply.readyRead.connect(lambda active=reply, output=target: output.write(active.readAll()))
        reply.finished.connect(
            lambda active=reply, output=target, path=destination: self._finish_download(active, output, path)
        )

    def request_json(
        self,
        operation: str,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
        authenticated: bool = True,
        extra_headers: dict[str, str] | None = None,
    ) -> None:
        request = self._request_headers(path, authenticated)
        for name, value in (extra_headers or {}).items():
            request.setRawHeader(name.encode("ascii"), value.encode("utf-8"))
        body = QByteArray()
        if payload is not None:
            request.setHeader(QNetworkRequest.ContentTypeHeader, "application/json")
            body = QByteArray(json.dumps(payload, ensure_ascii=False).encode("utf-8"))
        reply = self._manager.sendCustomRequest(request, method.encode(), body)
        reply.setProperty("operation", operation)
        reply.finished.connect(lambda active=reply: self._finish(active))

    def _request_headers(self, path: str, authenticated: bool) -> QNetworkRequest:
        request = QNetworkRequest(QUrl(f"{self._server_url}{path}"))
        request.setRawHeader(b"Accept", b"application/json")
        if authenticated and self._token:
            request.setRawHeader(b"Authorization", f"Bearer {self._token}".encode())
        return request

    def _finish(self, reply: QNetworkReply) -> None:
        operation = str(reply.property("operation"))
        status = reply.attribute(QNetworkRequest.HttpStatusCodeAttribute)
        status_code = int(status) if status is not None else 0
        raw = bytes(reply.readAll())
        network_error = reply.errorString()
        reply.deleteLater()
        if 200 <= status_code < 300:
            if not raw:
                self.completed.emit(operation, None)
                return
            try:
                self.completed.emit(operation, json.loads(raw))
            except (UnicodeDecodeError, json.JSONDecodeError):
                self.failed.emit(operation, "服务器返回了无法解析的数据", status_code)
            return
        message = api_error_message(operation, status_code, network_error, raw)
        self.failed.emit(operation, message, status_code)

    def _finish_download(self, reply: QNetworkReply, target: QSaveFile, destination: str) -> None:
        target.write(reply.readAll())
        status = reply.attribute(QNetworkRequest.HttpStatusCodeAttribute)
        status_code = int(status) if status is not None else 0
        network_error = reply.errorString()
        reply.deleteLater()
        if 200 <= status_code < 300 and target.commit():
            self.completed.emit("attachment-download", {"path": destination})
            return
        target.cancelWriting()
        message = api_error_message("attachment-download", status_code, network_error, b"")
        self.failed.emit("attachment-download", message, status_code)


def text_part(name: str, value: str) -> QHttpPart:
    part = QHttpPart()
    part.setRawHeader(b"Content-Disposition", f'form-data; name="{name}"'.encode("ascii"))
    part.setBody(QByteArray(value.encode("utf-8")))
    return part


def normalize_server_url(value: str) -> str:
    candidate = value.strip().rstrip("/")
    if "://" not in candidate:
        candidate = f"http://{candidate}"
    parts = urlsplit(candidate)
    if parts.scheme not in {"http", "https"} or not parts.netloc:
        raise ValueError("服务器地址必须是有效的 http:// 或 https:// 地址")
    return urlunsplit((parts.scheme, parts.netloc, parts.path.rstrip("/"), "", ""))


def api_error_message(operation: str, status: int, network_error: str, raw: bytes) -> str:
    if status == 0:
        return f"无法连接服务器：{network_error}"
    if status == 401:
        return "用户名、密码或登录状态无效" if operation == "authenticate" else "登录已过期"
    if status == 403:
        return "没有执行此操作的权限"
    if status == 404:
        return "目标不存在或已失效"
    if status == 409:
        return "当前状态不允许此操作"
    if status == 429:
        return "操作过于频繁，请稍后再试"
    try:
        parsed = json.loads(raw)
        if isinstance(parsed, dict) and parsed.get("message"):
            return str(parsed["message"])
    except (UnicodeDecodeError, json.JSONDecodeError):
        pass
    return f"请求失败（HTTP {status}）"
