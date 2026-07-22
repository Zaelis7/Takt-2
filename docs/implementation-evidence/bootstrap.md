## Implementation Evidence

- Commit: `c90c7b9b0f809a6c31dcb626a0342db51bd073a3`
- Requirements: `PRD-API-002` (contract/codegen baseline), `PRD-NFR-001` (dependency-free bootstrap start), `PRD-NFR-003` (embedded production-build foundation), `PRD-NFR-008` (health/readiness), `PRD-NFR-010` (deterministic domain test), `PRD-MON-007` (Proto generation only; remote probe behavior remains out of scope)
- Contracts changed: no; `specs/contracts/openapi.yaml`, `probe.proto`, and `takt-config.schema.json` are unchanged
- Migrations: none
- Tests added: `crates/api/tests/health_http.rs`, `crates/domain/tests/resource_id.rs`, `crates/probe-protocol/tests/generated_contract.rs`, `web/src/App.test.tsx`, `tests/e2e/bootstrap.spec.ts`
- Commands executed: see exact command list below; every final gate exited `0`
- Security review: no secret, auth, persistence, migration, or external data flow added; CSP, frame, referrer, and MIME-sniffing headers are centralized; independent validation found that commit `c90c7b9` reflected non-v7 UUID request IDs
- Known limitations: readiness has no external dependency checks because database, keys, and workers are intentionally absent; no prior release exists for OpenAPI breaking-change comparison; Gherkin is syntax-validated but release scenarios are not implemented in this bootstrap; CI is defined but was not run on GitHub; duplicate Rust transitive build-tool versions remain `cargo-deny` warnings
- Reviewer verdict: changes requested; the original approval missed non-v7 UUID request-ID reflection
- Validator verdict: failed for commit `c90c7b9`; all repository gates passed, but the UUIDv7 HTTP behavior did not

### Exact commands and exit codes

| Command | Exit code | Result |
|---|---:|---|
| `cargo test -p takt-domain -p takt-api` | 1 | Expected test-first failure before implementation (`ResourceId` missing) |
| `pnpm install --frozen-lockfile` | 0 | Lockfile verified; dependencies already current |
| `pnpm contracts:openapi` | 0 | OpenAPI 3.1 valid under Redocly recommended rules |
| `pnpm contracts:schema` | 0 | `specs/examples/takt.yaml` valid against Draft 2020-12 schema |
| `pnpm contracts:proto` | 0 | Proto compiled through protox/Prost and matched committed output |
| `pnpm contracts:gherkin` | 0 | Three feature files parsed |
| `pnpm check:architecture` | 0 | Dependency directions and unsafe guards valid |
| `pnpm check:generated` | 0 | OpenAPI types, Prost types, and embedded web output have no drift |
| `cargo fmt --all -- --check` | 0 | Rust formatting valid |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | No warning |
| `cargo test --workspace --all-features` | 0 | Eight Rust integration/domain/protocol tests passed |
| `cargo deny check` | 0 | Advisories, bans, licenses, and sources passed; duplicate build-tool versions warned |
| `cargo audit` | 0 | 127 locked crate dependencies scanned; no vulnerability reported |
| `pnpm audit --audit-level high` | 0 | No known vulnerability |
| `pnpm licenses list --long` | 0 | Complete Node license inventory produced and reviewed |
| `pnpm lint` | 0 | ESLint strict/type-aware rules passed |
| `pnpm typecheck` | 0 | TypeScript strict check passed |
| `pnpm test --run` | 0 | One Vitest file/test passed |
| `pnpm build` | 0 | Vite production build completed; 60.20 kB gzip JavaScript |
| `pnpm playwright test` | 0 | One Chromium E2E test passed |
| `cargo build --workspace --all-features --release --locked` | 0 | Optimized workspace and embedded server build completed |

### Current independent validation addendum

The failed validator verdict above remains the historical result for commit
`c90c7b9b0f809a6c31dcb626a0342db51bd073a3`. Commit
`4ef411a4718d21fc4f364494dc3810f716215e98` was subsequently validated from a
clean detached checkout on `2026-07-22`; all current repository gates passed
against real PostgreSQL 16.9 and SQLite. The exact independent verdict, commands
and remaining non-release limitations are recorded in
`docs/implementation-evidence/evid-001-independent-head-validation.md`.
