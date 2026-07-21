# Implementation Evidence: IAM-012

- Evidence date: `2026-07-21`; base commit: `225a306165f4e2d286f0277759495e1428116310`; requirements: `PRD-API-005`, `PRD-IAM-001`.
- Runtime delta: login admission is fixed at ten attempts per 60-second window for both the socket peer IP and normalized account key; the eleventh attempt returns `429 rate_limit_exceeded` with `Retry-After`.
- Security: keys and total memory are bounded; unknown/known accounts use the same pre-auth path; invalid credentials add a non-blocking 100 ms step capped at 2 s. The formula is the narrow implementation assumption for the specified but otherwise undefined increasing delay.
- Limits/verdict: forwarded-IP headers remain untrusted until proxy networks are specified, recovery remains separate, and API tokens follow in `SPEC-015`/`IAM-013`; `full_local` passed, so `implemented`, not independently `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-api --test auth_http login_rate_limit_is_enforced_with_retry_after -- --exact` before/after | 101 / 0 | Attempt 11 returned 200 first; final real HTTP test passed with 429 and Retry-After. |
| `cargo test -p takt-api --all-features -- --test-threads=1` | 0 | Independent peer/account limits, bounded normalization, delay growth/cap/reset and the fixed production minimum passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace, PostgreSQL 16.9 and SQLite passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, audit and release gates passed after applying the package-local formatting correction. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository gates passed; 37 bindings remain planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, web, browser and whitespace gates passed. |
