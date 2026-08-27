# Foundation delivery archive: FND-001 through FND-004

This record closes the foundation wave that preceded the public product
documentation. Runtime configuration facts intentionally remain in
`docs/configuration.md`; this file records ownership, outcomes, and verification
rather than duplicating operational instructions.

## Delivery chain

| Task | Branch | Green commit | Outcome |
| --- | --- | --- | --- |
| FND-001 | `agent/fnd-001-fix-ci-blockers` | `3bdc92a` | Established the clean integration baseline and removed obsolete tracked output. |
| FND-002 | `agent/fnd-002-close-wip` | `7a2a108` | Completed the interrupted rooms, favorites, AI, configuration, and browser work already present in the repository. |
| FND-003 | `agent/fnd-003-split-hotspots` | `8bad3c1` | Split oversized Web and Rust hotspots by responsibility without changing public contracts. |
| FND-004 | `agent/fnd-004-ci-gates` | `9fd11d7` | Added migration, upgrade, source-size, Web, desktop, SQLite, and real PostgreSQL release gates. |

## Public contract and schema

FND-001 introduced no contract. FND-002 completed the existing public room,
favorite, AI, and configuration behavior. FND-003 was a behavior-preserving
module split. FND-004 changed CI and test coverage only. No applied migration
was edited in the foundation wave.

## Verification at close

The FND-004 green commit passed formatting, Clippy with warnings denied, Rust
all-target/all-feature tests, the Vue unit/type/build suite, desktop Pytest and
Ruff checks, migration parity and upgrade tests for SQLite and PostgreSQL, and
the source-file size gate. The Rust suite completed with 145 passing tests and
no failures; the Web suite completed with 121 passing tests and no failures;
the desktop suite completed with 8 passing tests and no failures.

## Remaining integration patches

None. The next task starts from `9fd11d7`.
