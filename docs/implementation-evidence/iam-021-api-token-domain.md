# Implementation Evidence: IAM-021
- Evidence date: `2026-07-21`; base commit: `6b6622e`
- Requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`
- Contract delta: no public API or persistence change; this package implements the framework-free domain/application boundary under the already reviewed `SPEC-015` OpenAPI contract.
- Behavior: typed token IDs, kinds, prefixes, exact scopes, status and canonical IPv4/IPv6 CIDRs; source-IP/expiry/revoke authorization; 320 generated random bits split into a public 64-bit lookup prefix and 256 secret bits; Argon2id hash/verify boundary and repository plans for CRUD/authentication.
- Security review: raw token and slow hash use zeroizing, redacted wrappers; permission checks are exact-match and cannot derive `monitors:write` from `monitors:read`; malformed scopes, non-canonical CIDRs, expired/revoked/IP-mismatched tokens and unsafe metadata fail closed.
- Known limits: SQL storage, atomic write audit, idempotency, signed cursors, HTTP CRUD and production Bearer composition remain explicitly in `IAM-022`/`IAM-013`; the v0.1 monitor-scope scenario remains planned until `API-010` supplies monitor routes.<br>Verdict: builder review and `full_local` validation passed; `implemented`, not independently `verified`; `EVID-001` remains open.

## Test-first and validation
| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-application --test api_tokens` before/after implementation | 101 / 0 | Missing API-token modules and types failed first; four focused scope, CIDR, status/IP and secret/hash tests pass. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full workspace passed against PostgreSQL 16.9 and SQLite without skip. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 / 0 / 0 | Rust format, lint, policy, vulnerability and locked release gates passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Repository tool/contract gates passed; architecture was initially red for the missing test-crate unsafe guard, then passed after the scoped fix. |
| `pnpm audit --audit-level high`; `pnpm check:licenses`; `pnpm check:secrets`; `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test`; `git diff --check` | 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 / 0 | Supply-chain, secret, web, browser and whitespace gates passed. |
