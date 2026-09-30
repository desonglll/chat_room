#!/usr/bin/env bash
#
# Worktree lifecycle for Telegram-parity tasks.
#
#   ./scripts/tg-worktree.sh new TG-101 virtual-message-list
#   ./scripts/tg-worktree.sh list
#   ./scripts/tg-worktree.sh status TG-101
#   ./scripts/tg-worktree.sh check TG-101
#   ./scripts/tg-worktree.sh rm TG-101
#
# Every worktree compiles into the shared build directory from
# .cargo/config.toml. See docs/tg/agent-protocol.md for the rules this enforces.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKTREE_ROOT="${REPO_ROOT}/.claude/worktrees"
DEVLOG_DIR="${REPO_ROOT}/docs/devlog"
TEMPLATE="${DEVLOG_DIR}/_TEMPLATE.md"
BASE_BRANCH="${TG_BASE_BRANCH:-main}"

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }
upper() { printf '%s' "$1" | tr '[:lower:]' '[:upper:]'; }

# Reads a `- Field: value` line out of a devlog file.
devlog_field() {
  local file="$1" field="$2"
  sed -n "s/^- ${field}:[[:space:]]*//p" "$file" 2>/dev/null | head -1
}

# A task's devlog lives inside its worktree while the task is active, and in the
# main checkout once the branch has been merged and the worktree removed.
devlog_path() {
  local task="$1" id in_worktree
  id="$(lower "$task")"
  in_worktree="${WORKTREE_ROOT}/${id}/docs/devlog/${task}.md"
  if [ -f "$in_worktree" ]; then
    printf '%s' "$in_worktree"
  else
    printf '%s' "${DEVLOG_DIR}/${task}.md"
  fi
}

cmd_new() {
  local task_raw="${1:-}" slug="${2:-}"
  [ -n "$task_raw" ] || die "usage: $0 new <TASK-ID> <slug>"
  [ -n "$slug" ] || die "usage: $0 new <TASK-ID> <slug>  (slug describes the work, e.g. virtual-message-list)"

  local task id path branch devlog base_sha base_subject
  task="$(upper "$task_raw")"
  id="$(lower "$task")"
  path="${WORKTREE_ROOT}/${id}"
  branch="agent/${id}-${slug}"
  # The devlog lives inside the worktree so it can be committed together with
  # the code it describes, as the protocol requires.
  devlog="${path}/docs/devlog/${task}.md"

  [ -e "$path" ] && die "worktree already exists: ${path}"
  git -C "$REPO_ROOT" show-ref --verify --quiet "refs/heads/${branch}" &&
    die "branch already exists: ${branch}"
  [ -f "$TEMPLATE" ] || die "missing devlog template: ${TEMPLATE}"

  base_sha="$(git -C "$REPO_ROOT" rev-parse --short "$BASE_BRANCH")"
  base_subject="$(git -C "$REPO_ROOT" log -1 --format=%s "$BASE_BRANCH")"

  mkdir -p "$WORKTREE_ROOT"
  git -C "$REPO_ROOT" worktree add -b "$branch" "$path" "$BASE_BRANCH" >/dev/null

  if [ -f "$devlog" ]; then
    printf 'devlog already exists, leaving it untouched: %s\n' "$devlog"
  else
    sed \
      -e "s|<TASK-ID>|${task}|g" \
      -e "s|<BRANCH>|${branch}|g" \
      -e "s|<WORKTREE>|.claude/worktrees/${id}|g" \
      -e "s|<BASE-COMMIT>|${base_sha} (${base_subject})|g" \
      -e "s|<DATE>|$(date '+%Y-%m-%d')|g" \
      "$TEMPLATE" >"$devlog"
    printf 'created devlog: %s\n' "${devlog#$REPO_ROOT/}"
  fi

  cat <<EOF

worktree  ${path}
branch    ${branch}
base      ${base_sha} (${base_subject})
build dir shared — see .cargo/config.toml

Before writing code:
  1. Read docs/tg/README.md, docs/tg/agent-protocol.md, and your task card in
     docs/tg/roadmap.md.
  2. Fill in Owner, Allowed paths, Migration prefix, and the Frozen Interface
     section of docs/devlog/${task}.md. Do this first, not at the end.
  3. Claim the task in docs/tg/board.md on the ${BASE_BRANCH} branch, not here.
  4. cd ${path}

Before ending any session, however short:
  ./scripts/tg-worktree.sh check ${task}
  and rewrite the "Handoff snapshot" section of docs/devlog/${task}.md.
EOF
}

