## Implementation Evidence

- Evidence date: `2026-07-20`
- Target: uncommitted `SPEC-012` working tree based on commit `53be57ab890391c887719a7eaa380cde8e4770d6`, layered on the uncommitted implemented `SPEC-010` change; no commit, push or pull request was requested
- Requirements: `PRD-API-001`, `PRD-API-002`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-005`
- Contracts changed: yes; `specs/contracts/openapi.yaml` adds browser login, logout, session and password-recovery operations, explicit session-cookie/CSRF/request-ID/rate-limit headers, bounded closed input schemas and redacted session output; `web/src/generated/openapi.ts` was regenerated
- Migrations: none
- Tests added: `tools/openapi-auth-contract.test.mjs` verifies endpoint security overrides, CSRF, cookie policy, response sets, generic recovery, request/rate-limit headers, password/token bounds, write-only inputs, session expiry defaults and absence of credential/session identifiers from JSON output
- Security review: anonymous endpoints are limited to login and one-time recovery; session/logout accept only the session cookie, logout requires a session-bound CSRF header, invalid login and recovery requests do not reveal account existence, password/recovery inputs are closed and write-only, and JSON session responses omit password, recovery token and opaque session ID
- Known limitations: this is a contract-only package; no auth route is exposed by the runtime, API-token/monitor/SSE/notification/status-page gaps remain under `SPEC-003`, recovery-token delivery and the audited local fallback belong to `IAM-010` and later runtime packages, no released OpenAPI baseline exists for an automated compatibility comparison, and independent clean-checkout validation remains blocked by the uncommitted target plus `EVID-001`
- Reviewer verdict: builder-side diff review passed for the scoped additive contract change. The review found no runtime, migration, authorization implementation or secret-storage change to approve; anonymous/authenticated boundaries, CSRF, cookie clearing, generic account responses, response redaction and generated-code drift are covered. Independent review is still pending.
- Validator verdict: **not passed as a repository-wide verdict**. Every available focused, contract, Rust-without-PostgreSQL, supply-chain and web gate passed, but the mandatory full workspace test exited 101 because `TAKT_TEST_POSTGRES_URL` is not configured and neither a running Docker daemon nor `pg_isready` is available. The target is also uncommitted, so clean-checkout validation cannot yet be performed. `SPEC-012` therefore remains `implemented`, never `verified`.

### Package split

The original `SPEC-012` combined five resource families and would have exceeded the package limit against the existing 1,200-line OpenAPI document. Tracking now keeps auth/session/recovery in `SPEC-012` and defers dependency-gated API-token, monitor-operation/SSE, notification-channel and status-page contracts to `SPEC-015` through `SPEC-018`. Runtime packages depend on the corresponding contract package, so contracts are added immediately before implementation rather than speculatively.

### Test-first evidence and commands

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` (before split) | 0 | 57 requirements, 75 packages and 8 findings were valid. |
| `pnpm check:tracking` (after split) | 0 | 57 requirements, 79 packages and 8 findings were valid with only `SPEC-012` in progress. |
| `node --test tools/openapi-auth-contract.test.mjs` (initial test-first run) | 1 | Both tests failed because auth paths and schemas did not exist. |
| `node --test tools/openapi-auth-contract.test.mjs` (expiry/generic-error refinement) | 1 | The test exposed missing generic-login and default-expiry contract text. |
| `node --test tools/openapi-auth-contract.test.mjs` (header/rate-limit refinement) | 1 | The test exposed generic 429 responses without the required `Retry-After` and response request-ID headers. |
| `node --test tools/openapi-auth-contract.test.mjs` (rate-limit-code review refinement) | 1 | Final contract review exposed that 429 still referenced the generic Problem schema and did not guarantee `rate_limit_exceeded`. |
| `node --test tools/openapi-auth-contract.test.mjs` | 0 | Both focused auth-contract tests passed. |
| `pnpm contracts:openapi` | 0 | Redocly accepted the OpenAPI 3.1 document. |
| `pnpm generate:openapi` | 0 | TypeScript API types were regenerated from the contract. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web generated artifacts had no drift. |
| `pnpm test:tools` | 0 | Ten tool tests passed, including both auth contract tests. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | The full workspace passed Clippy with warnings denied. |
| `cargo test --workspace --all-features -- --test-threads=1` | 101 | All suites reached before the mandatory PostgreSQL contract passed; that contract stopped because `TAKT_TEST_POSTGRES_URL` is missing. This is not counted as a pass. |
| `cargo test --workspace --all-features --exclude takt-persistence -- --test-threads=1` | 0 | All non-persistence workspace tests passed. |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 | All six shared SQLite migration, repository and bootstrap cases passed. |
| `cargo deny check` | 0 | Dependency policy passed; duplicate-version notices remained warnings. |
| `cargo audit` | 0 | No RustSec vulnerability caused a failure. |
| `cargo build --workspace --all-features --release` | 0 | Release build passed. |
| `pnpm install --frozen-lockfile` | 0 | The pinned Node dependency graph installed without lockfile changes. |
| `pnpm contracts:validate` | 0 | OpenAPI, JSON Schema, Proto and Gherkin syntax validation passed. |
| `pnpm check:architecture` | 0 | Architecture constraints passed. |
| `pnpm check:tracking` | 0 | 57 requirements, 79 packages and 8 findings validated; final state is 5 implemented, 72 planned and 2 blocked packages. |
| `pnpm check:secrets` | 0 | Secret scanning passed for 97 source files. |
| `pnpm check:licenses` | 0 | Production and development Node license policy passed. |
| `pnpm audit --audit-level high` | 0 | No high-or-higher Node advisory caused a failure. |
| `pnpm lint` | 0 | Web lint passed with zero warnings. |
| `pnpm typecheck` | 0 | Strict TypeScript checking passed. |
| `pnpm test --run` | 0 | The web unit test passed. |
| `pnpm build` | 0 | The production web bundle built successfully. |
| `pnpm playwright test` | 0 | The Chromium bootstrap accessibility smoke test passed. |
| `git diff --check` | 0 | No whitespace errors were found after the final evidence update. |
