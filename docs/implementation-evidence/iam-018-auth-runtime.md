# Implementation Evidence: IAM-018

- Evidence date: `2026-07-21`; base commit: `387abf3`; requirements: `PRD-API-001`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`.
- Runtime delta: the server composition now adapts the framework-free browser-auth service to real login/session/logout HTTP routes backed by the shared repository.
- Security: Argon2 stays off async workers; unknown accounts use a production-cost dummy hash; cookies are HttpOnly/SameSite=Lax/Path=/ and Secure outside local mode; missing or wrong CSRF cannot revoke a session.
- Limits/verdict: permissions remain empty until the specified permission engine, login throttling follows in `IAM-012`, and recovery/API tokens remain separate; `full_local` passed, so `implemented`, not independently `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-server browser_authentication_runtime_rotates_and_revokes_session -- --exact` before implementation | 101 | Failed to compile because `RuntimeAuthentication` did not exist. |
| `cargo test -p takt-server tests::browser_authentication_runtime_rotates_and_revokes_session -- --exact`; `cargo test -p takt-api --test auth_http` | 0 / 0 | Real SQLite HTTP composition, cookie/CSRF/revoke path and production cookie flags passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace, PostgreSQL 16.9 and SQLite passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, audit and release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository gates passed; 37 bindings remain planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, web, browser and whitespace gates passed. |
