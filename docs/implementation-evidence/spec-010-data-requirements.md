## Implementation Evidence

- Evidence date: `2026-07-20`
- Target: uncommitted `SPEC-010` working tree based on commit `53be57ab890391c887719a7eaa380cde8e4770d6`; no commit, push or pull request was requested
- Requirements: `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-IAM-001`, `PRD-IAM-003`, `PRD-NFR-002`, `PRD-NFR-005`, `PRD-NFR-007`
- Contracts changed: yes; `specs/00-product-requirements.md` now canonically defines the three already-used data requirements, `specs/10-traceability.md` maps them, and `specs/acceptance/v0.1.feature` makes the existing persistence scenario explicit. OpenAPI, JSON Schema and Proto are unchanged.
- Migrations: none; both unreleased `0001_persistent_identity.sql` files were reviewed and already reference the now-canonical IDs, so their contents and checksums remain unchanged
- Tests added: tracking-validator regression rejects an unknown-requirement exception after its finding is resolved; existing PostgreSQL, SQLite and server tests gained corrected requirement traceability only
- Security review: no external data flow, authorization rule, secret field, query or runtime behavior changed; the new requirement text preserves the existing engine-parity, fail-closed schema and typed identity constraints
- Known limitations: Docker and a local PostgreSQL service are unavailable, so the mandatory real-PostgreSQL contract and complete workspace gate were not passed on this working tree; Gherkin remains syntax-only under open finding `EVID-002`; independent clean-checkout validation requires a committed target and remains open under `EVID-001`
- Reviewer verdict: builder-side contract/diff review found no semantic expansion, migration checksum change, authorization drift, secret exposure or runtime change; independent review is pending
- Validator verdict: not passed; every available gate below passed, but `cargo test --workspace --all-features -- --test-threads=1` exited 101 because `TAKT_TEST_POSTGRES_URL` is unavailable

### Decision and traceability

`SPEC-001` offered two remedies. Replacing the invalid references with broad IAM/NFR IDs would have lost the already normative distinctions in chapters 01, 02, 06, 07 and 08. `SPEC-010` therefore promotes the three previously used stable IDs into chapter 00 without adding product behavior:

| Requirement | Existing normative source retained | Current coverage |
|---|---|---|
| `PRD-DATA-001` | PostgreSQL/SQLite repository semantics and explicit SQLite limits | Partial: identity/bootstrap only |
| `PRD-DATA-002` | Forward-only numbered migrations, newer-schema rejection and migration-time readiness | Partial: initial schema only; release fixtures remain future work |
| `PRD-DATA-004` | UUIDv7 identity, UTC microseconds and monotonically versioned mutable resources | Partial: identity entities only |

No additional DATA identifier was introduced. Historical Evidence keeps its original wording, while all formerly exceptional references are now canonical and `known_unknown_requirement_refs` is empty.

### Commands and exit codes

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` (baseline) | 0 | Initial registry valid with 54 canonical requirements and the documented exceptions. |
| `pnpm test:tools` (test-first) | 1 | Expected failure: the validator accepted an unknown-ID exception after its finding was resolved. |
| `pnpm test:tools` | 0 | Eight tool tests passed, including the new regression. |
| `pnpm check:tracking` | 0 | 57 canonical requirements, 75 packages and 8 findings are structurally valid without unknown-ID exceptions. |
| `pnpm contracts:gherkin` | 0 | All three feature files parsed after the traceability update. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 | Six SQLite migration/repository/bootstrap cases passed. |
| `cargo test -p takt-server --bin takt-server readiness_is_served_during_schema_initialization` | 0 | Migration-time readiness behavior passed. |
| `docker version --format '{{.Server.Version}}'` | 1 | Docker Desktop Linux engine is not running. |
| PostgreSQL service/client/listener discovery | 1 | No local PostgreSQL service, `psql` client or listener on port 5432 exists. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | Workspace lint passed without warnings. |
| `cargo deny check` | 0 | Advisories, bans, licenses and sources passed; duplicate-version diagnostics remain warnings. |
| `cargo audit` | 0 | No Rust vulnerability reported. |
| `pnpm install --frozen-lockfile` | 0 | Locked dependencies were already current. |
| `pnpm contracts:validate` | 0 | OpenAPI, JSON Schema, Proto and Gherkin validation passed. |
| `pnpm check:architecture` | 0 | Architecture and unsafe-code guards passed. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web generated outputs have no drift. |
| `pnpm check:secrets` | 0 | Secret-pattern scan passed. |
| `pnpm check:licenses` | 0 | Node production and development license gates passed. |
| `pnpm audit --audit-level high` | 0 | No known high-severity Node vulnerability reported. |
| `pnpm lint` | 0 | Frontend ESLint passed. |
| `pnpm typecheck` | 0 | Strict TypeScript check passed. |
| `pnpm test --run` | 0 | Frontend Vitest suite passed. |
| `pnpm build` | 0 | Frontend production build passed. |
| `pnpm playwright test` | 0 | Chromium bootstrap E2E passed. |
| `cargo test --workspace --all-features -- --test-threads=1` | 101 | Mandatory PostgreSQL contract failed immediately because `TAKT_TEST_POSTGRES_URL` is missing; all tests started before it passed. |
| `cargo test --workspace --all-features --exclude takt-persistence -- --test-threads=1` | 0 | All non-persistence Rust unit, integration and doc tests passed. |
| `cargo build --workspace --all-features --release --locked` | 0 | Optimized workspace build passed. |
| `cargo fmt --all -- --check && pnpm test:tools && pnpm check:tracking && pnpm contracts:gherkin && pnpm check:secrets && git diff --check` (final consistency run) | 0 | Final formatting, tool tests, 57-requirement registry, Gherkin syntax, secret scan and whitespace review passed. |
