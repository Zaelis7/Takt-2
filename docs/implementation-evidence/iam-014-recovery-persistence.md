# Implementation Evidence: IAM-014
- Evidence date: `2026-07-21`; base commit: `a8df4b9960726278db093703956e1c0bb68831f5`
- Requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contract/migration delta: no public API change; internal `RecoveryRepository` plus forward-only PostgreSQL/SQLite `0003_recovery_tokens.sql`.
- Behavior: issue and completion audit atomically; completion replaces the Argon2id hash, revokes every active user session and permits exactly one unexpired consumer.
- Security review: SQL is bound; storage, debug output and audit expose no raw token/digest. Rollback, replay, expiry, constraints and concurrency are tested on both engines.
- Known limits: token generation/delivery, generic HTTP recovery, cookies/CSRF and rate limits follow in `IAM-012`; API tokens follow in `SPEC-015`/`IAM-013`.<br>Verdict: builder review and `full_local` validation passed; `implemented`, not independently `verified`; `EVID-001` remains open.

## Test-first and validation
| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before/after implementation | 101 / 0 | Missing recovery port failed first; final shared SQLite contract passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1`; same environment with `cargo test --workspace --all-features -- --test-threads=1` | 0 / 0 | PostgreSQL 16.9 contract and full workspace passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, vulnerability and locked release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository contract/tool gates passed; all 37 acceptance bindings remain honestly planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, web, browser and whitespace gates passed. |
