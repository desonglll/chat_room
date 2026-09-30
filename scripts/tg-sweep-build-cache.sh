#!/usr/bin/env bash
#
# Prune the shared Cargo build directory.
#
#   ./scripts/tg-sweep-build-cache.sh          # report only
#   ./scripts/tg-sweep-build-cache.sh --prune  # actually delete
#
# Why this exists: the repository's build directory reached 194 GB before it was
# first pruned, against 456 GB free on the volume. A clean `cargo clippy
# --all-targets` pass produces about 1.5 GB, so essentially all of that was
# sediment from past builds and superseded toolchains. Every worktree shares one
# build directory (.cargo/config.toml), so nothing reclaims it automatically.
#
# Run this monthly, and after finishing a milestone.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
RETAIN_DAYS="${TG_SWEEP_RETAIN_DAYS:-14}"

# Read the directory out of the config rather than duplicating the path here.
TARGET_DIR="$(
  sed -n 's/^target-dir[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "${REPO_ROOT}/.cargo/config.toml" | head -1
)"
TARGET_DIR="${CARGO_TARGET_DIR:-$TARGET_DIR}"

[ -n "$TARGET_DIR" ] || {
  printf 'error: could not determine the build directory from .cargo/config.toml\n' >&2
  exit 1
}

if [ ! -d "$TARGET_DIR" ]; then
  printf 'build directory does not exist yet: %s\n' "$TARGET_DIR"
  exit 0
fi

printf 'build directory  %s\n' "$TARGET_DIR"
printf 'current size     %s\n' "$(du -sh "$TARGET_DIR" 2>/dev/null | cut -f1)"
printf 'volume free      %s\n' "$(df -h "$TARGET_DIR" | tail -1 | awk '{print $4}')"
printf 'retain           artifacts used within %s days\n\n' "$RETAIN_DAYS"

if ! command -v cargo-sweep >/dev/null 2>&1; then
  cat <<'EOF'
cargo-sweep is not installed, so this script can only report.

  cargo install cargo-sweep

Install it when no other worktree is building — `cargo install` compiles, and
concurrent Cargo invocations serialize on the shared build directory's lock.

Without cargo-sweep the only blunt alternative is deleting the whole directory
and paying a full cold rebuild (about 1m45s for `cargo clippy --all-targets`,
considerably longer for `--all-features` and the test binaries).
EOF
  exit 0
fi

if [ "${1:-}" = "--prune" ]; then
  # A worktree mid-build holds the lock; sweeping under it would delete
  # artifacts it is about to use.
  printf 'checking for active builds...\n'
  if pgrep -x rustc >/dev/null 2>&1; then
    printf 'error: rustc is running. Wait for the build to finish before pruning.\n' >&2
    exit 1
  fi
  cargo sweep --time "$RETAIN_DAYS" "$TARGET_DIR"
  printf '\nsize after       %s\n' "$(du -sh "$TARGET_DIR" 2>/dev/null | cut -f1)"
  printf 'volume free      %s\n' "$(df -h "$TARGET_DIR" | tail -1 | awk '{print $4}')"
else
  cargo sweep --dry-run --time "$RETAIN_DAYS" "$TARGET_DIR"
  printf '\nre-run with --prune to delete.\n'
fi
