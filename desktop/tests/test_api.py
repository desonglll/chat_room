import pytest

from echo_chat.api import api_error_message, normalize_server_url


def test_normalize_server_url_accepts_local_server_and_removes_query() -> None:
    assert normalize_server_url("127.0.0.1:3000/") == "http://127.0.0.1:3000"
    assert normalize_server_url("https://chat.example.com/base/?ignored=1") == (
        "https://chat.example.com/base"
    )


def test_normalize_server_url_rejects_non_http_protocol() -> None:
    with pytest.raises(ValueError):
        normalize_server_url("file:///tmp/chat")


def test_api_error_prefers_server_message() -> None:
    raw = b'{"message":"room name already exists"}'
    assert api_error_message("create-room", 422, "", raw) == "room name already exists"
