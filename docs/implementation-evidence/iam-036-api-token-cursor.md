# Implementation Evidence: IAM-036

- Evidence date: `2026-07-23`
- Base commit: `863c00f8dfc77c06170e9fd26a5bae6275857344`; change is not yet committed.
- Requirements: `PRD-API-004`, `PRD-API-005`
- Contracts changed: no; the OpenAPI cursor, filter, stable-sort and `invalid_cursor` contract from `SPEC-015` remains unchanged.
- Migrations: none.
- Tests added: `api_token_cursor_is_signed_and_filter_bound` covers roundtrip, the 2048-character bound, zero signing keys, malformed input, MAC tampering, filter changes, key-debug redaction and rejection of non-UUIDv7 boundaries. The module test checks the HMAC-SHA-256 implementation against a known vector.
- Package split: the originally selected `IAM-013` HTTP slice measured about 907 Source-/Test insertions before production composition and therefore exceeded the 800-line hard limit. It was split before completion into cursor foundation (`IAM-036`), Axum read boundary (`IAM-035`), production read composition (`IAM-034`) and the remaining write runtime (`IAM-013`).
- Package size: 215 Source-/Test lines (170 source, 45 test), below the 300-line preflight estimate.
- Behavior: `ApiTokenCursorKey` creates an opaque HMAC-SHA-256 cursor from a UTC-microsecond boundary and UUIDv7. The MAC covers the payload; a SHA-256 filter fingerprint canonically binds the fixed `created_at:desc,id:desc` order and all active `project_id`, `kind`, `status` and `scope` values.
- Security review: the 32-byte signing key rejects the all-zero configuration, is zeroized at final ownership release and has a fixed redacted `Debug` representation. Verification uses RustCrypto's constant-time MAC check. Length is rejected before allocation-heavy decoding, every parse or validation failure has one non-disclosing error, and no cursor or key is included in errors.
- Dependency review: direct use of maintained RustCrypto `hmac 0.13.0` and `sha2 0.11.0` is pinned at workspace level. Both are MIT/Apache-2.0 licensed and were already present transitively in `Cargo.lock`, so the package graph gains no new crate version. The standard library does not provide HMAC-SHA-256 or a constant-time verifier.
- Contract/data review: no public schema, response, migration, repository behavior, audit event or external data flow changed.
- Known limitations: this package is only the framework-free cursor codec. HTTP query/problem serialization and credential extraction follow in `IAM-035`; Application/Persistence adapters and production composition follow in `IAM-034`; writes remain in `IAM-013`. Cursor expiry is not introduced because the governing OpenAPI contract defines no expiry policy.
- Builder verdict: implemented.
- Reviewer verdict: local spec, dependency, security and diff review found no remaining blocker; independent review is pending.
- Validator verdict: full local working-tree validation passed against the pinned PostgreSQL test service and SQLite. Independent clean-checkout and CI validation are pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-api --test api_token_cursor` after adding non-v7 rejection but before implementation | 101 | The new negative behavior failed to compile because `encode` still returned an unconditional `String`; no pass was claimed. |
| `cargo test -p takt-api --lib`; `cargo test -p takt-api --test api_token_cursor` after implementation | 0 / 0 | The known HMAC vector and complete cursor behavior/negative test passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Rust format and all-target lint gates passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | The complete workspace passed against PostgreSQL 16.9 and SQLite without skip. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust policy, advisories and locked optimized build passed; configured duplicate-version warnings remain non-blocking. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned workspace, tool tests and all contracts/bindings passed; acceptance still reports 37 planned and 0 runnable. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, spec index, tracking DAG, generated drift and secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Frontend lint, strict types, Vitest, production build and Chromium bootstrap passed. |
| `git diff --check` | 0 | Patch whitespace and integrity passed after the final tracking/evidence update. |

Intermediate dependency-integration attempts that omitted the required `KeyInit` trait or passed the zeroizing wrapper instead of its byte slice failed compilation and were corrected before the focused and full gates; they are not reported as passes.
