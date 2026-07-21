# Implementation Evidence: IAM-016
- Evidence date: `2026-07-21`; base commit: `eeaf1e1`; requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`; no public contract or migration changed.
- Behavior/security: unknown user and wrong password share one error; 256-bit values rotate and only SHA-256 digests persist; CSRF comparison is constant-time; expiry/revoke block access; login/logout audit is secret-free.
- Limits/verdict: HTTP, cookies and rate limits remain in `IAM-012`; builder review and `full_local` passed, so `implemented`, not independently `verified`; `EVID-001` stays open.

## Test-first and validation
| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before/after | 101 / 0 | Missing auth types failed first; final SQLite contract passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1`; same environment with `cargo test --workspace --all-features -- --test-threads=1` | 0 / 0 | PostgreSQL 16.9 and the workspace passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, audit and release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository gates passed; 37 bindings remain planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, web, browser and whitespace gates passed. |
