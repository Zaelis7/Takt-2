# IAM-042 Implementation Evidence – idempotent API-token mutations

- Evidence date: `2026-07-23`
- Base commit: `95e2a0722e90684e12cdc313fea4136e5922f93f`; the package change is uncommitted.
- Requirements: `PRD-API-003`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`
- Finding: `SPEC-008` was opened during contract review and is not resolved by this package.
- Contracts changed: no public machine-readable contract changed.
- Migrations: none; published migrations remain unchanged.
- Tests added: `prd_api_003_idempotent_api_token_mutations_are_safe_and_actor_bound` covers Browser/Bearer authorization, organization/project context, exact Bearer scope, actor/method/path/hash binding, Patch/Revoke replay, hash/version conflicts, truthful audit attribution and secret-negative outputs.
- Package size: 516 handwritten Source-/Test insertions (`application` 179, Application tests 337), below the 550-line estimate and 800-line hard limit. Tracking and this evidence are excluded as specified.
- Behavior: Patch and Revoke authorize and validate the target before constructing a truthful actor-bound audit and calling the existing atomic mutation-idempotency repository. A fresh result returns the safe token projection; an identical immediate replay returns the same projection without another mutation or audit. Repository result ID/version drift fails closed.
- Security review: browser read-only, foreign-project and Bearer read-only actors fail before mutation. Bearer audit/idempotency use the authenticating API-token identity. Mutation outputs, audits, contexts and errors contain neither the raw token nor its hash or Create replay ciphertext.
- Data review: no schema or repository behavior changed. The Application layer consumes the existing PostgreSQL-/SQLite-parity atomic Patch/Revoke contract and preserves typed version/idempotency conflicts.
- Authorization review: browser writes require the existing management `Write` permission and matching organization/project; Bearer writes require the exact `api_tokens:write` scope and matching organization/project.
- Observability review: request IDs continue into audit events; no new logs, metrics or traces were added and no sensitive value crosses an observability boundary.
- Known limitation: architecture chapter 01 requires an identical idempotency replay to return the original stored status, relevant headers and body for 24 hours. The existing Patch/Revoke persistence stores only token ID and result version, so after a later token mutation the original safe Patch body cannot be reconstructed. The Application fails closed instead of returning current data as an old replay. `SPEC-008` and planned package `IAM-043` require the missing OpenAPI semantics and forward-only safe-response snapshot before `IAM-041` exposes mutation HTTP routes.
- Acceptance status: the exact 37-entry acceptance inventory remains valid, but all bindings remain planned. The v0.1 runner correctly rejects all 15 planned v0.1 scenarios.
- Builder verdict: `implemented`.
- Reviewer verdict: local authorization, context, actor attribution, idempotency, version, audit, redaction, contract-drift and diff review passed; the replay-after-later-mutation gap is explicitly tracked as `SPEC-008`.
- Validator verdict: all available local working-tree gates passed against real PostgreSQL 16.9 and SQLite. No independent commit-bound or CI verdict exists, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-application --test api_tokens prd_api_003_idempotent_api_token_mutations_are_safe_and_actor_bound -- --exact` before implementation | 1 | The behavior test failed to compile because the idempotent mutation commands and service methods did not exist. |
| `cargo test -p takt-application --test api_tokens prd_api_003_idempotent_api_token_mutations_are_safe_and_actor_bound -- --exact`; `cargo test -p takt-application --all-features` | 0 / 0 | Focused Patch/Revoke behavior and all ten API-token/four credential Application tests passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Formatting and all Rust targets passed without warnings. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed PostgreSQL 16.9, SQLite, server runtime, five CLI process tests and doctests. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust license/source/advisory policy, advisory scan and optimized locked build passed; configured duplicate-version findings remain warnings. |
| `pnpm install --frozen-lockfile`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 / 0 | Pinned install, web lint/types/unit test/build and Chromium bootstrap test passed. |
| `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 | 33 tool tests, OpenAPI/Schema/Proto/Gherkin contracts and the exact 37-entry acceptance inventory passed. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, 16-entry specification index, 57 requirements/108 packages/14 findings, generated drift and 163-file secret scan passed. |
| `pnpm audit --audit-level=high`; `pnpm check:licenses` | 0 / 0 | No known Node vulnerability and both production/full dependency license sets passed. |
| `pnpm acceptance:run -- --release v0.1` | 1 | Expected release-readiness failure: all 15 v0.1 scenarios remain planned rather than runnable. |

## Handoff

`IAM-043` is the next high-priority unblocked 0.1 package: add the explicit Patch/Revoke replay contract and an engine-parity safe response snapshot so a replay remains identical after later mutations. `IAM-040` may then add Create HTTP, while `IAM-041` remains dependency-gated on `IAM-043`; production write composition follows in `IAM-013`.
