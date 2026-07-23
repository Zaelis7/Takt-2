# Implementation Evidence: IAM-037

- Evidence date: `2026-07-23`
- Base commit: `5d70f7cebf1fcad0a59c2dfd9dfcca02839d7dfa`; change is not yet committed.
- Requirements: `PRD-API-001`, `PRD-API-005`, `PRD-IAM-001`, `PRD-IAM-004`
- Contracts changed: no; the API-token Get, redacted metadata, authentication, ETag and Problem shapes from `SPEC-015` remain unchanged.
- Migrations: none.
- Tests added: `api_token_get_boundary_is_contract_shaped_and_fails_closed` exercises a real Axum TCP listener with runtime-generated Bearer and session credentials. It covers missing, malformed and ambiguous credentials before the injected port; Bearer and session success; safe metadata and ETag; permission, not-found and infrastructure Problems; trusted peer IP; credential Debug redaction; absence of token/hash material; and fail-closed rejection of a non-canonical CIDR projection.
- Package split: the first combined IAM-035 List/Get draft measured 1,122 handwritten Source-/Test insertions and exceeded the 800-line hard limit. Before completion it was split into this Get/credential slice (`IAM-037`) and the remaining List/query/cursor slice (`IAM-035`).
- Package size: 765 Source-/Test insertions (462 in `api_tokens.rs`, 269 in its HTTP test and 34 in `lib.rs`), below the 800-line hard limit. The final size exceeded the 650-line estimate after complete real-HTTP negative coverage; List/filter/cursor behavior remains excluded.
- Behavior: `/api/v1/api-tokens/{api_token_id}` extracts exactly one bounded Bearer token or `takt_session` cookie into a zeroizing, redacted credential and passes it with the direct peer IP, UUIDv7 and request ID to an injected read port. A valid safe resource becomes the exact OpenAPI metadata projection with quoted version ETag and `no-store`; malformed credentials fail before Application, while permission, absence and infrastructure remain typed.
- Security review: no raw token or token hash type exists in the response projection. Credential values are zeroized, have a fixed redacted `Debug`, and are exposed only to the injected authentication/Application port. Duplicate credentials, mixed Bearer/session input and malformed values fail generically. The response boundary validates UUIDv7 identifiers, bounded names/prefixes/scopes, unique canonical CIDRs, timestamps and positive versions before serialization. Internal logs contain only a stable event code and request ID.
- Dependency review: `serde_json` is a test-only dependency used to inspect the real JSON response. Its pinned workspace version was already present in `Cargo.lock`; only the existing `takt-api` dependency list changed and no crate version was added.
- Contract/data review: no public contract, migration, repository behavior, audit event or new external data flow changed. The direct TCP peer address is forwarded without interpreting proxy headers; trusted-proxy handling remains a deployment concern and no client-controlled forwarding header is accepted here.
- Known limitations: IAM-035 still owns List query/filter/cursor integration. IAM-034 owns the Application/Persistence adapter and production router composition, so the default server cannot yet complete this route. Create/Patch/Revoke and Browser CSRF remain in IAM-013; monitor routes and the runnable 0.1 exact-scope product scenario follow later. All 37 product acceptance bindings remain planned.
- Builder verdict: implemented.
- Reviewer verdict: local contract, authorization, secret, observability and diff review found and test-first corrected the non-canonical CIDR projection; independent review is pending.
- Validator verdict: full local working-tree validation passed against the pinned PostgreSQL test service and SQLite. Independent clean-checkout and CI validation are not claimed, so the package is not `verified`.

## Test-first and validation

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p takt-api --test api_tokens_http` before implementation | non-zero (reported 1) | The new real-HTTP behavior test failed to compile because the API-token HTTP types, port and router builder did not exist; no pass was claimed. |
| `cargo test -p takt-api --test api_tokens_http` after the first implementation | 0 | Credential, safe metadata, ETag and typed Problem behavior passed before the mandatory size measurement. |
| Source-/test-line measurement of the combined List/Get draft | n/a | 1,122 insertions exceeded the 800-line limit, so the draft was split and only the Get slice retained. |
| `cargo test -p takt-api --test api_tokens_http` after adding non-canonical CIDR output | 1 | The new negative case received `200`, demonstrating the missing canonical-projection check before it was fixed. |
| `cargo test -p takt-api --test api_tokens_http`; `cargo clippy -p takt-api --all-targets --all-features -- -D warnings` after the fix | 0 / 0 | The complete focused behavior and all-target package lint passed. |
| `cargo test -p takt-api --all-features`; `pnpm check:tracking` | 0 / 0 | Every API crate test passed and the split registry was valid with 57 requirements, 102 packages and 12 findings. |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 / 0 | Rust formatting and all-target workspace lint passed. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | The complete workspace passed against the pinned PostgreSQL 16.9 container and SQLite without skip. |
| `cargo deny check`; `cargo audit`; `cargo build --workspace --all-features --release --locked` | 0 / 0 / 0 | Rust policy, current advisory scan and locked optimized build passed; configured duplicate-version warnings remain non-blocking. |
| `pnpm install --frozen-lockfile`; `pnpm test:tools`; `pnpm contracts:validate`; `pnpm acceptance:check` | 0 / 0 / 0 / 0 | The pinned workspace, 33 tool tests, OpenAPI/Schema/Proto/Gherkin contracts and the exact scenario inventory passed. Acceptance honestly reports 37 planned, 0 runnable and 0 verified scenarios. |
| `pnpm check:architecture`; `pnpm check:spec-index`; `pnpm check:tracking`; `pnpm check:generated`; `pnpm check:secrets` | 0 / 0 / 0 / 0 / 0 | Architecture, 16-path spec index, 57-requirement/102-package/12-finding registry, generated drift and the 156-file secret scan passed. |
| `pnpm audit --audit-level high`; `pnpm check:licenses` | 0 / 0 | Node advisories and production/tooling license policies passed. |
| `pnpm lint`; `pnpm typecheck`; `pnpm test --run`; `pnpm build`; `pnpm playwright test` | 0 / 0 / 0 / 0 / 0 | Frontend lint, strict types, Vitest, production build and the Chromium accessibility bootstrap passed. |
| `git diff --check` | 0 | Patch whitespace and integrity passed before the final tracking/evidence update and is repeated in the final review. |
