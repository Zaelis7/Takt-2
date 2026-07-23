# Implementation Evidence: IAM-039

- Evidence date: `2026-07-23`
- Base commit: `86d1d0929cde841b7d61a3de0bbbc59000bb02f7`; the package change is uncommitted.
- Requirements: `PRD-API-003`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`
- Contracts changed: no public machine-readable contract changed.
- Migrations: none; published migrations remain unchanged.
- Tests added: `prd_api_003_idempotent_api_token_create_binds_actor_context_and_secrets` covers Browser/Bearer authorization, organization/project context, truthful audit actors, identical encrypted secret/projection replay, key/hash conflict and redaction. The shared persistence contract additionally proves identical replay after token expiry but before idempotency expiry on PostgreSQL and SQLite.
- Package size: 731 handwritten Source-/Test insertions (`application` 388, Application tests 272, persistence 19, shared persistence tests 51, server exhaustive mapping 1), below the 800-line hard limit. Tracking and this evidence are excluded as specified.
- Split: the initial combined Create/Patch/Revoke implementation measured 876 handwritten insertions and exceeded the hard limit. `IAM-039` was narrowed to Create and the remaining Patch/Revoke orchestration was registered as dependency-correct `IAM-042` before completion.
- Behavior: Browser and exact `api_tokens:write` Bearer actors are authorized against organization/project context before side effects. Create atomically stores the token, one truthful audit event and an actor/method/path/key/hash-bound encrypted replay. Identical replay returns the original one-time token and safe creation projection without another write or audit; a reused key with another request hash fails typed.
- Security review: the raw token is zeroized at client-input, generation, replay-plaintext and ciphertext boundaries; Debug, errors and audit metadata remain redacted. AEAD additional data binds actor type/ID, method, path, key and request hash. Browser writes use the authenticated user identity; Bearer writes use the authenticating API-token identity. Read-only scopes and foreign project context fail before repository effects.
- Data review: no schema changed. Both engines now check an existing idempotency reservation before applying validation that is meaningful only for a new token, so an identical replay remains valid for the full 24-hour idempotency window even if the created token expired earlier. New reservations still validate token and audit data inside the transaction before any business insert.
- Known limitations: idempotent Patch/Revoke application orchestration follows in `IAM-042`; Create and Patch/Revoke HTTP boundaries follow in `IAM-040`/`IAM-041`; production write composition remains `IAM-013`. All 37 acceptance bindings remain planned, including all 15 v0.1 scenarios.
- Builder verdict: `implemented`.
- Reviewer verdict: local permission, actor attribution, replay, redaction, engine-parity and diff review passed; independent review remains pending.
- Validator verdict: all available local working-tree gates passed against real PostgreSQL 16.9 and SQLite. The v0.1 release-readiness runner correctly failed because all release scenarios remain planned. No independent commit-bound or CI verdict exists, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-application --test api_tokens prd_api_003_idempotent_api_token_writes_bind_actor_context_and_secrets -- --exact` before implementation | 101 | The behavior test failed to compile because the write actor, idempotent command/service and typed conflict did not exist. Two immediate retries were blocked by Windows Code Integrity (OS error 4551) before test execution and were not counted as results. |
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` with the old validation order | 1 | The new short-lived-token replay case reached the repository and failed with `Repository(ConstraintViolation)`, proving the regression before the ordering fix. |
| `cargo test -p takt-application --test api_tokens prd_api_003_idempotent_api_token_create_binds_actor_context_and_secrets -- --exact`; `cargo test -p takt-application --all-features`; `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 / 0 / 0 | Focused Create behavior, all nine API-token/four credential Application tests and all six SQLite contracts passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract postgres_migrations_repository_and_bootstrap_contracts -- --exact --test-threads=1` | 0 | Real PostgreSQL 16.9 passed the same shared replay-after-token-expiry contract. An earlier misspelled filter ran zero tests and is not counted as a pass. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Formatting and all Rust targets passed. The first full Clippy run found the missing exhaustive server mapping for the new error variant (exit 1); the fail-closed mapping was added and the complete rerun passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed PostgreSQL, SQLite, server runtime, five CLI process tests and doctests. |
| `cargo deny check`; `cargo audit` | 0 / 0 | License/source/advisory policy and Rust advisory scan passed; configured duplicate-version findings remain warnings. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned install, 33 tool tests, OpenAPI/Schema/Proto/Gherkin contracts and the exact 37-entry acceptance binding inventory passed. |
| `pnpm acceptance:run -- --release v0.1` | 1 | Release readiness correctly rejected all 15 v0.1 scenarios because they remain `planned`, not runnable. This is not counted as an acceptance pass. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, 16-path spec index, 107-package tracking DAG, generated drift and 161-file secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and production/development license gates passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Web lint, strict types, Vitest, production build and Chromium accessibility smoke passed. |
| `cargo build --workspace --all-features --release --locked`; `git diff --check` | 0 / 0 | Locked optimized workspace build and final whitespace review passed. |

PostgreSQL used the repository-pinned `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` image on loopback port 55432.
