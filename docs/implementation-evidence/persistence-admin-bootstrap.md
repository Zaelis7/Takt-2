## Implementation Evidence

- Evidence date: `2026-07-20`
- Target: current working tree based on commit `c6f0161d5d8840be50be4d3937eac32b9db901aa`; no commit, push or pull request was requested
- Requirements: `PRD-API-002`, `PRD-DATA-001`, `PRD-DATA-002`, `PRD-DATA-004`, `PRD-IAM-001` (partial: local administrator and credential only), `PRD-IAM-003` (partial: persistence model and all five roles), `PRD-IAM-004` (preparatory application/repository boundary), `PRD-IAM-005` (partial: append-only bootstrap audit), `PRD-NFR-001`, `PRD-NFR-002`, `PRD-NFR-005`, `PRD-NFR-007` (partial: migration controls; backup verification remains later scope), `PRD-NFR-008`, `PRD-NFR-010`
- Contracts changed: application repository ports now include an optimistic organization update and a stable organization audit listing; OpenAPI, JSON Schema, Proto and product specs are unchanged
- Migrations: the unreleased `0001` migrations gained traceability headers and equivalent bounded-text constraints; PostgreSQL SHA-256 `0077AB471F8DEE6FC89B64B2E3346F1D33E435404B753ACC6D4F3DAF4DB59EC6`; SQLite SHA-256 `4A5AA7F9A659FB35CE0DEBAC12424C83AFA81440B41727DA083716DCC030B5F9`
- Security review: no secret-bearing API, audit or log fields were added; password inputs and hashes remain zeroized/redacted; SQL remains literal and parameter-bound; anonymous readiness errors remain generic
- Builder review: diff reviewed for engine parity, authorization/contract drift, migration checksums, redaction, failure classification and startup/shutdown behavior; no unresolved finding remains from the second-milestone validation
- Independent validation: the independent validator's failing report against `c6f0161d5d8840be50be4d3937eac32b9db901aa` is the input to this remediation; a new independent post-remediation verdict is still pending

### Validator finding remediation

| Finding | Remediation and evidence |
|---|---|
| PostgreSQL/SQLite bounded-text drift | SQLite now enforces every PostgreSQL `VARCHAR` maximum, the repository pre-validates bounded fields, and PostgreSQL SQLSTATE `22001` maps to `ConstraintViolation`; the unchanged shared suite asserts the 121-character organization-name case on both engines. |
| SQLite lexical path bypass | configured paths are resolved through the nearest existing ancestor, canonicalized and normalized before current-directory/repository checks; the CLI regression uses a sibling/`..` path that resolves into the repository. |
| Incomplete shared repository contract | the common suite now covers scoped project uniqueness, cross-organization reuse, username collision, foreign keys, invalid roles, length parity, optimistic version conflicts, stable audit ordering, concurrent writes and controlled shutdown; both engines invoke it. Both engines also force and verify a mid-bootstrap rollback. |
| No HTTP readiness during migration | server mode binds and polls the HTTP server concurrently with schema initialization; the production startup helper is tested at 503 during gated initialization and 200 after completion. |
| Argon2 on Tokio worker | `BootstrapService` depends on an async password-hashing port; the server adapter performs hash and verify operations with `spawn_blocking`. |
| stdin over-limit suffix ignored | stdin reads one detection byte beyond the maximum CRLF-framed input and rejects any trailing data; the CLI regression covers 1024 bytes plus CRLF plus extra bytes. |
| Stale implementation evidence | this evidence is tied to the current base commit and explicitly identifies the uncommitted target state, current hashes, commands and post-remediation status. |
| Missing migration traceability | both unreleased `0001` migration files now carry requirement and milestone headers. |

### Toolchain and service

| Tool | Version |
|---|---|
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| Node.js | `v24.15.0` |
| pnpm | `11.7.0` |
| Docker Engine | `29.6.1` |
| PostgreSQL image | `postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7` |

### Exact post-remediation commands and exit codes

| Command | Exit code | Result |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | Workspace lint passed without warnings. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test -p takt-persistence --test postgres_contract -- --test-threads=1` | 0 | Real PostgreSQL 16.9 engine contract passed, including shared behavior and forced rollback. |
| `$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'; cargo test --workspace --all-features -- --test-threads=1` | 0 | Full Rust workspace, both engines, CLI, production startup composition and doctests passed. |
| `cargo deny check` | 0 | Advisories, licenses and sources passed; duplicate-version diagnostics remain warnings. |
| `cargo audit` | 0 | 261 locked Rust dependencies scanned; no vulnerability reported. |
| `pnpm install --frozen-lockfile` | 0 | Locked Node dependencies unchanged. |
| `pnpm contracts:validate` | 0 | OpenAPI, JSON Schema, Proto and all Gherkin files passed. |
| `pnpm check:architecture` | 0 | Crate directions, framework exclusions and unsafe guards passed. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web generated output had no drift. |
| `pnpm check:secrets` | 0 | Repository secret-pattern scan passed. |
| `pnpm test:tools` | 0 | Four repository-tool tests passed. |
| `pnpm audit --audit-level high` | 0 | No known high-severity Node vulnerability. |
| `pnpm check:licenses` | 0 | Production and development Node license gates passed. |
| `pnpm lint` | 0 | Frontend ESLint passed. |
| `pnpm typecheck` | 0 | Frontend strict TypeScript check passed. |
| `pnpm test --run` | 0 | Frontend Vitest suite passed. |
| `pnpm build` | 0 | Vite production build passed. |
| `pnpm playwright test` | 0 | Chromium bootstrap E2E passed. |
| `cargo build --workspace --all-features --release --locked` | 0 | Optimized workspace/server build passed. |
| `git diff --check` | 0 | No whitespace errors. |

### Engine evidence

| Behavior | SQLite | PostgreSQL 16.9 |
|---|---|---|
| Empty and repeated migration | passed | passed |
| Unknown newer schema rejection | passed | passed |
| Shared repository error/constraint contract | passed | passed |
| Scoped uniqueness and foreign keys | passed | passed |
| Optimistic version conflict | passed | passed |
| Stable audit ordering and immutability | passed | passed |
| Concurrent write classification | passed | passed |
| Atomic/idempotent bootstrap | passed | passed |
| Forced mid-transaction rollback | passed | passed |
| Controlled pool shutdown | passed | passed |
| Database-unavailable readiness | pool/schema checks passed | real connection outage passed |

### Known milestone limits

Sessions, API tokens, HTTP login, full RBAC enforcement and general audit CRUD remain explicitly out of scope. Backup verification is a later `PRD-NFR-007` deliverable. No compile-time `query!` macros are used, so no `.sqlx` offline cache is required. Clean-checkout and second independent post-remediation validation require a committed target and remain pending.
