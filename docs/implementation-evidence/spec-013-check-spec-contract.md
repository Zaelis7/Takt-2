## Implementation Evidence

- Evidence date: `2026-07-20`
- Target: uncommitted `SPEC-013` working tree based on commit `53be57ab890391c887719a7eaa380cde8e4770d6`, layered on the pre-existing uncommitted implemented `SPEC-010` and `SPEC-012` changes; no commit, push or pull request was requested
- Requirements: `PRD-MON-002`, `PRD-API-002`
- Contracts changed: yes; `specs/contracts/openapi.yaml`, `specs/contracts/takt-config.schema.json`, `specs/contracts/probe.proto` and the normative mapping in `specs/04-probes-and-checks.md`; generated OpenAPI/Proto types were refreshed
- Migrations: none
- Tests added: positive and negative seven-kind fixtures under `specs/contracts/fixtures/`, `tools/check-spec-contract.test.mjs`, and a Proto round-trip case in `crates/probe-protocol/tests/generated_contract.rs`
- Security review: HTTP auth accepts only persistent SecretRefs, browser `fill` requires a SecretRef, dispatch maps references to short-lived Proto `ephemeral_key` values inside the sealed bundle, no fixture contains a usable credential, and invalid Proto input is specified as `REJECTED_INVALID` rather than a target failure. The full Node audit found the unrelated pre-existing high-severity `js-yaml` advisory recorded as `SEC-001`; the production-only audit is clean.
- Known limitations: this is a contract-only package. Common proxy, resolver and address-family options remain in `SPEC-019`; Rust domain types remain in `MON-011`; monitor persistence, local/remote executors, typed observations and runtime conformance are absent. `CHECK-030` still owns the complete 0.3 browser-worker contract. No released OpenAPI baseline exists for compatibility comparison. Independent clean-checkout validation is impossible for the uncommitted stacked target.
- Reviewer verdict: builder-side contract diff review passed for field coverage, units/defaults, SecretRef boundaries, Proto presence semantics, generated drift and release classification. Independent review is still pending.
- Validator verdict: **not passed as a repository-wide verdict**. Focused contract/Proto checks, full Clippy, release build, supply-chain policy except the newly reported Node advisory, and all web gates passed. The exact debug workspace test fails locally while resolving `sqlx`; the release-profile workspace test reaches the mandatory PostgreSQL suite and then fails because `TAKT_TEST_POSTGRES_URL` is absent. The full Node audit fails on `SEC-001`. `SPEC-013` is therefore `implemented`, never `verified`.

### Scope split

The first implementation pass combined the three machine contracts with full Rust domain validation and all common network options, exceeding the package's 800-handwritten-line target. The domain code was removed, and common proxy, resolver and address-family mapping was moved to the dependent `SPEC-019` package. `SPEC-013` now contains the reviewable per-kind contract slice; `MON-011` remains responsible for framework-free Rust domain types and their positive/negative tests.

### Test-first evidence and commands

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` (selection baseline) | 0 | 57 requirements, 79 packages and 8 findings were valid; `SPEC-013` was the first unblocked 0.1 package addressing a high contract finding. |
| `node --test tools/check-spec-contract.test.mjs` (initial test-first run) | 1 | Both tests failed: valid HTTP options were absent from OpenAPI/Config/Proto and the negative Push fixture was accepted. |
| `node --test tools/check-spec-contract.test.mjs` | 0 | Seven valid and ten invalid CheckSpec fixtures passed across OpenAPI, Config Schema and Proto semantics; exact fields, defaults and limits are compared for drift. |
| `pnpm contracts:validate` | 0 | OpenAPI, Config Schema/example, Proto, Gherkin syntax and the new CheckSpec golden suite passed. |
| `pnpm generate:openapi` | 0 | TypeScript OpenAPI types were regenerated. |
| `cargo run --locked -p xtask -- generate-proto` | 0 | Prost types were regenerated with stable wire tags and boxed large oneof variants. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web generated artifacts have no drift. |
| `cargo test -p takt-probe-protocol --release` | 0 | Generated types and explicit Proto `0`/`false` presence round-trips passed. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | The complete workspace passed Clippy with warnings denied. |
| `cargo test --workspace --all-features -- --test-threads=1` | 1 | Repeated debug-profile runs fail locally with `can't find crate for sqlx`; this is not counted as a pass. |
| `cargo build --workspace --all-features --release --locked` | 0 | The full optimized workspace compiled successfully, including `takt-persistence` and `takt-server`. |
| `cargo test --workspace --all-features --release -- --test-threads=1` | 1 | All suites reached before PostgreSQL passed; the mandatory PostgreSQL contract then failed because `TAKT_TEST_POSTGRES_URL` is not configured. |
| `cargo deny check` | 0 | Advisories, bans, licenses and sources passed; existing duplicate-version notices remain warnings. |
| `cargo audit` | 0 | No RustSec vulnerability caused failure. |
| `pnpm install --frozen-lockfile` | 0 | The pinned dependency graph installed without lockfile changes. |
| `pnpm test:tools` | 0 | Thirteen tool tests passed, including auth, tracking and the three CheckSpec contract tests. |
| `pnpm check:architecture` | 0 | Workspace dependency directions and unsafe-code guards passed. |
| `pnpm check:tracking` | 0 | Final ledger validated 57 requirements, 81 packages and 9 findings; `SPEC-013` is implemented, common network alignment is planned as `SPEC-019`, and `SEC-001` has a planned remediation package. |
| `pnpm check:secrets` | 0 | Secret scanning passed for 101 source files. |
| `pnpm check:licenses` | 0 | Production and development Node license policy passed. |
| `pnpm audit --audit-level high` | 1 | High advisory `GHSA-52cp-r559-cp3m` affects transitive dev dependency `js-yaml@4.2.0`; recorded as `SEC-001`. |
| `pnpm audit --prod --audit-level high` | 0 | No production dependency vulnerability was reported. |
| `pnpm lint` | 0 | Web lint passed with zero warnings. |
| `pnpm typecheck` | 0 | Strict TypeScript checking passed. |
| `pnpm test --run` | 0 | The web unit test passed. |
| `pnpm build` | 0 | The production web bundle built successfully. |
| `pnpm playwright test` | 0 | The Chromium bootstrap accessibility smoke test passed. |
| `git diff --check` | 0 | No whitespace error was reported in the final stacked diff. |
