# Repository Agent Rules

Use this file for repository-wide coordination. Product scope, task IDs, dependencies,
and acceptance criteria live in `docs/product-roadmap-and-agent-plan.md`.

## Start A Task

1. Read `CONTEXT.md`, this file, and the assigned task card in the roadmap.
2. Start from the integration lead's named green commit, not from an arbitrary dirty tree.
3. Work on one task ID in one branch/worktree named `agent/<task-id>-<slug>`.
4. Record the task's allowed paths, migration version, and public interface before editing.
5. Preserve user changes and unrelated work. Escalate an overlapping path to the
   integration lead instead of resolving ownership implicitly.

Completion criterion: the task has one owner, one base commit, an explicit file set,
and checkable acceptance criteria before code changes begin.

## Ownership

- Feature agents own new files inside their assigned domain module.
- The integration agent exclusively owns these shared hotspots during parallel waves:
  `src/lib.rs`, `src/models.rs`, `web/src/App.vue`, `web/src/router.ts`,
  `web/src/types.ts`, `web/src/api.ts`, `Cargo.toml`, `Cargo.lock`,
  `web/package.json`, the Web lockfile, `.github/workflows/*`, and this file.
- Add domain request/response types beside the domain implementation. Do not add new
  feature types to `src/models.rs` or `web/src/types.ts` merely for convenience.
- Add domain-specific browser clients such as `notificationsApi.ts`; do not expand
  `web/src/api.ts` during a parallel wave.
- A feature agent may prepare a small integration patch or handoff note for a shared
  hotspot, but the integration agent applies it after isolated modules are green.
- One agent owns a feature vertically when its database behavior and transport contract
  must change together. Split backend and frontend only after the contract is frozen.

Completion criterion: no two active agents have write ownership of the same existing
file, migration version, database table, or public interface.

## Database Changes

- Create matching SQLite and PostgreSQL migrations in the same task.
- Use the task family's reserved migration prefix from the roadmap and increment it.
- Never edit an applied migration. Add a forward migration instead.
- The same version and semantic name must exist in `migrations/` and
  `migrations-postgres/`.
- The migration owner also owns rollback/compatibility reasoning and tests for both
  database adapters.

Completion criterion: both fresh-schema and upgrade-path tests pass for SQLite and
PostgreSQL, with no duplicate migration version.

## Module Design

- Keep `Room` as the authorization and knowledge-isolation boundary.
- Original messages remain the source of truth; indexes, notifications, summaries, and
  AI output are projections.
- Put behavior behind a small domain interface. HTTP and WebSocket handlers translate
  protocol data and call that interface; they do not duplicate domain rules.
- Introduce a seam only when there are at least two real adapters, normally production
  and a test substitute.
- Test observable behavior through the module interface. Do not widen a production
  interface only so an integration test can reach private storage methods.
- Reuse existing project patterns and dependencies. New dependencies require an owner,
  a present need, and an integration-lead review.

## Change Discipline

- Keep edits inside the assigned task. Avoid unrelated renames, formatting, or cleanup.
- Hand-written source files warn at 350 physical lines and block at 500. Split a file by
  responsibility before adding behavior to a blocking file.
- Never make a generic `utils`, `helpers`, or `common` dumping ground.
- Preserve authorization checks at read time as well as write time.
- Do not log tokens, passwords, capability URLs, provider keys, private message bodies,
  or retrieved AI evidence.
- Do not modify generated output, vendored code, lockfiles, or existing migrations unless
  the task explicitly owns them.

## Verification And Handoff

Run the checks relevant to the task and report the exact result. The full release gate is
listed in the roadmap. Every handoff must include:

- task ID and base commit;
- changed files and migrations;
- interface/schema changes;
- commands run and pass/fail counts;
- screenshots for user-facing changes when browser tooling is available;
- unresolved risks and an explicit integration patch list for shared hotspots.

Completion criterion: another agent can integrate the task without reconstructing its
intent, ownership, contract, or verification state from the diff.
