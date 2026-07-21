# Implementation Evidence: IAM-020

- Evidence date: `2026-07-21`; base commit: `b7ba5d40efc2f48185945ba1dfeb01d278a91fe5`; requirements: `PRD-API-005`, `PRD-IAM-001`.
- Contract delta: no OpenAPI change; the existing login/session/logout paths now have injectable bounded handlers and transport DTOs without forbidden workspace dependencies.
- Security: unknown user/wrong password are identical; unknown JSON fields and oversized bodies fail before use cases; credential/token DTO debug is redacted and response caching disabled.
- Limits/verdict: production composition and success/cookie/CSRF E2E follow in `IAM-018`, rate limits in `IAM-012`; `full_local` passed, so `implemented`, not independently `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-api --test auth_http login_failure_is_generic_and_contract_shaped -- --exact` before/after | 101 / 0 | Missing port/router/config failed first; final real-HTTP boundary passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace, PostgreSQL 16.9 and SQLite passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, audit and release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository gates passed; 37 bindings remain planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, web, browser and whitespace gates passed. |
