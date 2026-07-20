# AGENTS.md – Takt Repository Instructions

These instructions apply to the entire repository. More specific `AGENTS.md` files may add constraints but must not weaken these rules.

## Mission

Implement Takt according to the versioned specification in `specs/`. Do not invent scope, report unverified completion, or trade correctness and security for speed.

## Required reading

Before changing code, read:

1. `specs/README.md`
2. the affected numbered spec chapters
3. affected files in `contracts/` and `acceptance/`
4. existing ADRs, code and tests in the change path

Use requirement IDs in the issue, tests, pull request and evidence.

## Contract order

When sources disagree, use this order and stop to fix the contradiction:

1. machine-readable contracts
2. acceptance scenarios
3. release exit criteria
4. numbered product and architecture specs
5. examples

Never silently reinterpret a contract.

## Architecture invariants

- Keep the system a modular monolith plus the separate probe/browser workers through 0.3.
- Keep domain logic free of database, HTTP and runtime frameworks.
- Never classify a Takt infrastructure or probe failure as a target failure.
- Store evaluation, transition and outbox event atomically.
- Treat every ingest and write boundary as idempotent.
- The Web UI uses only the public API.
- PostgreSQL and SQLite share the same domain and repository behavior.
- Do not expose secrets through APIs, logs, audit, exports, metrics, traces or test fixtures.
- Do not add a broker, microservice, SQL engine, arbitrary script execution or plugin runtime without an approved spec/ADR.

## Change workflow

1. Preserve unrelated user changes and inspect the current tree.
2. State the requirement IDs and risk areas.
3. Add a failing behavior or contract test.
4. Update contracts in the same change when public behavior changes.
5. Implement the smallest complete vertical behavior.
6. Run focused checks, then every required repository gate.
7. Review the diff for contract drift, authorization, migration, redaction and observability.
8. Produce Implementation Evidence as defined by `specs/09-ai-implementation.md`.

Do not disable, weaken, delete or blindly update tests to obtain a pass. A skipped or unavailable check is not a pass.

## Implementation tracking and automatic next-task selection

The operational implementation state is maintained in `docs/implementation-tracking/`.
Before selecting or changing a work package, read its `README.md`,
`requirements.yaml`, `work-packages.yaml`, `findings.yaml` and
`current-status.md`, then run `pnpm check:tracking`.

When the user asks to continue with the next open task without naming a package,
select it autonomously. Do not ask the user to choose between ordinary in-scope
packages. Apply this deterministic order:

1. Resume an `in_progress` package whose blockers are resolved.
2. Otherwise consider only `planned` packages whose `depends_on` packages are
   `implemented` or `verified` and whose referenced findings do not require an
   unresolved owner decision.
3. Prefer packages for the earliest unfinished release: `0.1`, then `0.2`, then
   `0.3`, then `post-0.3`. A `cross-release` package is considered at the earliest
   release that depends on it.
4. Within the same release, first resolve open critical/high contract, security,
   migration or evidence findings that block implementation; otherwise use file
   order in `work-packages.yaml`.
5. Work on exactly one package at a time. If its estimated implementation or
   validation size violates the package limits in `specs/09-ai-implementation.md`,
   split it in the tracking files first and execute only the first new unblocked
   package.

For the selected package, derive the complete prompt content from its registry
entry and the specs: scope, exclusions, contracts, risks, acceptance, gates and
evidence. Perform the normal test-first workflow without requiring the user to
repeat these instructions. In the same change, update affected requirement
coverage/verification, package status, findings and Implementation Evidence.
Never mark a package `verified` without the required independent review and
commit-bound validation. Stop for user input only when the specs require an
owner decision or the selected package cannot be made unblocked within scope.

## Rust rules

- Stable pinned toolchain and committed `Cargo.lock`.
- `#![forbid(unsafe_code)]` in first-party crates unless an approved ADR says otherwise.
- No panic-based handling of user input or external failure.
- Avoid `unwrap` and `expect` in production paths.
- Use typed domain IDs/errors and explicit timeouts/cancellation.
- Keep blocking work away from async worker threads.

## Web rules

- TypeScript strict; no unchecked `any`.
- Generate API types from OpenAPI.
- Every async view has loading, empty, error and success states.
- Permission hiding in the UI is never the authorization boundary.
- Keyboard use, visible focus and accessible names are release requirements.

## Data and security rules

- Bind SQL parameters; never interpolate user values or unchecked sort fields.
- Published migrations are immutable and forward-only.
- Test migrations on PostgreSQL and SQLite fixtures.
- Resolve secrets only at the last responsible boundary and redact centrally.
- Update threat model and audit behavior for every new external data flow.
- Browser checks run only in the isolated worker with declarative steps.

## Standard gates

Use the repository-provided pinned commands. At minimum they must cover:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
cargo audit
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test --run
pnpm build
pnpm playwright test
```

Also validate OpenAPI, JSON Schema, Proto, migrations, generated-code drift and release-relevant acceptance scenarios.

## Escalate instead of assuming

Stop and request a product decision for breaking contracts, scope outside the roadmap, weaker security, material data-loss risk, a new required external service, or licensing/business-model changes. Otherwise make the narrowest documented assumption and continue.
