# Implementation Evidence: IAM-026

- Evidence date: `2026-07-23`
- Base commit: `a58c16879eb1d02b2bb44a4b4f726aa1aed1b482`; the package change is uncommitted.
- Requirements: `PRD-API-003`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contracts changed: no; the OpenAPI Patch/idempotency contract and migration `0005` remain unchanged.
- Migrations: none; immutable PostgreSQL/SQLite migration `0005_api_token_idempotency.sql` already provides safe `(api_token_id, result_version)` references for Patch.
- Tests added: one shared engine contract covering identical Patch replay, request-hash conflict, audit failure rollback and retry, exact concurrent requests, and reuse at the 24-hour boundary.
- Package size: the code/test slice contains 659 handwritten insertions. The first combined Patch/Revoke attempt measured 840 insertions, so Revoke was removed and tracked as the dependent package `IAM-028` before completion.
- Behavior: each engine reserves the actor/method/path/key tuple inside the Patch transaction and commits the versioned update, exactly one redacted audit event and the safe result reference together. Identical requests return that stored reference; a different hash returns `KeyReused` without business or audit effect.
- Security review: all SQL values are bound; the context actor must match the audit actor; the method and full token path must match the mutated ID; no token secret or encrypted Create replay is read or written by Patch.
- Data review: no schema drift; UUIDv7 references, monotonic result versions, exact 24-hour expiry and UTC-microsecond storage are engine-parity tested.
- Known limitations: Revoke idempotency is `IAM-028`; management/Bearer use cases are `IAM-025`; HTTP serialization, CSRF and production composition are `IAM-013`.
- Builder verdict: `implemented`.
- Reviewer verdict: local contract, transaction, context-binding, redaction and diff review passed; independent review remains pending.
- Validator verdict: all local working-tree gates passed against real PostgreSQL 16.9 and SQLite. No independent commit-bound or CI verdict exists, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --test-threads=1` before implementation | 1 | Failed to compile because `SqlxRepository` did not implement the new idempotent Patch method. |
| Same focused SQLite command after implementation | 0 | The shared replay/conflict/concurrency/rollback/expiry contract passed. Two earlier execution attempts were rejected before test start by Windows Code Integrity (`os error 4551`) and were not counted as passes. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | Real PostgreSQL 16.9 passed the same shared contract. |
| `cargo clippy -p takt-application -p takt-persistence --all-targets --all-features -- -D warnings` | 0 | Focused application/persistence lint passed after the size split. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Full Rust formatting and lint passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | The repeated full workspace run passed both engines and all five CLI process tests. The first run was not a pass because Code Integrity blocked a rebuilt `auth_http` binary before execution. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | License/source/advisory policy and the locked optimized build passed; configured duplicate-version findings remain warnings. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned install, 32 tool tests and machine contracts passed; all 37 product scenarios remain honestly planned. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, indexes, 96-package tracking, generated drift and secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed without exception. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 | Web lint/types/unit/build/browser and whitespace gates passed. |

PostgreSQL used the repository-pinned `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` image on loopback port 55432.
