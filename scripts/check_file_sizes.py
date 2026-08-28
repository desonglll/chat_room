#!/usr/bin/env python3
"""Enforce repository source-file size limits with an explicit warning baseline."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

WARNING_LINES = 350
FAILURE_LINES = 500
SOURCE_SUFFIXES = {
    ".bash",
    ".css",
    ".js",
    ".jsx",
    ".py",
    ".rs",
    ".sh",
    ".sql",
    ".toml",
    ".ts",
    ".tsx",
    ".vue",
    ".yaml",
    ".yml",
}

ROOT = Path(__file__).resolve().parents[1]
BASELINE_PATH = ROOT / "scripts" / "file-size-baseline.json"


def repository_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    return [
        ROOT / name.decode()
        for name in result.stdout.split(b"\0")
        if name
        and (ROOT / name.decode()).is_file()
        and Path(name.decode()).suffix.lower() in SOURCE_SUFFIXES
    ]


def physical_lines(source: Path) -> int:
    return len(source.read_bytes().splitlines())


def main() -> int:
    baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    failures: list[str] = []
    warnings: list[str] = []
    observed: set[str] = set()

    for source in repository_files():
        relative = source.relative_to(ROOT).as_posix()
        lines = physical_lines(source)
        if lines < WARNING_LINES:
            continue

        observed.add(relative)
        baseline_limit = baseline.get(relative)
        if lines >= FAILURE_LINES:
            failures.append(f"{relative}: {lines} lines reaches the {FAILURE_LINES}-line hard limit")
        elif baseline_limit is None:
            failures.append(f"{relative}: {lines} lines is a new {WARNING_LINES}-line threshold violation")
        elif lines > baseline_limit:
            failures.append(f"{relative}: grew from baseline {baseline_limit} to {lines} lines")
        else:
            warnings.append(f"{relative}: {lines} lines (baseline {baseline_limit})")

    for relative in sorted(set(baseline) - observed):
        failures.append(f"{relative}: baseline is stale; remove it after the file drops below {WARNING_LINES} lines")

    for warning in warnings:
        print(f"warning: {warning}")
    if failures:
        for failure in failures:
            print(f"error: {failure}", file=sys.stderr)
        print(
            f"file-size audit failed: {len(failures)} error(s), {len(warnings)} baseline warning(s)",
            file=sys.stderr,
        )
        return 1

    print(f"file-size audit passed: {len(warnings)} baseline warning(s), no growth")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
