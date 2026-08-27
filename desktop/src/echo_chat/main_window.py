from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QSettings, Qt
from PySide6.QtGui import QCloseEvent, QIcon, QPixmap
from PySide6.QtWidgets import (
    QApplication,
    QHBoxLayout,
    QInputDialog,
    QLabel,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QSplitter,
    QStackedWidget,
    QStatusBar,
    QVBoxLayout,
    QWidget,
)

from .api import ApiClient
from .chat_view import ChatView
from .contacts_view import ContactsView
from .login_view import LoginView
from .models import Conversation, FriendRequest, JsonObject, User
from .notifications import NotificationManager
from .realtime import RealtimeClient
from .sidebar import Sidebar
from .workspace_features import WorkspaceFeaturesMixin
from .workspace_responses import WorkspaceResponsesMixin


class MainWindow(WorkspaceResponsesMixin, WorkspaceFeaturesMixin, QMainWindow):
    def __init__(self, server_url: str = "", parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Echo Gate")
        self.resize(1220, 790)
        self.setMinimumSize(880, 600)
        self._settings = QSettings()
        self._icon = QIcon(str(Path(__file__).parent / "assets" / "app-icon.png"))
        self.setWindowIcon(self._icon)
        self._api = ApiClient(self)
        self._realtime = RealtimeClient(self)
        self._current_user: User | None = None
        self._conversations: dict[str, Conversation] = {}
        self._friends: list[User] = []
        self._incoming: list[FriendRequest] = []
        self._outgoing: list[FriendRequest] = []
        self._blocked: list[User] = []
        self._members_dialog = None
        self._active_room_id = ""
        self._active_room_password = ""
        self._room_passwords: dict[str, str] = {}
        self._pending_room_id = ""
        self._pending_created_password = ""
        self._max_upload_bytes = 50 * 1024 * 1024
        self._quitting = False
        self._build(server_url or str(self._settings.value("server/url", "http://127.0.0.1:3000")))
        self._connect_signals()

    def _build(self, server_url: str) -> None:
        self._pages = QStackedWidget(objectName="appRoot")
        self._login = LoginView(server_url)
        self._pages.addWidget(self._login)
        workspace = QWidget(objectName="appRoot")
        workspace_layout = QHBoxLayout(workspace)
        workspace_layout.setContentsMargins(0, 0, 0, 0)
        splitter = QSplitter(Qt.Horizontal)
        self._sidebar = Sidebar()
        splitter.addWidget(self._sidebar)
        self._content = QStackedWidget()
        self._empty = self._empty_page()
        self._chat = ChatView()
        self._contacts = ContactsView()
        self._content.addWidget(self._empty)
        self._content.addWidget(self._chat)
        self._content.addWidget(self._contacts)
        splitter.addWidget(self._content)
        splitter.setSizes([320, 900])
        splitter.setCollapsible(0, False)
        splitter.setCollapsible(1, False)
        workspace_layout.addWidget(splitter)
        self._pages.addWidget(workspace)
        self.setCentralWidget(self._pages)
        self.setStatusBar(QStatusBar())
        self._notifications = NotificationManager(self._icon, self)

    def _empty_page(self) -> QWidget:
        page = QWidget(objectName="chatPage")
        layout = QVBoxLayout(page)
        layout.addStretch()
        icon = QLabel(alignment=Qt.AlignCenter)
        pixmap = QPixmap(str(Path(__file__).parent / "assets" / "app-icon.png"))
        icon.setPixmap(pixmap.scaled(58, 58, Qt.KeepAspectRatio, Qt.SmoothTransformation))
        icon.setStyleSheet("opacity:0.4;")
        layout.addWidget(icon)
        title = QLabel("选择一个会话", objectName="sectionTitle", alignment=Qt.AlignCenter)
        layout.addWidget(title)
        layout.addWidget(QLabel("消息会显示在这里", objectName="muted", alignment=Qt.AlignCenter))
        layout.addStretch()
        return page

    def _connect_signals(self) -> None:
        self._login.authenticate_requested.connect(self._authenticate)
        self._api.completed.connect(self._api_completed)
        self._api.failed.connect(self._api_failed)
        self._sidebar.conversation_selected.connect(self._open_conversation)
        self._sidebar.selection_cleared.connect(self._clear_conversation)
        self._sidebar.contacts_requested.connect(lambda: self._content.setCurrentWidget(self._contacts))
        self._sidebar.create_room_requested.connect(self._create_room)
        self._sidebar.discover_rooms_requested.connect(self._api.rooms)
        self._sidebar.join_room_requested.connect(self._lookup_room)
        self._sidebar.profile_requested.connect(self._edit_profile)
        self._sidebar.logout_requested.connect(self._request_logout)
        self._contacts.close_requested.connect(self._show_active_chat)
        self._contacts.search_requested.connect(self._api.search_users)
        self._contacts.action_requested.connect(self._contact_action)
        self._chat.send_requested.connect(self._send_message)
        self._chat.edit_requested.connect(self._realtime.edit_message)
        self._chat.recall_requested.connect(self._recall_message)
        self._chat.reaction_requested.connect(self._realtime.react)
        self._chat.read_candidate.connect(self._mark_read)
        self._chat.typing_requested.connect(self._realtime.send_typing)
        self._chat.attachment_requested.connect(self._upload_attachment)
        self._chat.download_requested.connect(self._download_attachment)
        self._chat.forward_requested.connect(self._forward_message)
        self._chat.ai_requested.connect(self._request_ai_suggestions)
        self._chat.manage_members_requested.connect(self._manage_room_members)
        self._realtime.account_event.connect(self._account_event)
        self._realtime.room_event.connect(self._room_event)
        self._realtime.room_status.connect(self._chat.set_status)
        self._notifications.open_requested.connect(self._raise_window)
        self._notifications.conversation_requested.connect(self._open_from_notification)
        self._notifications.quit_requested.connect(self._quit)

    def _authenticate(self, mode: str, server: str, username: str, password: str) -> None:
        self._login.set_error("")
        self._login.set_loading(True)
        try:
            self._api.authenticate(mode, server, username, password)
        except ValueError as error:
            self._login.set_loading(False)
            self._login.set_error(str(error))

    def _complete_authentication(self, session: JsonObject) -> None:
        token = str(session.get("token", ""))
        user_value = session.get("user")
        if not token or not isinstance(user_value, dict):
            self._api_failed("authenticate", "登录响应不完整", 0)
            return
        self._current_user = User.from_dict(user_value)
        self._api.set_session(self._api.server_url, token)
        self._settings.setValue("server/url", self._api.server_url)
        self._login.set_loading(False)
        self._sidebar.set_user(self._current_user)
        self._chat.set_current_user(self._current_user.id)
        self._pages.setCurrentIndex(1)
        self._realtime.connect_account(self._api.server_url, token)
        self._api.conversations()
        self._api.public_config()
        self._refresh_contacts()
        self.statusBar().showMessage("登录成功", 2200)

    def _open_conversation(self, room_id: str) -> None:
        conversation = self._conversations.get(room_id)
        if not conversation:
            return
        password = ""
        if conversation.has_password:
            password = self._room_passwords.get(room_id, "")
            if not password:
                password, accepted = QInputDialog.getText(
                    self,
                    "聊天室密码",
                    f"输入“{conversation.title}”的访问密码",
                    QLineEdit.EchoMode.Password,
                )
                if not accepted:
                    return
                self._room_passwords[room_id] = password
        self._active_room_id = room_id
        self._active_room_password = password
        self._chat.set_conversation(conversation)
        self._content.setCurrentWidget(self._chat)
        self._realtime.open_room(room_id, password, conversation.has_password)

    def _clear_conversation(self) -> None:
        self._realtime.close_room()
        self._active_room_id = ""
        self._active_room_password = ""
        self._content.setCurrentWidget(self._empty)

    def _show_active_chat(self) -> None:
        self._content.setCurrentWidget(self._chat if self._active_room_id else self._empty)

    def _send_message(self, content: str, reply_to: str) -> None:
        if self._realtime.send_message(content, reply_to):
            self._chat.message_sent()
        else:
            self._chat.show_send_error()

    def _recall_message(self, message_id: str) -> None:
        if QMessageBox.question(self, "撤回消息", "确定撤回这条消息吗？") == QMessageBox.Yes:
            self._realtime.recall_message(message_id)

    def _mark_read(self, message_id: str) -> None:
        active = QApplication.applicationState() == Qt.ApplicationState.ApplicationActive
        if active and self._content.currentWidget() is self._chat:
            self._realtime.mark_read(message_id)

    def _account_event(self, event: JsonObject) -> None:
        kind = event.get("type")
        if kind == "new_message":
            room_id = str(event.get("room_id", ""))
            if self._current_user and event.get("sender_id") != self._current_user.id:
                inactive = QApplication.applicationState() != Qt.ApplicationActive
                if inactive or room_id != self._active_room_id:
                    title = str(event.get("conversation_title") or event.get("room_name") or "新消息")
                    body = str(event.get("content") or event.get("attachment_file_name") or "收到一条新消息")
                    self._notifications.show_message(room_id, title, body)
            self._api.conversations()
        elif kind == "unread_counts":
            self._api.conversations()
        elif kind == "social_changed":
            self._refresh_contacts()

    def _room_event(self, event: JsonObject) -> None:
        self._chat.apply_event(event)
        if event.get("type") in {"broadcast", "message_edited", "message_recalled"}:
            self._api.conversations()

    def _open_from_notification(self, room_id: str) -> None:
        self._raise_window()
        if not self._sidebar.select_room(room_id):
            self._pending_room_id = room_id
            self._api.conversations()

    def _raise_window(self) -> None:
        self.showNormal()
        self.raise_()
        self.activateWindow()

    def _request_logout(self) -> None:
        if QMessageBox.question(self, "退出登录", "确定退出当前账户吗？") == QMessageBox.Yes:
            self._api.logout()

    def _finish_logout(self) -> None:
        self._realtime.shutdown()
        self._current_user = None
        self._conversations.clear()
        self._active_room_id = ""
        self._pages.setCurrentIndex(0)
        self._login.clear_password()

    def _quit(self) -> None:
        self._quitting = True
        self._notifications.hide()
        self._realtime.shutdown()
        QApplication.quit()

    def closeEvent(self, event: QCloseEvent) -> None:
        if not self._quitting and self._notifications.available and self._current_user:
            event.ignore()
            self.hide()
            self._notifications.show_background_hint()
            return
        self._realtime.shutdown()
        event.accept()
