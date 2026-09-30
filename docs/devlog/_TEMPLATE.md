# <TASK-ID> <fill in: task title from docs/tg/roadmap.md>

- Task: <TASK-ID>
- Owner: <fill in: agent or person currently holding this>
- Worktree: <WORKTREE>
- Branch: <BRANCH>
- Base commit: <BASE-COMMIT>
- Status: in-progress
- Migration prefix: <fill in: reserved prefix from roadmap, or `none`>
- Allowed paths: <fill in: globs this task may write, copied from the task card>

<!--
  Two sections behave differently and this is the whole point of the format:

  * "Progress log" is APPEND-ONLY. Never rewrite history there.
  * "Handoff snapshot" is OVERWRITTEN every time. It always describes the
    present, never the past. An incoming agent reads only that section and can
    start working.

  Rules that make handoff actually work (docs/tg/agent-protocol.md):
  - Update the handoff snapshot before ending any session, even a 10-minute one.
  - "Next concrete action" names a file and an operation. "continue development"
    and "keep going on the list" are failures.
  - Commit this file together with the code it describes, in the same commit.
  - Only this worktree writes this file. Never edit another task's devlog.
-->

## Frozen interface

<!--
  The contract other tasks may depend on: HTTP shapes, WebSocket frames, store
  slice shapes, exported component props, DB columns. Changing anything here
  after another task starts consuming it requires an entry in "Decisions" and a
  note in docs/tg/board.md.
-->

<fill in>

## Decisions

<!--
  Every non-obvious choice, with the reason. The reason is the part that
  survives; "switched to X" without "because Y" is worthless to the next agent
  and will be re-litigated.
-->

- <DATE> <fill in: chose X over Y because Z>

## Progress log

<!-- Append. Newest at the bottom. -->

### <DATE>

<fill in: what was done, what state the code is in>

## Handoff snapshot

<!-- OVERWRITE this whole section each time. Present tense only. -->

- Done: <fill in>
- In progress: <fill in: name the file and function, not the feature>
- Next concrete action: <fill in: e.g. "packages/web/src/features/messageList/useAnchor.ts — implement restoreAnchor() for the prepend path; the append path already works">
- Known broken: <fill in, or `nothing known`>
- Verification: <fill in: exact commands run and their last result, e.g. "cargo test --all-targets → 412 passed; bun test → 68 passed; cargo clippy → clean">
- Do not touch: <fill in: paths another task owns that this one must leave alone>

## Blockers and dependencies

- <fill in, or `none`>

## Residual risk

<!-- What a reviewer should distrust. Empty is a valid answer only if you looked. -->

- <fill in, or `none identified`>
