## Implementation Evidence

- Evidence date: `2026-07-21`
- Target: uncommitted `IAM-010` working tree based on commit `5cb6dace3ec75241205b6ad0ed8b9d133490e8e8`; the worktree was clean before package activation, and no commit, push or pull request was requested
- Requirements: `PRD-API-001`, `PRD-API-002`, `PRD-API-005`, `PRD-IAM-001`
- Contracts changed: yes; `specs/contracts/openapi.yaml` now assigns stable generic Problem codes to browser-auth failure classes, and `web/src/generated/openapi.ts` is regenerated from that contract
- Migrations: none
- Tests added: `crates/domain/tests/session_policy.rs`; `tools/openapi-auth-contract.test.mjs` extended with response and stable-code assertions
- Security review: no session ID, cookie, password, recovery token or CSRF token value enters the domain model; CSRF is represented only as boundary-verified proof. Unknown accounts and wrong passwords share `authentication_failed`; invalid recovery tokens share `recovery_failed` without revealing existence, expiry or prior use. Input and response schemas remain bounded and secret fields remain write-only. No database, audit, logging, telemetry or external-data-flow change occurred.
- Known limitations: this package defines contracts and pure rules only. Session/recovery persistence, hashing and one-time consumption follow in `IAM-011`; HTTP routes, cookie construction, constant-time CSRF verification, rate limiting, recovery delivery/local fallback and audit orchestration follow in `IAM-012`. API-token contracts/runtime remain `SPEC-015`/`IAM-013`, so `SPEC-003` remains open. The working tree is uncommitted and has no independent clean-checkout or CI verdict, so `EVID-001` remains open.
- Reviewer verdict: builder-side diff review approved after refining the recovery-completion `400` response to preserve both `invalid_request` for malformed input and generic `recovery_failed` for an unusable token. No contract drift, secret exposure, persistence, authorization-runtime or observability change was found. Independent review is pending.
- Validator verdict: all required local repository gates passed on the working tree, including the real PostgreSQL 16.9 contract without skips. This is `full_local` evidence, not independent or CI validation; `IAM-010` is therefore `implemented`, not `verified`.

### Scope and test-first evidence

The package stayed within its preflight boundary: one public error-contract refinement plus pure session-domain rules, with no migration, runtime endpoint or credential storage. Initial failing tests established both behavior slices before implementation. The final change adds about 650 hand-written lines including tests, tracking and evidence (generated TypeScript excluded), below the 800-line hard limit; splitting it would separate the two acceptance bullets of the registered package.

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` after activating `IAM-010` | 0 | 57 requirements, 82 packages and 9 findings valid; exactly `IAM-010` was in progress. |
| `cargo test -p takt-domain --test session_policy` before implementation | 101 | Failed to compile because the session domain module did not exist. |
| `node --test tools/openapi-auth-contract.test.mjs` before contract refinement | 1 | Failed because auth operations still referenced the generic Problem response. |
| `cargo test -p takt-domain --test session_policy` | 0 | Six expiry, refresh, overflow, rotation/revoke and CSRF cases passed. |
| `node --test tools/openapi-auth-contract.test.mjs` | 0 | Two browser-auth contract tests passed. |
| `pnpm contracts:openapi` | 0 | Redocly accepted the OpenAPI 3.1 contract. |
| `pnpm generate:openapi` | 0 | TypeScript API types regenerated from the contract. |
| `cargo test -p takt-domain --all-features` | 0 | All domain and doctest suites passed. |
| `pnpm test:tools` | 0 | All 28 repository tool tests passed. |
| `pnpm contracts:validate` | 0 | OpenAPI, JSON Schema, Proto, Gherkin syntax and CheckSpec contracts passed. |
| `pnpm acceptance:check` | 0 | All 37 scenarios remained exactly inventoried; all are still honestly planned, not reported as product acceptance. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web artifacts had no drift. |
| `pnpm check:architecture` | 0 | Dependency directions and unsafe-code guards passed. |
| `pnpm check:spec-index` | 0 | All 16 indexed specification paths resolved. |
| `pnpm check:tracking` | 0 | 57 requirements, 82 packages and 9 findings validated with `IAM-010` implemented. |
| `pnpm install --frozen-lockfile` | 0 | Pinned Node dependencies were already current. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | Full Rust lint passed with warnings denied. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed against SQLite and the pinned real PostgreSQL 16.9 service. |
| `cargo deny check` | 0 | Advisory, ban, license and source policies passed; existing duplicate-version notices remained warnings. |
| `cargo audit` | 0 | No known Rust vulnerabilities found. |
| `pnpm audit --audit-level high` | 0 | No high-or-higher Node advisory caused a failure. |
| `pnpm check:licenses` | 0 | Production and development Node license policies passed. |
| `pnpm check:secrets` | 0 | Secret scan passed for 114 source files. |
| `pnpm lint` | 0 | Web lint passed with zero warnings. |
| `pnpm typecheck` | 0 | Strict TypeScript checking passed. |
| `pnpm test --run` | 0 | Web unit tests passed. |
| `pnpm build` | 0 | Production web bundle built successfully. |
| `pnpm playwright test` | 0 | Chromium accessibility smoke test passed. |
| `cargo build --workspace --all-features --release --locked` | 0 | Pinned optimized workspace build passed. |
| `git diff --check` | 0 | No whitespace errors were found. |

### Validation service

The PostgreSQL suite used the repository-pinned disposable loopback service `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` with database `takt_test`. No external database or committed test secret was used.
