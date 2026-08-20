from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QStandardPaths
from PySide6.QtWidgets import QDialog, QFileDialog, QInputDialog, QLineEdit, QMessageBox, QWidget

from .action_dialogs import ForwardDialog, ProfileDialog
from .models import Conversation, JsonObject, Room, User
from .room_dialogs import CreateRoomDialog, DiscoverRoomsDialog


def create_room_values(parent: QWidget) -> tuple[str, str, str, str, str] | None:
    dialog = CreateRoomDialog(parent)
    return dialog.values if dialog.exec() == QDialog.DialogCode.Accepted else None


def discover_room(parent: QWidget, rooms: list[Room]) -> tuple[Room, str] | None:
    dialog = DiscoverRoomsDialog(rooms, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted or not dialog.selected_room:
        return None
    room = dialog.selected_room
    password = ""
    if room.has_password and not room.can_open:
        password, accepted = QInputDialog.getText(
            parent,
            "聊天室密码",
            f"输入“{room.name}”的访问密码",
            QLineEdit.EchoMode.Password,
        )
        if not accepted:
            return None
    return room, password


def room_identifier(parent: QWidget) -> str:
    value, accepted = QInputDialog.getText(parent, "加入聊天室", "聊天室 ID")
    return value.strip() if accepted else ""


def profile_values(parent: QWidget, user: User) -> dict[str, str] | None:
    dialog = ProfileDialog(user, parent)
    return dialog.values if dialog.exec() == QDialog.DialogCode.Accepted else None


def choose_upload(parent: QWidget, max_upload_bytes: int) -> str:
    file_path, _ = QFileDialog.getOpenFileName(parent, "发送附件")
    if not file_path:
        return ""
    size = Path(file_path).stat().st_size
    if size <= max_upload_bytes:
        return file_path
    limit = max_upload_bytes / (1024 * 1024)
    QMessageBox.warning(parent, "文件过大", f"单个文件不能超过 {limit:g} MB。")
    return ""


def choose_download(parent: QWidget, attachment: JsonObject) -> tuple[str, str] | None:
    download_url = str(attachment.get("download_url", ""))
    if not download_url:
        return None
    file_name = str(attachment.get("file_name", "文件"))
    downloads = QStandardPaths.writableLocation(QStandardPaths.StandardLocation.DownloadLocation)
    destination, _ = QFileDialog.getSaveFileName(
        parent,
        "保存附件",
        str(Path(downloads) / file_name),
    )
    return (download_url, destination) if destination else None


def forward_targets(parent: QWidget, conversations: list[Conversation]) -> list[str]:
    dialog = ForwardDialog(conversations, parent)
    if dialog.exec() != QDialog.DialogCode.Accepted:
        return []
    return dialog.selected_room_ids
