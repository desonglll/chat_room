from __future__ import annotations


def application_stylesheet() -> str:
    return """
    * {
        font-family: "SF Pro Text", "PingFang SC", "Microsoft YaHei";
        font-size: 13px;
        color: #172321;
    }
    QMainWindow, QWidget#appRoot { background: #f7faf9; }
    QWidget#loginPage { background: #eef4f1; }
    QFrame#loginPanel {
        background: #ffffff;
        border: 1px solid #dce6e2;
        border-radius: 8px;
    }
    QWidget#sidebar { background: #ffffff; border-right: 1px solid #dce6e2; }
    QWidget#chatPage, QWidget#contactsPage, QWidget#featurePage { background: #f7faf9; }
    QFrame#topBar { background: #ffffff; border-bottom: 1px solid #dce6e2; }
    QLabel#brandTitle { font-size: 17px; font-weight: 700; }
    QLabel#pageTitle { font-size: 16px; font-weight: 650; }
    QLabel#sectionTitle { font-size: 18px; font-weight: 650; }
    QLabel#dialogTitle { font-size: 20px; font-weight: 700; }
    QLabel#muted, QLabel.muted { color: #697d78; }
    QLabel#error {
        color: #b9362b; background: #fff0ed; border: 1px solid #ffc7bd;
        border-radius: 6px; padding: 8px;
    }
    QLineEdit, QTextEdit {
        background: #ffffff;
        border: 1px solid #c4d1cd;
        border-radius: 7px;
        padding: 8px 10px;
        selection-background-color: #b8e5d7;
    }
    QLineEdit:focus, QTextEdit:focus { border: 1px solid #087f6b; }
    QPushButton {
        min-height: 34px;
        padding: 0 13px;
        border: 1px solid #c4d1cd;
        border-radius: 7px;
        background: #ffffff;
        font-weight: 550;
    }
    QPushButton:hover { background: #eef4f1; border-color: #a8b8b3; }
    QPushButton:pressed { background: #dce6e2; }
    QPushButton:disabled { color: #8a9b96; background: #eef4f1; }
    QPushButton[primary="true"] { color: #ffffff; background: #087f6b; border-color: #087f6b; }
    QPushButton[primary="true"]:hover { background: #066456; }
    QPushButton[danger="true"] { color: #b9362b; border-color: #ffc7bd; }
    QPushButton[segment="true"] { border: 0; background: transparent; color: #52615d; }
    QPushButton[segment="true"]:checked { background: #ddf4ec; color: #066456; }
    QToolButton {
        min-width: 34px;
        min-height: 34px;
        border: 0;
        border-radius: 7px;
        background: transparent;
    }
    QToolButton:hover { background: #eef4f1; }
    QToolButton::menu-indicator { image: none; width: 0; }
    QListWidget { background: transparent; border: 0; outline: 0; }
    QListWidget::item { border: 0; padding: 0; }
    QListWidget::item:selected { background: transparent; }
    QListWidget#featureList {
        background: #ffffff; border: 1px solid #dce6e2; border-radius: 7px;
    }
    QListWidget#featureList::item {
        min-height: 42px; padding: 8px 10px; border-bottom: 1px solid #eef4f1;
    }
    QListWidget#featureList::item:hover { background: #eef4f1; }
    QListWidget#featureList::item:selected { background: #ddf4ec; color: #172321; }
    QFrame#conversationRow { background: transparent; border-radius: 7px; }
    QFrame#conversationRow[selected="true"] { background: #e6f5f0; }
    QFrame#contactRow { background: #ffffff; border-bottom: 1px solid #eef4f1; }
    QLabel#unreadBadge {
        color: #ffffff;
        background: #087f6b;
        border-radius: 9px;
        min-width: 18px;
        min-height: 18px;
        font-size: 10px;
        font-weight: 700;
    }
    QScrollArea { border: 0; background: transparent; }
    QScrollArea > QWidget > QWidget { background: transparent; }
    QFrame#incomingBubble {
        background: #ffffff;
        border: 1px solid #dce6e2;
        border-radius: 8px;
    }
    QFrame#outgoingBubble {
        background: #ddf4ec;
        border: 1px solid #b8e5d7;
        border-radius: 8px;
    }
    QFrame#incomingBubble[contextSelected="true"], QFrame#outgoingBubble[contextSelected="true"] {
        border: 2px solid #087f6b;
    }
    QFrame#incomingBubble[jumpTarget="true"], QFrame#outgoingBubble[jumpTarget="true"] {
        border: 2px solid #b9770e;
    }
    QFrame#selectedContext {
        background: #eef7ff; border: 1px solid #bfd8ee; border-radius: 7px;
    }
    QFrame#systemMessage { background: transparent; }
    QPushButton[reaction="true"] {
        min-height: 25px;
        padding: 0 7px;
        border-radius: 6px;
        background: #ffffff;
        border-color: #dce6e2;
        font-weight: 500;
    }
    QPushButton[reaction="true"]:checked { background: #ddf4ec; border-color: #7dcdb9; }
    QPushButton[attachment="true"] {
        min-height: 38px; padding: 0 10px; text-align: left;
        color: #066456; background: rgba(255, 255, 255, 0.72);
        border-color: #b8d8cf;
    }
    QFrame#suggestionPanel {
        background: #f1f6ff; border: 1px solid #d8e4f5; border-radius: 7px;
    }
    QDialog { background: #f8faf9; }
    QDialog QListWidget { background: #ffffff; border: 1px solid #dce6e2; border-radius: 7px; }
    QDialog QListWidget::item { padding: 8px; border-radius: 5px; }
    QDialog QListWidget::item:hover { background: #eef4f1; }
    QDialog QListWidget::item:selected { background: #ddf4ec; color: #066456; }
    QStatusBar { background: #ffffff; border-top: 1px solid #dce6e2; color: #52615d; }
    QMenu { background: #ffffff; border: 1px solid #dce6e2; padding: 5px; }
    QMenu::item { min-height: 28px; padding: 3px 22px 3px 10px; border-radius: 5px; }
    QMenu::item:selected { background: #eef4f1; }
    QSplitter::handle { background: #dce6e2; width: 1px; }
    QScrollBar:vertical { width: 9px; background: transparent; margin: 2px; }
    QScrollBar::handle:vertical { background: #c4d1cd; min-height: 32px; border-radius: 4px; }
    QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical { height: 0; }
    """
