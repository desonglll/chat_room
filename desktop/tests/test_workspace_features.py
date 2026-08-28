from echo_chat.models import Conversation, ConversationPreferences
from echo_chat.workspace_features import WorkspaceFeaturesMixin


class FakeSidebar:
    def __init__(self) -> None:
        self.items: list[Conversation] = []

    def set_conversations(self, items: list[Conversation]) -> None:
        self.items = items


class Workspace(WorkspaceFeaturesMixin):
    def __init__(self) -> None:
        self._conversations: dict[str, Conversation] = {}
        self._sidebar = FakeSidebar()


def conversation(
    room_id: str, activity: str, *, pinned: bool = False, archived: bool = False
) -> Conversation:
    return Conversation(
        room_id=room_id,
        kind="group",
        title=room_id,
        last_activity_at=activity,
        preferences=ConversationPreferences(room_id, pinned, archived),
    )


def test_conversations_sort_pinned_then_recent_then_archived() -> None:
    workspace = Workspace()
    workspace._conversations = {
        "recent": conversation("recent", "2026-08-28T10:00:00Z"),
        "pinned": conversation("pinned", "2026-08-20T10:00:00Z", pinned=True),
        "archived": conversation("archived", "2026-08-29T10:00:00Z", archived=True),
        "older": conversation("older", "2026-08-27T10:00:00Z"),
    }

    workspace._render_conversations()

    assert [item.room_id for item in workspace._sidebar.items] == [
        "pinned",
        "recent",
        "older",
        "archived",
    ]
