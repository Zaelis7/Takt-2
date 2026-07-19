# Local development

The bootstrap repository is pinned to Rust 1.95.0, Node.js 24.15.0 and pnpm
11.7.0. `Cargo.lock` and `pnpm-lock.yaml` are part of the source tree. Tests and
contract validation use only local fixtures and loopback listeners; they do not
call public services.

## Initial setup

```text
corepack enable
corepack prepare pnpm@11.7.0 --activate
pnpm install --frozen-lockfile
```

Rustup reads `rust-toolchain.toml` automatically. The policy gates additionally
require `cargo-deny` 0.20.2 and `cargo-audit` 0.22.2:

```text
cargo install --locked cargo-deny --version 0.20.2
cargo install --locked cargo-audit --version 0.22.2
```

Install the browser used by the bootstrap E2E test once:

```text
pnpm exec playwright install chromium
```

## Development servers

Run the API and embedded web build on the secure loopback default
`http://127.0.0.1:8080`. The local profile automatically creates and migrates
SQLite under the platform data directory, never under the repository or current
working directory:

```text
cargo run --locked -p takt-server
```

For frontend development with hot reload:

```text
pnpm --dir web dev
```

`/health/live` remains independent of persistence. `/health/ready` returns 200
only after the database is reachable and its embedded migration set is current;
anonymous 503 responses contain no connection or migration details.

Create the first local owner non-interactively (the password is read only from
stdin):

```text
cargo run --locked -p takt-server -- admin bootstrap --username admin --password-stdin --output json < /run/secrets/takt_admin_password
```

See `docs/persistence.md` for configuration, migration modes and stable exit
codes.

## Real PostgreSQL contract service

The PostgreSQL suite intentionally fails instead of skipping when
`TAKT_TEST_POSTGRES_URL` is absent. Start the pinned PostgreSQL 16.9 test image
on loopback; `trust` authentication is limited to this disposable local test
container and avoids committing a test secret:

```text
docker run --rm --name takt-postgres-test -p 127.0.0.1:55432:5432 -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=takt_test postgres:16.9-alpine@sha256:7c688148e5e156d0e86df7ba8ae5a05a2386aaec1e2ad8e6d11bdf10504b1fb7
```

In a second shell:

```text
TAKT_TEST_POSTGRES_URL=postgresql://postgres@127.0.0.1:55432/takt_test cargo test -p takt-persistence --test postgres_contract -- --test-threads=1
```

On PowerShell, set
`$env:TAKT_TEST_POSTGRES_URL='postgresql://postgres@127.0.0.1:55432/takt_test'`
before the Cargo command. The test refuses to reset a database whose name does
not begin with `takt_test`.

## Generation and drift

```text
pnpm generate:openapi
pnpm generate:proto
pnpm build
pnpm check:generated
```

OpenAPI generates strict TypeScript declarations. `protox` compiles the Proto
contract in pure Rust and Prost emits the committed Rust types. The Vite
production output is committed because `takt-server` embeds it at compile time.
`pnpm check:generated` independently regenerates all three outputs in temporary
directories and compares their bytes.

## Contract and architecture checks

```text
pnpm contracts:openapi
pnpm contracts:schema
pnpm contracts:proto
pnpm contracts:gherkin
pnpm check:architecture
```

The architecture check rejects unknown workspace crates, forbidden internal
dependency directions, runtime/web/database frameworks in `takt-domain`, and a
missing `#![forbid(unsafe_code)]` crate attribute.

## Complete local gates

```text
pnpm install --frozen-lockfile
pnpm contracts:validate
pnpm check:architecture
pnpm check:generated
pnpm check:secrets
pnpm test:tools
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
cargo audit
pnpm audit --audit-level high
pnpm check:licenses
pnpm lint
pnpm typecheck
pnpm test --run
pnpm build
pnpm playwright test
cargo build --workspace --all-features --release --locked
```

The complete Rust test command requires the real PostgreSQL service and
`TAKT_TEST_POSTGRES_URL` described above. SQL statements use bound runtime
queries rather than `query!` compile-time macros, so no `.sqlx` offline cache is
required. The migration files are compile-time embedded and migration drift is
covered by empty/repeated/unknown-version tests on both engines.

## Reproducible production build

```text
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --release -p takt-server
```

The resulting `takt-server` binary contains the exact files from `web/dist`.
