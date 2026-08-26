#!/usr/bin/env python3
"""Fail when the stable Web entry bundle reaches the 500 KB release limit."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENTRY_BUNDLE = ROOT / "web" / "dist" / "assets" / "app.js"
MAX_BYTES = 500_000


def main() -> int:
    if not ENTRY_BUNDLE.is_file():
        print(f"error: missing {ENTRY_BUNDLE.relative_to(ROOT)}; run `bun run build` first")
        return 1
    size = ENTRY_BUNDLE.stat().st_size
    relative = ENTRY_BUNDLE.relative_to(ROOT)
    if size >= MAX_BYTES:
        print(f"error: {relative} is {size} bytes; entry bundle must stay below {MAX_BYTES} bytes")
        return 1
    print(f"web bundle check passed: {relative} is {size} bytes (< {MAX_BYTES})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
