# Implementation Evidence: IAM-015
- Evidence date: `2026-07-21`
- Commit: uncommitted working tree based on `d0277f65fa8eb8632914b6b9dcaa4033cf260374`
- Requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contracts changed: no public contract; the internal `SessionRepository` port adds refresh and revoke plans with optimistic versioning and a stable `auth.session.revoked` audit action
- Migrations: none; IAM-011 migration `0002_sessions.sql` already contains the required lifecycle columns and constraints<br>Tests added: shared positive/negative/concurrent lifecycle cases plus all-row raw-storage/audit redaction checks on both engines
- Security review: refresh SQL is parameter-bound and requires the current version, active state, monotone activity, unexpired inactivity/absolute boundaries and unchanged issue/absolute limits. Revoke requires coherent actor/resource/time audit metadata and writes state plus audit in one transaction. No cookie, CSRF, digest or raw token enters an audit or returned secret field; the PostgreSQL service used the pinned disposable loopback image without committed credentials.
- Known limitations: recovery and bulk user-session revoke follow in `IAM-014`; HTTP conflict mapping, cookies, rotation, digest derivation, constant-time CSRF verification and rate limits follow in `IAM-012`. No independent clean-checkout/CI verdict exists, so `EVID-001` remains open.
- Reviewer verdict: builder-side review approved; no public contract drift, migration mutation, authorization claim, secret leak or observability change found.<br>Validator verdict: `full_local` passed against PostgreSQL 16.9 and SQLite; package is `implemented`, not `verified`.
## Test-first and validation commands
| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before implementation | 101 | Expected compile failure: lifecycle plan, audit action and repository methods did not exist. |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 | Six SQLite suites passed, including concurrency, expiry/revoke rejection and atomic rollback. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` before/after all-row aggregation | 101 / 0 | Multiple valid lifecycle audits exposed the old scalar subquery; aggregation now verifies every audit row on PostgreSQL. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full Rust workspace, both real engines and doctests passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit` | 0 / 0 / 0 / 0 | Formatting, warning-denied lint, policy and vulnerability gates passed; existing duplicate-version notices remain warnings. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Pinned install, 28 tool tests, machine contracts, 37 honestly planned bindings, architecture, spec index, tracking and generated drift passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain/security and all strict web gates passed. |
| `cargo build --workspace --all-features --release --locked`; `git diff --check` | 0 / 0 | Locked release build and whitespace review passed. |
