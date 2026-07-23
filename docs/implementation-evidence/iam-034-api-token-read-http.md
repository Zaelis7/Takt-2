# Implementation Evidence: IAM-034

- Evidence date: `2026-07-23`
- Base commit: `5af9ce932219615f31e78a5034f8e6febcef7385`; the package change is uncommitted.
- Requirements: `PRD-API-001`, `PRD-API-004`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`
- Contracts changed: no; the existing API-token List/Get, credential, Problem, cursor and ETag contracts remain unchanged.
- Migrations: none; immutable migration `0004_api_tokens.sql` already provides the shared PostgreSQL/SQLite read model.
- Tests added: one framework-free Application read test and one real SQLite/Axum production-composition test covering List/Get, filters, ETag, session/Bearer actors, context, scope, credential, CSRF-stability and secret-redaction behavior.
- Package size: 792 handwritten Source-/Test insertions plus two manifest lines, below the 800-line hard limit; lockfile, tracking and this evidence are excluded as specified.
- Behavior: production List/Get authenticates exactly one read credential, maps it to a context-bound actor, applies `api_tokens:read` exactly for Bearer tokens, evaluates status at one injected clock instant and performs stable lookahead pagination through the existing repository.
- Security review: browser reads validate an active session without CSRF rotation; Bearer validation remains prefix/Argon2id/status/IP based and generic on credential failure. Organization/project checks occur before returning data. The cursor key is generated from operating-system randomness per process and is never logged. Responses and Problems expose neither raw tokens nor hashes.
- Architecture review: the server depends on API, Application and Persistence only; shared domain IDs and token enums cross the public Application boundary, preserving the enforced workspace dependency direction.
- Data review: no schema or SQL changed. SQLite exercises the full HTTP composition, while the unchanged shared repository contract passes on real PostgreSQL 16.9 and SQLite with the same filter, ordering and cursor boundary.
- Known limitations: Create/Patch/Revoke HTTP, browser write-CSRF and HTTP idempotency remain in `IAM-013`; the general role/permission engine and other public resources remain later packages.
- Builder verdict: `implemented`.
- Reviewer verdict: local contract, authorization, redaction, architecture and diff review passed; independent review remains pending.
- Validator verdict: all local working-tree gates passed against real PostgreSQL 16.9 and SQLite. No independent commit-bound or CI verdict exists, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-application --test api_tokens prd_iam_004_api_token_reads_require_exact_scope_and_context` before implementation | 101 | Failed to compile because `ApiTokenReadActor` and `ApiTokenReadService` did not exist. |
| `cargo test -p takt-application --test api_tokens`; `cargo test -p takt-api --test api_tokens_http`; `cargo test -p takt-server --bin takt-server api_token_read_runtime_enforces_scope_context_and_redaction` | 0 / 0 / 0 | Application, HTTP boundary and real SQLite production-composition behavior passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | Real PostgreSQL 16.9 passed the unchanged shared migration/repository contract. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Full Rust formatting and lint passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | The final full workspace run passed both engines, all server runtime tests and all five CLI process tests. Earlier attempts were not counted because Windows Smart App Control rejected freshly built test/build-script binaries with OS code 4551 before execution; the unchanged final binary and full command subsequently ran successfully. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | License/source/advisory policy and the locked optimized build passed; configured duplicate-version findings remain warnings. |
| `pnpm install --frozen-lockfile`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm test:tools` | 0 / 0 / 0 / 0 | Pinned install, machine contracts, all 37 honestly planned acceptance bindings and repository tool tests passed. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, indexes, tracking, generated drift and secret scan passed. The first architecture run correctly rejected a direct server-to-domain dependency; the boundary was corrected before completion. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed without exception. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 | Web lint/types/unit/build/browser and whitespace gates passed. |

PostgreSQL used the repository-pinned `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` image on loopback port 55432.
