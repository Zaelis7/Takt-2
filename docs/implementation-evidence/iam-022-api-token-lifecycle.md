# Implementation Evidence: IAM-022
- Evidence date: `2026-07-21`; base commit: `c6c2981`
- Requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contract/migration delta: no public contract or migration change; this package completes the lifecycle repository ports built on committed migration `0004`.
- Behavior: optimistic Patch and Revoke with immutable privilege-bearing fields; monotonic Last-used; revoked/expired/backdated/stale/replayed writes fail without state changes; successful Patch/Revoke increments version.
- Security review: Patch/Revoke validate token identity, organization/project, actor presence, action and timestamp before atomically writing one redacted audit event; duplicate-audit failures roll state back; Last-used cannot reactivate an invalid token and writes no bearer material.
- Known limits: public CRUD, Idempotency-Key storage, signed cursor encoding, conditional session CSRF, one-time token response, Bearer hash verification and Scope enforcement remain in `IAM-013`.<br>Verdict: focused PostgreSQL/SQLite validation passed, but the required full Workspace gate is externally blocked by Windows Smart App Control; package remains `in_progress`, no commit was created.

## Test-first and validation
| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before/after implementation | 101 / 0 | Missing lifecycle adapter failed first; final SQLite lifecycle contract passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | Real PostgreSQL 16.9 lifecycle parity passed; SQLite focused contract also passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo test --workspace --all-features -- --test-threads=1` | 0 / 0 / 101 | All Rust tests through PostgreSQL/SQLite passed; five CLI integration cases then failed because Code Integrity blocked `target/debug/takt-server.exe` before process start (OS 4551). |
| `cargo test -p takt-server --test admin_bootstrap_cli -- --test-threads=1`; direct `target/debug/takt-server.exe --help` | 101 / blocked | Reproduction confirmed Enterprise signing policy `{0283ac0f-fff1-49ae-ada1-8a933130cad6}` in `Microsoft-Windows-CodeIntegrity/Operational` event 3077. |
| Remaining full Rust and Node/Web gate groups | not run | Stopped at the explicit non-package gate blocker as required; no success is claimed. |
