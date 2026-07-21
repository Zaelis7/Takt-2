# Implementation Evidence: SPEC-015

- Evidence date: `2026-07-21`; base commit: `2361f5e`; requirements: `PRD-API-001` through `PRD-API-005` and `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`.
- Contract delta: `/api/v1/api-tokens` now defines list/create/get/patch/revoke with cursor filters, idempotency, ETag/If-Match, bearer/session auth, conditional browser CSRF and explicit success/problem responses.
- Security: the opaque value exists only in `ApiTokenCreated`; safe/page schemas expose only a non-authenticating prefix. Scopes and actor bindings are immutable after creation; expiry, IP networks and metadata remain bounded.
- Limits/verdict: no runtime is claimed; persistence, slow hash/prefix lookup, bearer authentication, scope checks and audit follow in `IAM-013`. `full_local` contract validation passed, so `implemented`, not independently `verified`; `SPEC-003` remains open for other resource families.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `node --test tools/openapi-token-contract.test.mjs` before/after | 1 / 0 | Missing paths/schemas failed first; final CRUD, concurrency and secret-isolation assertions passed. |
| `pnpm contracts:openapi`; `pnpm generate:openapi`; `pnpm check:generated`; `pnpm test:tools` | 0 / 0 / 0 / 0 | OpenAPI is valid, generated TypeScript is current, and all tool/contract tests pass. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace, PostgreSQL 16.9 and SQLite passed unchanged. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, audit and release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking` | 0 / 0 / 0 / 0 / 0 / 0 | Repository gates passed; 37 bindings remain planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, web, browser and whitespace gates passed. |
