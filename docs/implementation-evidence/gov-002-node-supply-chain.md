## Implementation Evidence

- Evidence date: `2026-07-21`
- Target: uncommitted `GOV-002` working tree based on commit `1f3aa3c62966faba93923dd9d189fcd8929094b2`; no commit, push or pull request was requested
- Requirements: `PRD-API-002`
- Contracts changed: no; OpenAPI and generated API types are unchanged
- Migrations: none
- Tests added: `tools/node-supply-chain.test.mjs` rejects every `js-yaml` package older than 4.3.0 in the committed pnpm lockfile
- Security review: the workspace-wide override changes the only resolved `js-yaml` path from vulnerable development dependency 4.2.0 to MIT-licensed 4.3.0. No runtime dependency, secret, external data flow or audit behavior changed. The full Node audit reports no known vulnerability, license policy passes, and no audit exception was added.
- Known limitations: `PRD-API-002` remains partial because common network contracts, runtime conformance and comparison with a released compatibility baseline remain separate work. Independent commit-bound validation is pending. The full Rust workspace test cannot pass locally without the required PostgreSQL service.
- Reviewer verdict: builder-side diff review passed for lockfile scope, pnpm 11 configuration, regression coverage, unchanged generated output and unchanged audit policy; independent review is pending
- Validator verdict: `implemented`, not `verified`; all focused and independently executable repository gates pass, while the mandatory PostgreSQL contract is not verified in this environment

### Scope and test-first evidence

The preflight estimated 120 handwritten lines and 25 validation minutes. The package includes the pnpm override, lockfile update, a lockfile regression test, tracking and this evidence. It excludes OpenAPI semantics, generated API changes, runtime behavior, unrelated dependency upgrades and independent release validation. The final manual count is 166 changed handwritten lines excluding the generated lockfile, safely below the 600-line review threshold.

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` (selection baseline) | 0 | The clean baseline validated 57 requirements, 82 packages and 9 findings; `GOV-002` was the first unblocked package addressing a high security finding. |
| `node --test tools/node-supply-chain.test.mjs` (initial test-first run) | 1 | The new `PRD-API-002` test found `js-yaml@4.2.0` in `pnpm-lock.yaml`. |
| `pnpm install --lockfile-only` with the first package-local override attempt | 0 | pnpm 11 warned that `package.json#pnpm.overrides` was ignored; the regression test stayed red and the ineffective change was removed. |
| `pnpm install --lockfile-only` with the workspace override | 0 | The lockfile resolved `js-yaml@4.3.0`. |
| `pnpm install --frozen-lockfile` | 0 | The pinned workspace installed without lockfile drift. |
| `node --test tools/node-supply-chain.test.mjs` | 0 | The lockfile contains one `js-yaml` package at 4.3.0 and no older package. |
| `pnpm why js-yaml` | 0 | The only path is `openapi-typescript` through `@redocly/openapi-core` to `js-yaml@4.3.0`. |

### Focused package gates

| Command | Exit code | Result |
|---|---:|---|
| `pnpm audit --audit-level high` | 0 | No known Node vulnerability was reported; the policy was not weakened. |
| `pnpm contracts:validate` | 0 | OpenAPI, Config Schema, Proto, Gherkin syntax and CheckSpec golden fixtures passed. |
| `pnpm check:generated` | 0 | OpenAPI, Proto and embedded-web generated artifacts have no drift. |
| `pnpm check:licenses` | 0 | Production and development Node license policy passed. |
| `pnpm check:tracking` | 0 | Both the active preflight and final implemented ledger validated with 57 requirements, 82 packages and 9 findings. |
| `pnpm test:tools` | 0 | All 18 repository tool tests passed, including the new supply-chain regression. |

### Repository gates

| Command | Exit code | Result |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | The complete workspace passed Clippy with warnings denied. |
| `cargo test --workspace --all-features` | 101 | All suites reached before persistence passed; the mandatory PostgreSQL contract then failed because `TAKT_TEST_POSTGRES_URL` is absent. This is not counted as a pass. |
| `cargo deny check` | 0 | Advisory, ban, license and source policy passed; duplicate-version notices remain warnings. |
| `cargo audit` | 0 | No RustSec vulnerability caused failure. |
| `cargo build --workspace --all-features --release --locked` | 0 | The optimized workspace build passed. |
| `pnpm check:architecture` | 0 | Dependency directions and unsafe-code guards passed. |
| `pnpm check:secrets` | 0 | Secret scanning passed for 104 source files. |
| `pnpm lint` | 0 | Web lint passed with zero warnings. |
| `pnpm typecheck` | 0 | Strict TypeScript checking passed. |
| `pnpm test --run` | 0 | The web unit suite passed. |
| `pnpm build` | 0 | The production web bundle built successfully. |
| `pnpm playwright test` | 0 | The Chromium accessibility smoke test passed. |
| `git diff --check` | 0 | No whitespace error was reported. |
| Docker/PostgreSQL availability probe | 1 | The Docker client exists but its engine is not running; native `psql` and `pg_isready` are absent, so the PostgreSQL gate cannot be retried locally. |
