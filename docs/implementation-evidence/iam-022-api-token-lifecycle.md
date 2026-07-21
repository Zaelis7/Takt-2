# Implementation Evidence: IAM-022

- Evidence date: `2026-07-21`
- Base commit: `6dd79d11bc14b5b209c3b67c20c92faa1bf4e746`; corrective working-tree change is not yet committed.
- Requirements: `PRD-IAM-001`, `PRD-IAM-004`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-NFR-002`, `PRD-NFR-005`
- Contracts changed: no; the public API-token contract remains `specs/contracts/openapi.yaml`.
- Migrations: none; this package completes lifecycle behavior on committed migration `0004` without changing it.
- Tests added: the shared PostgreSQL/SQLite lifecycle contract now proves that an expired token cannot be patched back to active by clearing `expires_at` and cannot be mutated by revoke.
- Package size: aggregate diff from `c6c2981` is 478 insertions and 17 deletions across the eight package artifacts, below the 800-line hard limit.
- Behavior: Patch and Revoke are optimistic, reject stale/revoked/expired/backdated writes and increment the version on success. Last-used is monotonic and cannot reactivate an invalid token.
- Security review: Patch/Revoke validate token identity, organization/project, actor presence, action and timestamp before atomically writing one redacted audit event. Duplicate-audit failures roll state back. The continuation review found and closed an expiry predicate gap that otherwise allowed removal of an already elapsed expiry.
- Known limitations: public CRUD, Idempotency-Key storage, signed cursor encoding, conditional session CSRF, one-time token response, Bearer hash verification and Scope enforcement remain in `IAM-013`.
- Builder verdict: `implemented`.
- Reviewer verdict: local spec/diff review completed and the expiry-bypass finding was fixed; independent review remains pending.
- Validator verdict: all local working-tree gates passed; independent clean-checkout and CI validation remain pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-persistence --test sqlite_contract sqlite_runs_the_shared_repository_contract -- --exact --test-threads=1` before the expiry fix | 101 | The new negative assertion proved that an expired token could be patched to clear `expires_at` and become active again. |
| Same focused SQLite command after the fix | 0 | Shared lifecycle contract, including expired Patch/Revoke rejection, passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | Real PostgreSQL 16.9 passed the same lifecycle contract. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | All Rust targets passed without warnings. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Entire workspace passed, including both engines and five real `takt-server.exe` CLI process tests. |
| `target/debug/takt-server.exe --help` after the rebuild | 0 | Direct process start is no longer blocked by the local Code Integrity policy. |
| `cargo deny check`; `cargo audit` | 0 / 0 | License/source/advisory policies passed; configured duplicate-version warnings remain non-blocking. |
| `pnpm install --frozen-lockfile` | 0 | Pinned Node workspace was already current. |
| `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 | OpenAPI, schema, Proto, Gherkin and CheckSpec contracts passed; all 37 acceptance bindings remain honestly `planned`, not product-verified. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, spec index, tracking, generated drift and secret scan passed. |
| `pnpm test:tools` | 0 | All 30 repository tool tests passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | No known Node vulnerability and all production/development licenses passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run` | 0 / 0 / 0 | Web lint, strict typecheck and Vitest passed. |
| `pnpm build`; `pnpm playwright test` | 0 / 0 | Production web build and Chromium bootstrap test passed. |
| `cargo build --workspace --all-features --release --locked` | 0 | Locked optimized workspace build passed. |

## Historical blocked validation

Before the corrective rebuild, Windows Smart App Control rejected the then-existing unsigned `target/debug/takt-server.exe` with OS code 4551 and policy `{0283ac0f-fff1-49ae-ada1-8a933130cad6}`. The earlier full workspace run therefore exited 101 after the repository tests, and no pass was claimed. During this continuation, Cargo rebuilt the affected binary and the same full workspace command, including all CLI process cases, passed. This supplements rather than overwrites the historical failed evidence.
