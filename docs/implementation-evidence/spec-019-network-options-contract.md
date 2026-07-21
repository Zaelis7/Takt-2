## Implementation Evidence

- Evidence date: `2026-07-21`
- Target: uncommitted `SPEC-019` working tree based on commit `1f3aa3c62966faba93923dd9d189fcd8929094b2`, layered on the uncommitted implemented `GOV-002` change; no commit, push or pull request was requested
- Requirements: `PRD-API-002`, `PRD-MON-002`
- Contracts changed: yes; `specs/contracts/openapi.yaml`, `specs/contracts/takt-config.schema.json`, `specs/contracts/probe.proto` and the normative mapping in `specs/04-probes-and-checks.md`; generated OpenAPI and Proto types were refreshed
- Migrations: none
- Tests added: cross-contract common-network assertions and five negative network cases in the CheckSpec golden suite, positive network fields across all six active network checks, and a generated Proto round-trip proving proxy credentials remain ephemeral references
- Security review: resolver and proxy URIs are authority-only, reject userinfo/path/query/fragment, and allow only declared schemes. Proxy auth requires username and password SecretRefs in OpenAPI/config and only `ephemeral_key` references in Proto. Push cannot receive outbound network options; DNS and ICMP cannot receive proxies. No runtime data flow, stored secret, audit behavior or Egress policy changed.
- Known limitations: this is a contract-only package. Framework-free Rust domain types remain in `MON-011`; resolver/proxy executors and runtime conformance remain in later CHECK packages. No released OpenAPI baseline exists for compatibility comparison. Independent commit-bound validation is pending, and the full Rust workspace test cannot pass locally without the required PostgreSQL service.
- Reviewer verdict: builder-side contract diff review passed for field applicability, naming, URI bounds, SecretRef isolation, additive Proto tags, generated drift and negative cases; independent review is pending
- Validator verdict: `implemented`, not `verified`; all focused and independently executable repository gates pass, while the mandatory PostgreSQL contract is not verified in this environment

### Scope and test-first evidence

The preflight estimated 560 handwritten lines and 25 validation minutes. The package includes the normative spec, three machine contracts, positive and negative fixtures, cross-contract and Proto tests, generated types, tracking and this evidence. It excludes domain/runtime implementation, Push network options, DNS/ICMP proxies, migrations, API endpoints, UI and Egress-policy changes.

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` (selection baseline) | 0 | The stacked baseline validated 57 requirements, 82 packages and 9 findings; `SPEC-019` was the first unblocked 0.1 package addressing a high contract finding. |
| `node --test tools/check-spec-contract.test.mjs` (baseline) | 0 | The three pre-existing per-kind CheckSpec tests passed before adding common-network expectations. |
| `node --test tools/check-spec-contract.test.mjs` (initial test-first run) | 1 | Valid fixtures were rejected for missing proxy/resolver/address-family fields, negative Proto fixtures could not find them, and the common schema test failed because the definitions were absent. |
| `node --test tools/check-spec-contract.test.mjs` | 0 | Four tests pass across six active network checks and Push exclusion; fifteen invalid fixtures cover existing boundaries plus proxy scheme, embedded credentials, literal proxy auth, resolver scheme and address family. |
| `pnpm contracts:openapi` | 0 | OpenAPI 3.1 lint passed. |
| `pnpm contracts:schema` | 0 | The Config Schema and example validation passed. |
| `cargo run --locked -p xtask -- check-proto` (before generation) | 1 | The additive Proto contract correctly triggered the generated-type drift gate. |
| `pnpm generate:openapi` | 0 | TypeScript OpenAPI types were regenerated. |
| `cargo run --locked -p xtask -- generate-proto` | 0 | Prost types were regenerated from the additive wire contract. |
| `cargo test -p takt-probe-protocol` | 0 | Three generated-contract tests passed, including proxy SecretRef and address-family round-trip behavior. |
| `pnpm contracts:validate` | 0 | OpenAPI, Config Schema, Proto, Gherkin syntax and all CheckSpec golden cases passed. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web generated artifacts have no drift. |
| `pnpm check:tracking` | 0 | Both the active preflight and final implemented ledger validated with 57 requirements, 82 packages and 9 findings. |

### Repository gates

| Command | Exit code | Result |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | The complete workspace passed Clippy with warnings denied. |
| `cargo test --workspace --all-features` | 101 | All suites reached before persistence passed; the mandatory PostgreSQL contract then failed because `TAKT_TEST_POSTGRES_URL` is absent. This is not counted as a pass. |
| `cargo deny check` | 0 | Advisory, ban, license and source policy passed; duplicate-version notices remain warnings. |
| `cargo audit` | 0 | No RustSec vulnerability caused failure. |
| `cargo build --workspace --all-features --release --locked` | 0 | The optimized workspace build passed. |
| `pnpm install --frozen-lockfile` | 0 | The pinned workspace installed without lockfile drift. |
| `pnpm audit --audit-level high` | 0 | No known Node vulnerability was reported. |
| `pnpm check:licenses` | 0 | Production and development Node license policy passed. |
| `pnpm check:architecture` | 0 | Dependency directions and unsafe-code guards passed. |
| `pnpm check:secrets` | 0 | Secret scanning passed for 105 source files. |
| `pnpm test:tools` | 0 | All 19 repository tool tests passed. |
| `pnpm lint` | 0 | Web lint passed with zero warnings. |
| `pnpm typecheck` | 0 | Strict TypeScript checking passed. |
| `pnpm test --run` | 0 | The web unit suite passed. |
| `pnpm build` | 0 | The production web bundle built successfully. |
| `pnpm playwright test` | 0 | The Chromium accessibility smoke test passed. |
| `git diff --check` | 0 | No whitespace error was reported. |
| Docker/PostgreSQL availability probe | 1 | The Docker client exists but its engine is not running; native `psql` and `pg_isready` are absent, so the PostgreSQL gate cannot be retried locally. |
