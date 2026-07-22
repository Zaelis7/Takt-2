# Implementation Evidence: SPEC-020

- Evidence date: `2026-07-21`
- Base commit: `51349e3dd5391f4c9a0a5375a92754a1743264b9`; the package working-tree change is not yet committed.
- Requirements: `PRD-API-002`, `PRD-API-003`, `PRD-IAM-001`, `PRD-IAM-005`
- Contracts changed: yes; `specs/01-architecture.md`, `specs/03-api-and-automation.md` and `specs/contracts/openapi.yaml` now agree on API-token Create replay behavior, and `web/src/generated/openapi.ts` was regenerated.
- Migrations: none; persistence and encryption follow in `IAM-024`.
- Tests added: `tools/openapi-token-contract.test.mjs` rejects an API-token Create operation without the bounded encrypted replay metadata, the specific hash-conflict response and its stable Problem code.
- Package split: the original `IAM-013` preflight combined persistence, application/Bearer and HTTP responsibilities at the 800-line hard limit. The missing migration made that estimate exceed the limit, so it was split before implementation into contract package `SPEC-020`, persistence package `IAM-024`, application package `IAM-025` and the remaining HTTP package `IAM-013`.
- Package size: the final working-tree diff contains 269 insertions and 27 deletions across ten artifacts, including the package split and this evidence, below the 800-line hard limit.
- Behavior: an identical replay returns the stored `201` status, relevant headers and token-bearing body for 24 hours. Reuse with a different request hash returns `409 idempotency_key_reused` without business or audit effect.
- Security review: the replay is bound to actor, method, path and request hash. Its token-bearing payload requires authenticated encryption, expires after 24 hours and may not appear in ordinary reads, audit, Problems, logs or telemetry.
- Known limitations: no replay persistence or runtime behavior was added. `IAM-024` supplies storage/encryption and atomic writes, `IAM-025` supplies CRUD/Bearer orchestration, and `IAM-013` supplies HTTP serialization, CSRF/Bearer composition and cursors.
- Builder verdict: `implemented`.
- Reviewer verdict: local spec/contract/diff review completed; independent review remains pending.
- Validator verdict: all local working-tree gates passed; independent clean-checkout and CI validation remain pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `node --test tools/openapi-token-contract.test.mjs` before the contract change | 1 | The new replay test proved that Create had no machine-readable retention, replay, encryption or stable key-reuse conflict contract. |
| Same focused contract test after the change | 0 | All three API-token contract tests passed. |
| `pnpm contracts:openapi`; `pnpm contracts:validate` | 0 / 0 | OpenAPI lint and all OpenAPI/Schema/Proto/Gherkin/CheckSpec contracts passed. |
| `pnpm test:tools` | 0 | All 31 repository tool tests passed. |
| `pnpm generate:openapi`; `pnpm check:generated` | 0 / 0 | TypeScript API types were regenerated and drift-free. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Rust formatting and all lint targets passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Entire workspace passed, including real PostgreSQL 16.9, SQLite and all five CLI process tests. |
| `cargo deny check`; `cargo audit` | 0 / 0 | License/source/advisory policies passed; configured duplicate-version warnings remain non-blocking. |
| `pnpm install --frozen-lockfile`; `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 / 0 | Pinned dependencies were current, no known high Node vulnerability was reported and licenses passed. |
| `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Acceptance inventory, architecture, spec index, tracking and secret scan passed; all 37 scenarios remain honestly planned. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run` | 0 / 0 / 0 | Web lint, strict typecheck and Vitest passed. |
| `pnpm build`; `pnpm playwright test` | 0 / 0 | Production web build and Chromium bootstrap test passed. |
| `cargo build --workspace --all-features --release --locked` | 0 | Locked optimized workspace build passed. |
