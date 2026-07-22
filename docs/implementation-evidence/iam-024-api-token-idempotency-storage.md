# Implementation Evidence: IAM-024

- Evidence date: `2026-07-22`
- Base commit: `21385c569faf4839713c8cf57c33469ae35b4db7`; the package change is uncommitted.
- Requirements: `PRD-API-003`, `PRD-IAM-001`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contracts changed: no; the public contract from `SPEC-020` is unchanged.
- Migrations: none; immutable PostgreSQL/SQLite migration `0005_api_token_idempotency.sql` is reused.
- Tests added: one shared engine contract covering identical replay, request-hash conflict, audit failure rollback, exact concurrent create, 24-hour expiry reuse and bounded cleanup.
- Package size: the final diff contains 741 handwritten insertions and stays below the 800-line limit.
- Behavior: each engine reserves the actor/method/path/key tuple inside the create transaction, then commits the token, exactly one redacted audit event and the complete encrypted replay record together. Identical requests return the stored envelope; a different hash returns `KeyReused` without token or audit effect.
- Security review: SQL values are bound; actor identity is matched to the audit actor; only versioned nonce/ciphertext crosses persistence; custom Debug boundaries remain redacted. Ordinary token reads and audit records cannot return replay ciphertext or plaintext.
- Data review: no schema drift; UUIDv7 token references, result version and UTC-microsecond expiry retain PostgreSQL/SQLite parity. Cleanup is bounded to 200 rows per call and never deletes the token.
- Known limitations: Patch/Revoke idempotency is `IAM-026`; management/Bearer use cases are `IAM-025`; HTTP serialization, CSRF and production composition are `IAM-013`.
- Builder verdict: `implemented`.
- Reviewer verdict: local contract, transaction, authorization-context, redaction and diff review passed; independent review remains pending.
- Validator verdict: all local working-tree gates passed against real PostgreSQL 16.9 and SQLite; no independent commit-bound/CI verdict exists, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before implementation | 1 | Failed to compile because the idempotency repository types and methods did not exist. |
| Same focused SQLite command after implementation | 0 | Shared Create replay/conflict/concurrency/rollback/expiry/cleanup contract passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` before/after starting the pinned service | 1 / 0 | Initial `PoolTimedOut` was not counted as a pass; real PostgreSQL 16.9 then passed the same contract. |
| Focused `cargo clippy -p takt-application -p takt-persistence --all-targets --all-features -- -D warnings` before/after boxing the large result variant | 1 / 0 | The new `large_enum_variant` warning was fixed; focused lint passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed against both engines, including five CLI process tests. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Full Rust static gates passed. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | License/source/advisory policy and locked release build passed; configured duplicate warnings remain non-blocking. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned install, tools and machine contracts passed; all 37 product scenarios remain honestly planned. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, indexes, tracking, generated drift and secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed without exception. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 | Web lint/types/unit/build/browser and whitespace gates passed. |

PostgreSQL used the repository-pinned `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` image on loopback port 55432.
