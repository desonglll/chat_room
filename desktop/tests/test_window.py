import os

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtWidgets import QApplication

from echo_chat.main_window import MainWindow


def test_main_window_registers_incremental_parity_pages() -> None:
    application = QApplication.instance() or QApplication([])
    window = MainWindow("http://127.0.0.1:3000")

    assert window._content.count() == 7
    assert not window._sidebar._ai_button.isVisible()
    assert window.minimumWidth() == 880

    assert window._chat.show_message_context(
        [
            {
                "id": "message-1",
                "sender_id": "user-1",
                "sender": "mike",
                "content": "older result",
                "created_at": "2026-08-20T10:00:00Z",
            }
        ],
        "message-1",
    )
    assert "message-1" in window._chat._bubbles

    window.close()
    assert application is not None
