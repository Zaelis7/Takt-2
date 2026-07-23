# IAM-043 Implementation Evidence – safe API-token mutation replay

- Evidence date: `2026-07-23`
- Base commit: `bd56cfcbbc152419d9c68c3c8b49a0e6effd44bc`; the package change is uncommitted.
- Requirements: `PRD-API-002`, `PRD-API-003`, `PRD-API-005`, `PRD-IAM-005`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`
- Finding: `SPEC-008` is resolved by this package.
- Contracts changed: yes – `specs/contracts/openapi.yaml` and regenerated `web/src/generated/openapi.ts`.
- Migrations: `migrations/postgres/0007_api_token_mutation_replay.sql`, `migrations/sqlite/0007_api_token_mutation_replay.sql`; published migrations `0001`–`0006` remain unchanged.
- Tests added or extended: the OpenAPI token contract fixes Patch/Revoke replay status, headers, body and conflict semantics; the Application test replays the original active Patch projection after a later Revoke; the shared PostgreSQL/SQLite contracts cover delayed Patch replay, Patch/Revoke expiry, hash conflict, audit-once behavior, schema constraints and persisted-snapshot redaction.
- Package size: 689 handwritten Contract-/Source-/Test-/Migration insertions, excluding generated TypeScript and tracking/evidence, below the 700-line estimate and 800-line hard limit.
- Behavior: Patch stores status `200`, the original ETag and a safe token projection; Revoke stores status `204` and a safe internal projection with no response body. Replay is still bound to actor, method, path, key and request hash, returns the original projection/status after later token changes and never repeats the business mutation or audit event.
- Security review: the snapshot schema and strict decoder admit only redacted API-token fields. Token value, Argon2 hash, request hash and encrypted Create replay payload never enter the snapshot; database constraints reject explicitly secret-bearing snapshot keys and regression tests inspect persisted rows on both engines.
- Data review: migration `0007` is forward-only and engine-parity tested. Status, ETag and snapshot complete atomically with token mutation and audit. Exact 24-hour expiry, concurrency, rollback and newer-schema rejection remain covered.
- Authorization review: no authorization boundary changed. Browser/Bearer permission and organization/project checks still run before the repository; idempotency identity remains the truthful user or API-token actor.
- Observability review: no new logs, metrics or traces were added; snapshot/debug/audit assertions remain secret-negative.
- Known limitations: API-token write HTTP DTOs, CSRF/If-Match parsing and production routing remain in `IAM-040`, `IAM-041` and `IAM-013`. Pre-0.1 mutation-idempotency rows created by migrations `0005`/`0006` have no reconstructible snapshot and therefore fail closed until their existing 24-hour expiry; no released database version is affected.
- Acceptance status: the exact 37-entry inventory remains valid, but all v0.1 bindings remain planned; this package is repository/Application behavior evidence, not release acceptance.
- Builder verdict: `implemented`.
- Reviewer verdict: local contract, migration, authorization, idempotency, redaction and diff review approved; no independent reviewer verdict exists.
- Validator verdict: the complete local working-tree gate matrix passed except for the intentionally strict v0.1 readiness runner, which correctly reports all 15 scenarios as planned rather than runnable. No commit-bound independent or CI verdict exists, so the package is not `verified`.

## Test-first and focused validation

| Command | Exit | Result |
|---|---:|---|
| `node --test tools/openapi-token-contract.test.mjs` before implementation | 1 | The new PRD-API-003 test found no Patch/Revoke `x-takt-idempotency` contract. |
| `cargo test -p takt-application --test api_tokens prd_api_003_idempotent_api_token_mutations_are_safe_and_actor_bound -- --exact` before implementation | 1 | The freshly rebuilt test binary was initially blocked by the local Windows application-control policy; the same exact behavior test executed and passed after implementation. |
| `node --test tools/openapi-token-contract.test.mjs` | 0 | Five API-token contract cases passed, including safe mutation replay. |
| `cargo test -p takt-application --test api_tokens prd_api_003_idempotent_api_token_mutations_are_safe_and_actor_bound -- --exact` | 0 | Delayed Patch replay preserved the original active version after Revoke and did not append audit. |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 | All six SQLite migration/repository/bootstrap contracts passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | The real pinned PostgreSQL 16.9 contract passed. |
| `cargo check --workspace --all-targets --all-features`; `cargo fmt --all`; `pnpm generate:openapi` | 0 / 0 / 0 | All targets type-checked, Rust was formatted and generated OpenAPI declarations were refreshed. |

## Full repository validation

| Command | Exit | Result |
|---|---:|---|
| `pnpm install --frozen-lockfile` | 0 | The committed lockfile installed without drift. |
| `pnpm contracts:validate`; `pnpm check:architecture`; `pnpm check:spec-index` | 0 / 0 / 0 | OpenAPI, JSON Schema, Proto, architecture and specification-index contracts passed. |
| `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 | Tracking is internally consistent, generated declarations match OpenAPI and the 167-file secret scan passed. |
| `pnpm acceptance:check` | 0 | All 37 acceptance scenarios have valid requirement/package bindings. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed after applying `cargo fmt --all` to the package changes. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | The complete Rust workspace passed with warnings denied. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55433/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | The full workspace, including all six SQLite contracts, real pinned PostgreSQL 16.9 and five CLI process tests, passed. |
| `cargo deny check`; `cargo audit` | 0 / 0 | License/advisory policy passed; `cargo deny` emitted only permitted duplicate-version warnings. |
| `cargo build --workspace --all-features --release --locked` | 0 | The locked release build passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisories and repository license policy passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build` | 0 / 0 / 0 / 0 | The strict web lint, type, unit-test and production-build gates passed. |
| `pnpm playwright test` | 0 | The Chromium browser test passed. |
| `pnpm test:tools` | 0 | All 34 repository tooling tests passed. |
| `pnpm acceptance:run -- --release v0.1` | 1 | Expected non-readiness: all 15 v0.1 scenarios remain `planned`, so this is recorded as not passed and not attributed to IAM-043 completion. |

The first full Rust test attempt encountered an environmental PostgreSQL `UnexpectedEof` after its disposable test container disappeared and another local container occupied port 55432. No external container was changed. A fresh, digest-pinned PostgreSQL 16.9 container on port 55433 produced the clean full-workspace pass above. The first formatting check also exposed one package-local formatting delta, which was corrected before the passing rerun. These earlier failures remain recorded rather than being treated as passes.
