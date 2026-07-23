# Implementation Evidence: IAM-035

- Evidence date: `2026-07-23`
- Base commit: `091e99e08973b1c8e088917f2c5fdb53199dab24`; change is not yet committed.
- Requirements: `PRD-API-001`, `PRD-API-004`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`.
- Contracts changed: no; the API-token List, filter, stable-sort, page and `invalid_cursor` shapes from `SPEC-015`/`SPEC-021` remain unchanged.
- Migrations: none.
- Tests added: `api_token_list_boundary_is_filter_and_cursor_bound` exercises a real Axum TCP listener with a runtime-generated Bearer credential. It covers the default and explicit limits, all four filters, a two-page cursor roundtrip, stable item order, empty-page termination, safe metadata, permission failure, MAC manipulation, filter changes, duplicate limits, invalid limits/enums, unknown query fields and absence of token/hash/credential material.
- Package size: 448 Source-/Test insertions plus three dependency-manifest insertions, below the 550-line preflight estimate.
- Behavior: `GET /api/v1/api-tokens` extracts exactly one existing redacted credential, bounds the complete query, accepts every supported parameter at most once, validates UUIDv7/enums/scope/limit, canonicalizes all filters and only then verifies the IAM-036 cursor. The injected port receives typed filters, the decoded exclusive boundary, direct peer IP and request ID. Successful pages contain only safe IAM-037 projections and a new cursor derived from the last returned item.
- Security review: malformed, duplicate, unknown, manipulated or filter-mismatched query state fails before the port with the operation's sole contracted `400 invalid_cursor` Problem and never echoes query, cursor or credential material. Returned pages fail closed when they exceed the requested limit, violate an active filter, overlap the incoming boundary, drift from `created_at desc, id desc`, expose an invalid safe projection or claim another page without an item. Internal logs retain only the existing stable event code and request ID.
- Dependency review: direct use of `form_urlencoded 1.2.2` provides bounded `application/x-www-form-urlencoded` query decoding, including percent decoding, without enabling Axum's broader query feature. It is pinned, MIT/Apache-2.0 licensed, actively maintained with the Rust URL ecosystem and was already present in `Cargo.lock`, so no crate version was added.
- Contract/data review: no public schema, migration, repository behavior, audit event or new external data flow changed. The OpenAPI operation fixes the sort and exposes no `sort` query; unknown fields therefore fail closed. Its only declared 400 response is `InvalidCursorProblem`, so all rejected list query/cursor state uses that stable response rather than introducing an undeclared error contract.
- Known limitations: IAM-034 still owns the authentication/Application/Persistence adapter and production router composition. Create/Patch/Revoke and Browser CSRF remain in IAM-013; monitor routes and the runnable exact-scope product scenario follow later. All 37 product acceptance bindings remain planned.
- Builder verdict: implemented.
- Reviewer verdict: local contract, authorization, pagination, redaction, dependency and diff review found no remaining blocker; independent review is pending.
- Validator verdict: full local working-tree validation passed against the pinned PostgreSQL test service and SQLite. Independent clean-checkout and CI validation are not claimed, so the package is not `verified`.

## Test-first and focused validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-api --test api_tokens_http` before implementation | 101 | The new List behavior failed to compile because the typed List query/page, port method and cursor-aware router did not exist; no pass was claimed. |
| `cargo test -p takt-api --test api_tokens_http` after implementation | 0 | Get and the new real-HTTP List behavior passed. |
| `cargo test -p takt-api --all-features`; `cargo clippy -p takt-api --all-targets --all-features -- -D warnings` | 0 / 0 | Every API crate test and all-target package lint passed after narrowing a test Mutex guard that Clippy correctly rejected across `await`. |
| `node --test tools/openapi-token-contract.test.mjs`; `pnpm contracts:openapi`; `pnpm check:tracking`; `git diff --check` | 0 / 0 / 0 / 0 | The governing token/OpenAPI contract, tracking DAG and patch integrity passed. |

## Full local validation

| Command | Exit | Result |
|---|---:|---|
| `pnpm install --frozen-lockfile`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 / 0 | The pinned Node workspace, Rust formatting and warning-denying all-target workspace lint passed. |
| `docker run --rm --detach --name takt-postgres-test -p 127.0.0.1:55432:5432 -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=takt_test postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7`; `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 / 0 | The disposable pinned PostgreSQL 16.9 service became ready and the complete workspace passed against PostgreSQL and SQLite without skip, including five real CLI process tests and doctests. The container was stopped and removed after validation. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust policy, current advisory scan and the locked optimized build passed; configured duplicate-version warnings remain informational. |
| `pnpm test:tools` | 0 | All 33 tooling, tracking, supply-chain and OpenAPI contract tests passed. |
| First `pnpm contracts:validate`; isolated `pnpm contracts:schema`, `pnpm contracts:proto`, `pnpm contracts:gherkin`, `pnpm contracts:check-spec`; repeated `pnpm contracts:validate` | `3221226505` / 0 / 0 | The first Windows aggregate process stopped after successful OpenAPI lint and was not counted as a pass. Every remaining subgate passed individually, and the exact aggregate command then passed on repetition. |
| `pnpm acceptance:check`; `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 / 0 | The exact 37-scenario inventory, architecture, 16-path spec index, 57-requirement/102-package/12-finding registry, generated drift and 157-file secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | No known Node vulnerability and no disallowed production/tooling license was found. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Frontend lint, strict types, Vitest, production build and Chromium accessibility bootstrap passed. |
| `pnpm acceptance:run -- --release v0.1` | 1 (expected, not a pass) | The runner rejected all 15 planned v0.1 bindings as non-runnable. This preserves `EVID-002`; contract and inventory checks are not presented as product acceptance. |
| `git diff --check` | 0 | Patch whitespace and integrity passed. |
