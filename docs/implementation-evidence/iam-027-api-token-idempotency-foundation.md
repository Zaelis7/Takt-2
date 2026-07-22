# Implementation Evidence: IAM-027

- Evidence date: `2026-07-22`
- Base commit: `dc5b74be2cd8de66ea8621e825cb3b78eeea5d9c`; the package working-tree change is not yet committed.
- Requirements: `PRD-API-002`, `PRD-API-003`, `PRD-IAM-001`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contracts changed: no public contract changed. The internal application boundary now types the API-token write method, actor, path, idempotency key, request hash, exact expiry and encrypted replay envelope.
- Migrations: forward-only PostgreSQL and SQLite migration `0005_api_token_idempotency.sql`; both engines enforce the same identity tuple, result shape, encrypted-payload shape and exact 24-hour lifetime without a plaintext replay column.
- Tests added: application AEAD round-trip/AAD/tamper/redaction coverage, PostgreSQL/SQLite schema-constraint coverage, updated migration-version rejection and CLI migration-count coverage, plus a lockfile supply-chain regression for `fast-uri`.
- Package split: the initially selected `IAM-024` was first separated from Patch/Revoke (`IAM-026`), but its preview implementation still measured 888 handwritten insertions. Before completion it was split again into the bounded AEAD/schema foundation `IAM-027` and atomic Create persistence `IAM-024`; only `IAM-027` is delivered here.
- Package size: the final working-tree diff contains 716 handwritten insertions excluding generated lockfiles, below the 800-line hard limit. The 68-line `Cargo.lock` and five-line `pnpm-lock.yaml` generated changes are reported separately.
- Behavior: replay ciphertext is authenticated with length-delimited actor type/ID, method, path, idempotency key and request hash; it has a random 96-bit nonce, versioned key reference, 64-KiB plaintext limit and a checked exact 24-hour context expiry.
- Security review: raw idempotency keys, request hashes, key bytes and ciphertext are omitted from custom `Debug` output. Decryption fails closed for changed context or ciphertext. The database permits only complete encrypted Create payloads, rejects payloads on Patch/Delete rows and has no plaintext payload column.
- Dependency assessment: `chacha20poly1305 = 0.11.0` is the RustCrypto pure-Rust RFC 8439 implementation, dual MIT/Apache-2.0 licensed and compatible with the pinned Rust 1.95 toolchain. It adds the smallest direct authenticated-encryption boundary needed here; AES-GCM was considered, while the Rust standard library provides no AEAD. `cargo deny` and `cargo audit` validate its license, source and advisory graph; per-dependency binary impact was not measured.
- Supply-chain finding: the mandatory Node audit exposed existing `fast-uri 3.1.3` as affected by `GHSA-v2hh-gcrm-f6hx`. `SEC-002` pins `3.1.4`, tests the lockfile floor and retains a zero-exception green audit.
- Known limitations: no repository lookup/reservation/write, Create replay, expiry purge or concurrent conflict behavior exists yet; those remain in `IAM-024`. Patch/Revoke idempotency remains in `IAM-026`, application/Bearer orchestration in `IAM-025` and HTTP composition in `IAM-013`.
- Builder verdict: `implemented`.
- Reviewer verdict: local spec, migration, crypto-boundary and diff review completed; independent review remains pending.
- Validator verdict: all local working-tree gates passed; independent clean-checkout and CI validation remain pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| Focused shared SQLite repository test before the package split | 101 | The first behavior test did not compile because the idempotency types and operations were absent; its attempted complete implementation then exposed the package-size violation, so repository behavior was returned to `IAM-024`. |
| `cargo test -p takt-application --test api_tokens` | 0 | Five application tests passed, including replay encryption, context mismatch, tamper and Debug-redaction cases. |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 | All six SQLite migration and repository cases passed, including `0005` constraints. |
| Focused SQLite migration test during final diff review, before/after tightening the result-shape constraint | 101 / 0 | The negative test first proved that ciphertext without a token result was accepted; the revised migration permits only an all-null reservation or a complete token/version/ciphertext result. |
| Focused PostgreSQL contract before Docker Desktop was available | 101 | Failed with `PoolTimedOut`; the required database check was recorded as unavailable, not passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | Real PostgreSQL 16.9 from the repository-pinned image passed the `0005` schema contract. |
| First full workspace test after migration `0005` | 101 | Correctly exposed a stale server CLI assertion expecting four migrations; the assertion and newer-schema fixture were updated to five/six and the focused test passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Rust formatting and every lint target passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Entire workspace passed against real PostgreSQL 16.9 and SQLite, including all CLI process tests. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust license/source/advisory policies and the locked optimized build passed; configured duplicate-version warnings remain non-blocking. |
| `node --test tools/node-supply-chain.test.mjs` before the `fast-uri` override | 1 | The new version-floor test found `fast-uri 3.1.3`. |
| Same focused test after the override; `pnpm audit --audit-level high` | 0 / 0 | Both dependency floors passed and the full Node audit reported no known vulnerabilities. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | Pinned install, repository tools and machine contracts passed; all 37 acceptance bindings remain honestly planned, not product-verified. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets`; `pnpm check:licenses` | 0 / 0 / 0 / 0 / 0 / 0 | Architecture, specification index, ledgers, generated drift, secret scan and licenses passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Web lint, strict types, unit tests, production build and Chromium bootstrap test passed. |

PostgreSQL validation used `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` on loopback port 55432. This is complete local working-tree validation, not independent or commit-bound evidence.
