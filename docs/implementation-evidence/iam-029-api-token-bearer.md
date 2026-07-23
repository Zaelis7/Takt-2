# Implementation Evidence: IAM-029

- Evidence date: `2026-07-23`
- Base commit: `863c00f8dfc77c06170e9fd26a5bae6275857344`; change is not yet committed.
- Requirements: `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`
- Contracts changed: no; `specs/contracts/openapi.yaml` and the v0.1 token-scope acceptance scenario remain unchanged.
- Migrations: none.
- Tests added: `prd_iam_001_bearer_authentication_fails_closed_and_records_use` in `crates/application/tests/api_tokens.rs` covers malformed and unknown values, same-prefix wrong hashes, expiry, revoke, source-IP denial, exact scopes, organization/project separation, Last-used and raw-secret redaction.
- Package size: 222 Source-/Test insertions beyond the completed `IAM-025` slice (73 application, 149 test), below the 300-line preflight estimate.
- Behavior: `ApiTokenBearerAuthenticationService` parses a zeroizing token wrapper, derives its non-secret lookup prefix, checks the repository prefix postcondition and verifies the Argon2id hash before status/IP evaluation. Only then does it construct a typed token actor and record successful use. `authorize_token_actor` applies exact organization, optional project restriction and exact scope at the Application boundary.
- Security review: invalid, missing, expired, revoked, IP-mismatched, stale-during-use and wrongly hashed values return the same authentication failure and never an Actor. Database/hash infrastructure failures are not mistaken for credentials or target failures. Raw token and slow hash remain absent from Debug, errors, audit, tracking and evidence.
- Data review: no schema or repository-semantic change; the existing monotonic PostgreSQL/SQLite Last-used contract is composed unchanged.
- Known limitations: HTTP Authorization parsing, trusted source-IP derivation, Problem serialization, rate limits, production async Argon2 adapter and router composition remain in `IAM-013`; monitor routes and the runnable v0.1 scope scenario remain in `API-010` and later packages. All 37 product acceptance bindings remain planned.
- Builder verdict: implemented.
- Reviewer verdict: local spec/diff review found no remaining blocker; independent review is pending.
- Validator verdict: full local working-tree validation passed after starting the documented pinned PostgreSQL test service. Independent clean-checkout and CI validation are pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-application --test api_tokens` before implementation | 101 | The behavior test failed to compile because the Bearer authentication service and token-actor authorization function did not exist. |
| `cargo test -p takt-application --test api_tokens` after implementation | 0 | Seven API-token tests passed, including all Bearer acceptance and negative cases. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Rust format and all-target lint gates passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` before the test service was started | 101 | The PostgreSQL contract alone failed after 30 seconds with `PoolTimedOut`; no pass was claimed. |
| `docker run --rm --detach --name takt-postgres-test -p 127.0.0.1:55432:5432 -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=takt_test postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7`; same full workspace test | 0 / 0 | The documented pinned PostgreSQL 16.9 service became ready; the full workspace passed against PostgreSQL and SQLite without skip. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust policy, advisories and locked optimized build passed; configured duplicate-version warnings remain non-blocking. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned workspace, 32 tool tests and all contracts/bindings passed; acceptance still reports 37 planned and 0 runnable. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, spec index, tracking DAG, generated drift and secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Frontend lint, strict types, Vitest, production build and Chromium bootstrap passed. |
| `git diff --check` | 0 | Patch whitespace and integrity passed again after the final tracking/evidence update. |
