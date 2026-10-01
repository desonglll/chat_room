"""TG-603: the desktop client speaks only the canonical `/api/chats` contract."""

from pathlib import Path


def test_no_deprecated_rooms_paths_in_the_client() -> None:
    source = Path(__file__).resolve().parents[1] / "src"
    offenders = [
        str(path.relative_to(source))
        for path in source.rglob("*.py")
        if "/api/" + "rooms" in path.read_text(encoding="utf-8")
    ]
    assert offenders == []
