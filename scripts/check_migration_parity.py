#!/usr/bin/env python3
"""Check paired SQLite/PostgreSQL migration versions and semantic names."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SQLITE_DIR = ROOT / "migrations"
POSTGRES_DIR = ROOT / "migrations-postgres"
PAIRED_HISTORY_START = 20260818000012
MIGRATION_NAME = re.compile(r"^(\d{14})_(.+)\.sql$")


def migrations(directory: Path) -> tuple[dict[int, str], list[str]]:
    found: dict[int, str] = {}
    errors: list[str] = []
    for source in sorted(directory.glob("*.sql")):
        match = MIGRATION_NAME.fullmatch(source.name)
        if match is None:
            errors.append(f"{source.relative_to(ROOT)}: expected <14-digit-version>_<name>.sql")
            continue
        version = int(match.group(1))
        if version in found:
            errors.append(f"{directory.relative_to(ROOT)}: duplicate migration version {version}")
        found[version] = match.group(2)
    return found, errors


def main() -> int:
    sqlite, errors = migrations(SQLITE_DIR)
    postgres, postgres_errors = migrations(POSTGRES_DIR)
    errors.extend(postgres_errors)
    paired_versions = sorted(
        {version for version in sqlite | postgres if version >= PAIRED_HISTORY_START}
    )

    for version in paired_versions:
        sqlite_name = sqlite.get(version)
        postgres_name = postgres.get(version)
        if sqlite_name is None:
            errors.append(f"{version}: missing from migrations/")
        elif postgres_name is None:
            errors.append(f"{version}: missing from migrations-postgres/")
        elif sqlite_name != postgres_name:
            errors.append(
                f"{version}: semantic name differs ({sqlite_name!r} vs {postgres_name!r})"
            )

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        print(f"migration parity failed: {len(errors)} error(s)", file=sys.stderr)
        return 1

    print(
        f"migration parity passed: {len(paired_versions)} paired versions from "
        f"{PAIRED_HISTORY_START}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
