import json

from echo_chat.realtime import RealtimeClient


class FakeSignal:
    def __init__(self) -> None:
        self.callbacks = []

    def connect(self, callback) -> None:
        self.callbacks.append(callback)

    def emit(self, *args) -> None:
        for callback in self.callbacks:
            callback(*args)


class FakeSocket:
    def __init__(self, _parent) -> None:
        self.connected = FakeSignal()
        self.disconnected = FakeSignal()
        self.text_received = FakeSignal()
        self.error = FakeSignal()
        self.urls: list[str] = []
        self.sent: list[dict[str, object]] = []
        self.connected_state = False
        self.aborted = 0

    def open(self, url) -> None:
        self.urls.append(url.toString())

    def close(self) -> None:
        self.connected_state = False

    def abort(self) -> None:
        self.aborted += 1
        self.connected_state = False

    def is_connected(self) -> bool:
        return self.connected_state

    def send_text(self, message: str) -> None:
        self.sent.append(json.loads(message))


def test_fake_websocket_adapter_covers_auth_room_frames_and_send() -> None:
    sockets: list[FakeSocket] = []

    def factory(parent) -> FakeSocket:
        socket = FakeSocket(parent)
        sockets.append(socket)
        return socket

    client = RealtimeClient(socket_factory=factory)
    account, room = sockets

    client.connect_account("https://chat.example/base", "secret")
    assert account.urls == ["wss://chat.example/base/ws/account"]
    account.connected_state = True
    account.connected.emit()
    assert account.sent == [{"token": "secret"}]

    client.open_room("room-1", "room-password", has_password=True)
    assert room.urls == ["wss://chat.example/base/ws/room-1"]
    room.connected_state = True
    room.connected.emit()
    assert room.sent == [{"type": "auth", "token": "secret", "password": "room-password"}]
    assert client.send_message("hello", "message-1")
    assert room.sent[-1] == {"type": "message", "content": "hello", "reply_to": "message-1"}

    client.shutdown()
