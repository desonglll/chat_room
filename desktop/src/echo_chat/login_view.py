from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import (
    QButtonGroup,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)


class LoginView(QWidget):
    authenticate_requested = Signal(str, str, str, str)

    def __init__(self, server_url: str, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setObjectName("loginPage")
        self._mode = "login"
        self._build(server_url)

    def _build(self, server_url: str) -> None:
        page = QVBoxLayout(self)
        page.setContentsMargins(24, 24, 24, 24)
        page.addStretch()
        panel = QFrame(objectName="loginPanel")
        panel.setFixedWidth(410)
        form = QVBoxLayout(panel)
        form.setContentsMargins(34, 32, 34, 34)
        form.setSpacing(13)

        icon = QLabel(alignment=Qt.AlignCenter)
        pixmap = QPixmap(str(Path(__file__).parent / "assets" / "app-icon.png"))
        icon.setPixmap(pixmap.scaled(64, 64, Qt.KeepAspectRatio, Qt.SmoothTransformation))
        form.addWidget(icon)
        title = QLabel("Echo Chat", objectName="brandTitle", alignment=Qt.AlignCenter)
        form.addWidget(title)
        subtitle = QLabel("登录到你的聊天工作区", objectName="muted", alignment=Qt.AlignCenter)
        form.addWidget(subtitle)

        modes = QHBoxLayout()
        modes.setSpacing(4)
        mode_group = QButtonGroup(self)
        mode_group.setExclusive(True)
        self._login_mode = self._mode_button("登录", "login", True)
        self._register_mode = self._mode_button("注册", "register", False)
        mode_group.addButton(self._login_mode)
        mode_group.addButton(self._register_mode)
        modes.addWidget(self._login_mode)
        modes.addWidget(self._register_mode)
        form.addLayout(modes)

        form.addWidget(QLabel("服务器"))
        self._server = QLineEdit(server_url, placeholderText="http://127.0.0.1:3000")
        self._server.setClearButtonEnabled(True)
        form.addWidget(self._server)
        form.addWidget(QLabel("用户名"))
        self._username = QLineEdit(placeholderText="输入用户名")
        self._username.setClearButtonEnabled(True)
        form.addWidget(self._username)
        form.addWidget(QLabel("密码"))
        self._password = QLineEdit(placeholderText="至少 8 个字符")
        self._password.setEchoMode(QLineEdit.Password)
        self._password.returnPressed.connect(self._submit)
        form.addWidget(self._password)
        self._error = QLabel(objectName="error", wordWrap=True)
        self._error.hide()
        form.addWidget(self._error)
        self._submit_button = QPushButton("登录")
        self._submit_button.setProperty("primary", True)
        self._submit_button.clicked.connect(self._submit)
        form.addWidget(self._submit_button)

        row = QHBoxLayout()
        row.addStretch()
        row.addWidget(panel)
        row.addStretch()
        page.addLayout(row)
        page.addStretch()

    def _mode_button(self, label: str, mode: str, checked: bool) -> QPushButton:
        button = QPushButton(label, checkable=True, checked=checked)
        button.setProperty("segment", True)
        button.clicked.connect(lambda: self._set_mode(mode))
        return button

    def _set_mode(self, mode: str) -> None:
        self._mode = mode
        self._submit_button.setText("创建账户" if mode == "register" else "登录")
        self.set_error("")

    def _submit(self) -> None:
        server = self._server.text().strip()
        username = self._username.text().strip()
        password = self._password.text()
        if not server or not username or len(password) < 8:
            self.set_error("请填写服务器、用户名和至少 8 个字符的密码。")
            return
        self.authenticate_requested.emit(self._mode, server, username, password)

    def set_loading(self, loading: bool) -> None:
        self._submit_button.setEnabled(not loading)
        idle_text = "创建账户" if self._mode == "register" else "登录"
        self._submit_button.setText("正在连接…" if loading else idle_text)
        self._server.setEnabled(not loading)
        self._username.setEnabled(not loading)
        self._password.setEnabled(not loading)

    def set_error(self, message: str) -> None:
        self._error.setText(message)
        self._error.setVisible(bool(message))

    def clear_password(self) -> None:
        self._password.clear()
        self._password.setFocus()
