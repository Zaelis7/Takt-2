# Implementation Evidence: IAM-038

- Evidence date: `2026-07-23`
- Base commit: `5af9ce932219615f31e78a5034f8e6febcef7385`; the package change and preceding `IAM-034` change are uncommitted.
- Requirements: `PRD-API-003`, `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contracts changed: no public contract changed. The internal audit contract now types user and API-token actor IDs and uses separate redacted metadata variants.
- Migrations: forward-only PostgreSQL/SQLite migration `0006_api_token_actor.sql`; published migrations `0001` through `0005` remain unchanged.
- Tests added: one shared engine contract for API-token actor-bound idempotent Create/replay and audit readback, plus engine-specific mutually exclusive actor, foreign-key and redaction schema negatives.
- Package size: 676 handwritten Source-/Test/Migration/Documentation insertions plus eight overlapping Application import/construction lines, 684 total and below the 800-line hard limit; tracking and this evidence are excluded as specified.
- Behavior: an API-token authenticated write can reserve idempotency with `actor_type=api_token` and atomically persist a matching audit event that references the authenticating token rather than a fabricated user or system actor. Identical replay preserves the stored actor and appends no event.
- Security review: database checks make user and API-token actor columns mutually exclusive; the token actor has a restrictive foreign key; repository validation binds actor type/ID, organization/project metadata and idempotency context before the transaction. Metadata and Debug output contain no raw token, Argon2id hash or replay plaintext.
- Data review: both engines expose the same typed domain projection. PostgreSQL adds the token reference and checks in place; SQLite rebuilds the two affected tables in forward migration `0006`, copies all existing columns, and restores indexes and append-only triggers. Schema version detection advances from 5 to 6 and rejects version 7.
- Known limitations: Browser/Bearer write-plan construction follows in `IAM-039`; Create and Patch/Revoke HTTP boundaries follow in `IAM-040`/`IAM-041`; production write composition remains `IAM-013`.
- Builder verdict: `implemented`.
- Reviewer verdict: local actor-binding, migration, foreign-key, redaction and diff review passed; independent review remains pending.
- Validator verdict: all local working-tree gates passed against real PostgreSQL 16.9 and SQLite. No independent commit-bound or CI verdict exists, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before implementation | 101 | Failed to compile because `ApiToken`, `AuditActorId` and `AuditMetadata` actor variants/types did not exist. |
| Same focused command; `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` after implementation | 0 / 0 | The focused API-token actor behavior and all six SQLite migration/repository/bootstrap tests passed. |
| PostgreSQL focused command before configuration; same command after starting the pinned service | 1 / 0 | The first command stopped because `TAKT_TEST_POSTGRES_URL` was absent and was not counted as a pass; real PostgreSQL 16.9 then passed the same shared actor contract and schema negatives. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Full Rust formatting and lint passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed both engines, server runtime/CLI tests and doctests. |
| `cargo deny check`; `cargo audit` | 0 / 0 | License/source/advisory policy and Rust advisory scan passed; configured duplicate-version findings remain warnings. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned install, all 33 tool tests, machine contracts and all 37 honestly planned acceptance bindings passed. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, indexes, 106-package tracking, generated drift and secret scan passed. The first combined run stopped at tracking because this required Evidence file had not yet been created; it was not counted as a pass and the complete rerun succeeded. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed without exception. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Web lint, strict types, unit test, production build and Chromium accessibility smoke passed. |
| `cargo build --workspace --all-features --release --locked`; `git diff --check` | 0 / 0 | Locked optimized workspace build and whitespace review passed. |

PostgreSQL used the repository-pinned `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` image on loopback port 55432.