cmd_list() {
  [ -d "$WORKTREE_ROOT" ] || { printf 'no worktrees\n'; return 0; }
  printf '%-10s %-12s %-40s %s\n' TASK STATUS BRANCH 'LAST DEVLOG UPDATE'
  local dir task devlog status branch updated
  for dir in "$WORKTREE_ROOT"/*/; do
    [ -d "$dir" ] || continue
    task="$(upper "$(basename "$dir")")"
    devlog="$(devlog_path "$task")"
    status='(no devlog)'
    branch='?'
    updated='-'
    if [ -f "$devlog" ]; then
      status="$(devlog_field "$devlog" Status)"
      branch="$(devlog_field "$devlog" Branch)"
      updated="$(date -r "$devlog" '+%Y-%m-%d %H:%M' 2>/dev/null || echo '-')"
    fi
    printf '%-10s %-12s %-40s %s\n' "$task" "${status:-?}" "${branch:-?}" "$updated"
  done
}

cmd_status() {
  local task="${1:-}"
  [ -n "$task" ] || die "usage: $0 status <TASK-ID>"
  task="$(upper "$task")"
  local devlog
  devlog="$(devlog_path "$task")"
  [ -f "$devlog" ] || die "no devlog for ${task} in either the worktree or the main checkout"
  # Print the handoff snapshot only: that is what an incoming agent needs.
  awk '/^## Handoff snapshot/{p=1} /^## /{if(p && !/^## Handoff snapshot/) exit} p' "$devlog"
}

cmd_check() {
  local task="${1:-}"
  [ -n "$task" ] || die "usage: $0 check <TASK-ID>"
  task="$(upper "$task")"
  local id path devlog failed=0
  id="$(lower "$task")"
  path="${WORKTREE_ROOT}/${id}"
  devlog="$(devlog_path "$task")"
  [ -d "$path" ] || die "no worktree: ${path}"

  printf '== devlog completeness ==\n'
  if [ ! -f "$devlog" ]; then
    printf 'FAIL  %s does not exist\n' "${devlog#"$REPO_ROOT"/}"
    failed=1
  else
    local field
    for field in Owner Status 'Base commit' 'Allowed paths'; do
      if [ -z "$(devlog_field "$devlog" "$field")" ]; then
        printf 'FAIL  devlog field empty: %s\n' "$field"
        failed=1
      fi
    done
    if grep -q '<fill in>' "$devlog"; then
      printf 'FAIL  devlog still contains <fill in> placeholders\n'
      failed=1
    fi
    if grep -qE '^- Next concrete action:[[:space:]]*$' "$devlog"; then
      printf 'FAIL  "Next concrete action" is empty — the next agent cannot start\n'
      failed=1
    fi
    [ "$failed" -eq 0 ] && printf 'ok\n'
  fi

  # Audit the worktree's own tree, not the main checkout.
  printf '\n== migration parity ==\n'
  if (cd "$path" && python3 scripts/check_migration_parity.py) 2>&1; then
    printf 'ok\n'
  else
    printf 'FAIL\n'
    failed=1
  fi

  printf '\n== file sizes ==\n'
  if (cd "$path" && python3 scripts/check_file_sizes.py) 2>&1; then
    printf 'ok\n'
  else
    printf 'FAIL\n'
    failed=1
  fi

  printf '\n== cargo fmt ==\n'
  if (cd "$path" && cargo fmt --all -- --check); then printf 'ok\n'; else printf 'FAIL\n'; failed=1; fi

  printf '\n== cargo clippy (shares the build lock; may block) ==\n'
  if (cd "$path" && cargo clippy --all-targets -- -D warnings); then printf 'ok\n'; else printf 'FAIL\n'; failed=1; fi

  printf '\n'
  if [ "$failed" -eq 0 ]; then
    printf 'all checks passed. Run `cargo test --all-targets` and the web gates before handoff.\n'
  else
    printf 'checks failed. Record the failure in the devlog rather than leaving it undocumented.\n'
    return 1
  fi
}

cmd_rm() {
  local task="${1:-}"
  [ -n "$task" ] || die "usage: $0 rm <TASK-ID>"
  task="$(upper "$task")"
  local id path
  id="$(lower "$task")"
  path="${WORKTREE_ROOT}/${id}"
  [ -d "$path" ] || die "no worktree: ${path}"

  if [ -n "$(git -C "$path" status --porcelain)" ]; then
    die "worktree has uncommitted changes; commit or discard them yourself first: ${path}"
  fi
  git -C "$REPO_ROOT" worktree remove "$path"
  printf 'removed %s\n' "$path"
  printf 'branch %s is kept; the devlog survives on it. Set Status: merged or abandoned before removing.\n' \
    "agent/$(lower "$task")-*"
}

case "${1:-}" in
  new) shift; cmd_new "$@" ;;
  list) shift; cmd_list "$@" ;;
  status) shift; cmd_status "$@" ;;
  check) shift; cmd_check "$@" ;;
  rm) shift; cmd_rm "$@" ;;
  *)
    cat <<'EOF'
usage: scripts/tg-worktree.sh <command>

  new <TASK-ID> <slug>   create worktree + branch + devlog from template
  list                   every worktree with its devlog status
  status <TASK-ID>       print the handoff snapshot (read this before resuming)
  check <TASK-ID>        devlog completeness + migration parity + fmt + clippy
  rm <TASK-ID>           remove a clean worktree, keep branch and devlog

Environment:
  TG_BASE_BRANCH   branch to fork from (default: main)
EOF
    exit 1
    ;;
esac
