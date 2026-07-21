## Implementation Evidence

- Evidence date: `2026-07-21`
- Target: uncommitted `SPEC-014` working tree based on commit `97d32704476410f0545b974ff3a00643b038cd14`; no commit, push or pull request was requested
- Requirements: none; this package repairs specification-package integrity without claiming product behavior
- Contracts changed: specification index only; `specs/README.md` no longer advertises the never-existing `AGENTS.template.md`. OpenAPI, Config Schema, Proto and Gherkin are unchanged
- Migrations: none
- Tests added: four cases in `tools/check-spec-index.test.mjs` for existing literals/globs, a missing path, an unmatched glob and a path escaping the specification package
- Security review: no runtime, authorization, data, secret or external-flow change; the checker is read-only, confines indexed paths to `specs/` and adds no dependency
- Known limitations: independent clean-checkout review is pending. The mandatory PostgreSQL contract remains unavailable locally and is unrelated to this specification-only change.
- Reviewer verdict: builder-side diff review passed for historical evidence, narrow spec scope, path confinement, glob behavior and absence of product-contract drift; independent review is pending
- Validator verdict: `implemented`, not `verified`; all locally satisfiable gates pass, but the full workspace test is not green because the required PostgreSQL test URL is unavailable

### Decision and test-first evidence

Repository history shows that `specs/AGENTS.template.md` was absent when the index row was introduced and has never existed in a later commit. Chapter 09 requires the actual root `AGENTS.md` and defines no template contract. Removing the false row therefore repairs the index without inventing a new artifact or semantics.

The preflight estimated 230 handwritten lines and 15 validation minutes. The final working-tree count is 266 changed handwritten lines, remaining well below the hard 800-line/30-minute package limits.

| Command | Exit code | Result |
|---|---:|---|
| `pnpm check:tracking` (baseline) | 0 | The clean baseline validated 57 requirements, 82 packages and 9 findings. |
| `node --test tools/check-spec-index.test.mjs` (initial test-first run) | 1 | The new suite failed with `ERR_MODULE_NOT_FOUND` before the checker existed. |
| `node --test tools/check-spec-index.test.mjs` | 0 | Four positive, negative and path-safety cases passed. |
| `pnpm check:spec-index` before the spec fix | 1 | The checker reproduced `missing indexed paths: AGENTS.template.md`. |
| `pnpm check:spec-index` after the spec fix | 0 | All 16 indexed literal/glob paths resolve inside `specs/`. |
| `pnpm test:tools` | 0 | Twenty-eight repository tool tests passed. |

### Repository gates

| Command | Exit code | Result |
|---|---:|---|
| `pnpm install --frozen-lockfile` | 0 | Pinned dependencies were already current; the lockfile did not change. |
| `pnpm contracts:validate`, `pnpm acceptance:check` | 0 | Machine contracts, Gherkin syntax, CheckSpec fixtures and 37-entry acceptance inventory passed unchanged. |
| `cargo fmt --all -- --check` | 0 | Rust formatting passed. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | The complete workspace passed Clippy with warnings denied. |
| `cargo test --workspace --all-features` | 101 | The mandatory PostgreSQL contract failed because `TAKT_TEST_POSTGRES_URL` is unavailable; this is not counted as a pass. |
| `cargo test --workspace --all-features --exclude takt-persistence` | 0 | All non-persistence workspace suites passed. |
| `cargo test -p takt-persistence --lib` and `cargo test -p takt-persistence --test sqlite_contract -- --test-threads=1` | 0 | Persistence library tests and all six SQLite contract cases passed. |
| `cargo deny check`, `cargo audit` | 0 | Rust advisory, ban, license and source gates passed; duplicate-version notices remain warnings. |
| `pnpm audit --audit-level high`, `pnpm check:licenses` | 0 | Node vulnerability and license gates passed. |
| `pnpm lint`, `pnpm typecheck`, `pnpm test --run`, `pnpm build` | 0 | Frontend lint, strict typing, unit test and production build passed. |
| `pnpm playwright test` | 0 | The Chromium accessibility smoke test passed. |
| `pnpm check:architecture`, `pnpm check:generated`, `pnpm check:secrets` | 0 | Architecture, unsafe-code, generated-drift and secret gates passed. |
| `cargo build --workspace --all-features --release --locked` | 0 | The optimized locked workspace build passed. |
| `pnpm check:tracking` | 0 | The final ledger validates 57 requirements, 82 packages and 9 findings. |
| `git diff --check` | 0 | No whitespace error was reported. |
