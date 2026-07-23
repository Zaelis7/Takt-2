# Implementation Evidence: IAM-025

- Evidence date: `2026-07-23`
- Base commit: `863c00f8dfc77c06170e9fd26a5bae6275857344`
- Requirements: `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`
- Contracts changed: no; `specs/contracts/openapi.yaml` and the v0.1 acceptance scenario remain unchanged.
- Migrations: none.
- Tests added: `prd_iam_001_api_token_crud_enforces_permission_context_and_audit` in `crates/application/tests/api_tokens.rs` covers Create/List/Get/Patch/Revoke, Read/Write denial, organization/project separation, audit actions/context and raw-token redaction.
- Package size: 741 handwritten Source-/Test insertions (`Cargo.toml` 3, application 353, tests 385); the one-line `Cargo.lock` update is generated. This is below the 800-line limit.
- Behavior: a framework-free management service receives an authenticated management actor, explicit organization/project target and typed command; it authorizes before secret generation or repository access, checks returned resources again, uses injected Clock/ID/secret/hash ports, and builds redacted Create/Patch/Revoke audit plans. Only the Create result contains the zeroizing token wrapper.
- Security review: no raw token enters audit, list/get/update/revoke output, repository lookup input, logs or error text. Project-scoped actors cannot access organization-wide or other-project targets; organization, project and exact Read/Write capability are checked in Application rather than relying only on repository filters. Repository and infrastructure failures remain typed and are never reclassified as target failures.
- Data review: no schema, migration or repository-semantic change. Existing PostgreSQL/SQLite repository contracts passed unchanged.
- Known limitations: Bearer prefix/hash/status/IP/scope authentication and monotonic `last_used_at` composition moved to `IAM-029`; HTTP Problems, idempotency composition, CSRF, signed cursors and production routing remain in `IAM-013`; the general role-to-permission engine remains future work. All 37 product acceptance bindings remain planned.
- Builder verdict: implemented.
- Reviewer verdict: local spec/diff review found the combined CRUD/Bearer slice would exceed the package limit, so it was split before implementation; no remaining local blocker found. Independent review is pending.
- Validator verdict: full local working-tree validation passed without skipped repository gates. Independent clean-checkout and CI validation are pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-application --test api_tokens` before implementation | 101 | The behavior test failed to compile because the management actor, commands and service did not exist. |
| `cargo test -p takt-application --test api_tokens` after implementation | 0 | Six API-token tests passed, including CRUD permission/context/audit/redaction behavior. |
| `cargo clippy -p takt-application --all-targets --all-features -- -D warnings` | 0 | Focused Application lint passed. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | Every Rust target passed without warnings. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed against pinned PostgreSQL 16.9 and SQLite, including five CLI process tests and doctests. |
| `cargo deny check`; `cargo audit` | 0 / 0 | Policy and vulnerability gates passed; configured duplicate-version warnings remain non-blocking. |
| `cargo build --workspace --all-features --release --locked` | 0 | Locked optimized workspace build passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools` | 0 / 0 | Pinned Node workspace and all 32 tool tests passed. |
| `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 | OpenAPI/schema/Proto/Gherkin contracts and all 37 bindings are valid; acceptance truthfully reports 37 planned, 0 runnable, 0 verified. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, spec index, 97-package tracking DAG, generated drift and secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | No known Node vulnerability and all installed dependency licenses passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Web lint, strict types, Vitest, production build and Chromium bootstrap passed. |
| `git diff --check` | 0 | Final whitespace and patch-integrity check passed after tracking/evidence updates. |

## Incomplete orchestration attempt

The first combined Rust-gate shell invocation was terminated by its 1-second shell timeout with exit 124 before any gate result was available. The identical commands were immediately repeated with a sufficient timeout and completed with exit 0 as recorded above; the interrupted invocation is not counted as a pass or failure of an individual gate.
