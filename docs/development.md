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
`http://127.0.0.1:8080`:

```text
cargo run --locked -p takt-server
```

For frontend development with hot reload:

```text
pnpm --dir web dev
```

The bootstrap readiness endpoint returns `{"status":"ok"}` once the HTTP
composition root is serving. No database, key store or worker exists in this
milestone, so there are no external readiness dependencies yet.

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
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
cargo audit
pnpm audit --audit-level high
pnpm licenses list --long
pnpm lint
pnpm typecheck
pnpm test --run
pnpm build
pnpm playwright test
cargo build --workspace --all-features --release --locked
```

## Reproducible production build

```text
pnpm install --frozen-lockfile
pnpm build
cargo build --locked --release -p takt-server
```

The resulting `takt-server` binary contains the exact files from `web/dist`.

