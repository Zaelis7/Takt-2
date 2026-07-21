# Implementation Evidence: IAM-023
- Evidence date: `2026-07-21`; base commit: `719bd9a`
- Requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Migration delta: forward-only PostgreSQL/SQLite `0004_api_tokens.sql`; latest supported schema advances from 3 to 4 and all newer-schema fixtures advance coherently.
- Behavior: atomic create plus one redacted audit event; safe ID/prefix lookup; project/kind/status/scope filters; signed-cursor-ready `(created_at, id)` boundary and stable descending order on both engines.
- Security review: all SQL values are bound and query shape/order are static; raw bearer values never enter the repository; schema and code accept only a separate safe prefix plus Argon2id PHC hash; direct non-Argon2 writes fail and audit metadata contains neither hash nor credential fixture.
- Known limits: optimistic Patch/Revoke/Last-used and their rollback evidence follow in `IAM-022`; HTTP idempotency, signed cursor encoding, one-time response and Bearer authentication follow in `IAM-013`.<br>Verdict: builder review and `full_local` validation passed; `implemented`, not independently `verified`; `EVID-001` remains open.

## Test-first and validation
| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before/after implementation | 101 / 0 | Missing constants/repository implementation failed first; the final shared SQLite contract passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1`; same environment with `cargo test --workspace --all-features -- --test-threads=1` | 0 / 0 | Real PostgreSQL 16.9 parity and full workspace passed without skip. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust, policy, vulnerability and locked release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository tool, contract, architecture, tracking and generated gates passed; all 37 acceptance bindings remain honestly planned. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, secret, web, browser and whitespace gates passed. |
