# Implementation Evidence: SPEC-021

- Evidence date: `2026-07-23`
- Base commit: `9dea75ad16697851aa5fa157d21c06c3db36ce4a`; change is not yet committed.
- Requirements: `PRD-API-002`, `PRD-API-004`, `PRD-API-005`
- Finding: `SPEC-007` resolved.
- Contracts changed: yes; `specs/contracts/openapi.yaml` adds the reusable `InvalidCursorProblem` response/schema and binds `listApiTokens` status 400 to it.
- Generated artifacts: `web/src/generated/openapi.ts` was regenerated with the repository generator.
- Migrations: none.
- Tests added: `PRD-API-004 and PRD-API-005 API token cursor failures have a stable problem` asserts the operation response reference, Problem schema reference, status 400 and stable `invalid_cursor` code.
- Package size: 39 handwritten Contract-/Test insertions (21 OpenAPI, 18 test), below the 100-line preflight estimate; the generated TypeScript delta is excluded.
- Behavior: invalid, expired, tampered or filter-/sort-mismatched API-token list cursors now have one machine-readable OpenAPI response with the normative `invalid_cursor` code. No fields, endpoints, successful responses or other error semantics changed.
- Security/data review: the Problem schema contains only the existing bounded Problem Details fields and adds no secret, credential, external flow, persistence behavior or audit effect. The API-token list remains a redacted metadata response.
- Known limitations: this package changes only the contract. Axum serialization, credential extraction and cursor decoding follow in `IAM-035`; production Application/Persistence composition follows in `IAM-034`. Other list resources bind their cursor problems with their own contract/runtime packages.
- Builder verdict: implemented.
- Reviewer verdict: local contract/diff review found no remaining contradiction or unrelated login/auth drift; independent review is pending.
- Validator verdict: full local working-tree validation passed against PostgreSQL 16.9 and SQLite. Independent clean-checkout and CI validation are pending, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `node --test tools/openapi-token-contract.test.mjs` before the contract change | 1 | The new test observed `#/components/responses/InvalidRequestProblem` instead of the normative `InvalidCursorProblem`; no pass was claimed. |
| Same focused test after the first edit | 1 | A too-broad patch changed the login 400 reference instead of the token-list reference; the focused test remained red and exposed the unintended drift before completion. |
| `node --test tools/openapi-token-contract.test.mjs tools/openapi-auth-contract.test.mjs` after correction | 0 | All six token/auth contract tests passed and confirmed that login semantics remained unchanged. |
| `pnpm contracts:validate` | 0 | OpenAPI 3.1 lint, Config Schema, Proto, Gherkin and cross-contract fixtures passed. |
| `pnpm check:generated` before regeneration | 1 | The gate correctly reported OpenAPI TypeScript drift; no pass was claimed. |
| `pnpm generate:openapi`; `pnpm check:generated` | 0 / 0 | Generated TypeScript was updated deterministically and the drift gate passed. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Rust format and all-target lint gates passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | The complete workspace passed against PostgreSQL 16.9 and SQLite without skip. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust policy, advisories and locked optimized build passed. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm acceptance:check` | 0 / 0 / 0 | Pinned workspace, tool tests and all 37 contract bindings passed; acceptance remains 37 planned and 0 runnable. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:secrets` | 0 / 0 / 0 / 0 | Architecture, spec index, 101-package tracking DAG and secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisory and license gates passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Frontend lint, strict types, Vitest, production build and Chromium bootstrap passed. |
| `git diff --check` | 0 | Patch whitespace and integrity passed after the final tracking/evidence update. |
