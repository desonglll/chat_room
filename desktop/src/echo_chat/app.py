from __future__ import annotations

import argparse
import sys

from PySide6.QtCore import QCoreApplication
from PySide6.QtWidgets import QApplication

from .main_window import MainWindow
from .theme import application_stylesheet


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Echo Chat native desktop client")
    parser.add_argument("--server", default="", help="Default server URL, for example http://127.0.0.1:3000")
    return parser


def create_application(argv: list[str] | None = None) -> tuple[QApplication, MainWindow]:
    arguments = list(sys.argv if argv is None else argv)
    options = build_parser().parse_args(arguments[1:])
    QCoreApplication.setOrganizationName("Echo Gate")
    QCoreApplication.setApplicationName("Echo Chat")
    app = QApplication(arguments)
    app.setStyle("Fusion")
    app.setStyleSheet(application_stylesheet())
    app.setQuitOnLastWindowClosed(False)
    window = MainWindow(options.server)
    app.setWindowIcon(window.windowIcon())
    return app, window


def main() -> int:
    app, window = create_application()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
