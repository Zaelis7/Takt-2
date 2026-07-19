# Dependency and license review

Review date: 2026-07-19. Every direct dependency is pinned exactly and both
ecosystem lockfiles are committed. Current releases, declared licenses and
upstream repositories were checked through the crates.io and npm registries.
`cargo deny check`, `cargo audit`, `pnpm audit` and `pnpm check:licenses` are
repository gates. The Node license gate enforces a permissive SPDX allow-list;
CC-BY-4.0 `caniuse-lite`, MPL-2.0 `lightningcss` platform tooling and
Python-2.0 `argparse` are package-scoped development-tool exceptions and are
rejected if they enter the production dependency graph.

## Rust runtime dependencies

| Dependency | Need | License | Supply-chain and replacement note |
|---|---|---|---|
| Axum 0.8.9 | Contracted HTTP router and JSON responses | MIT | Tokio project; replaceable by another Tower HTTP boundary without changing domain code. |
| Tokio 1.53.0 | Async listener and integration-test I/O | MIT | Contracted runtime; feature set is limited to the capabilities used. |
| Serde 1.0.229 | Contract JSON serialization | MIT OR Apache-2.0 | De-facto Rust serialization boundary; isolated to API/generated protocol use. |
| UUID 1.24.0 | Typed domain parsing and UUIDv7 request IDs | MIT OR Apache-2.0 | Generation stays outside the domain; another ID source can be injected later. |
| rust-embed 8.12.0 | Embed the reproducible Vite output in the server binary | MIT | Adds compile-time macros but no service; can be replaced by generated `include_bytes!` mappings. |
| Prost 0.14.4 | Rust message types for the normative Proto contract | Apache-2.0 | Contracted stack; wire format remains protobuf if generation changes. |

## Rust build and repository tooling

| Dependency | Need | License | Supply-chain and replacement note |
|---|---|---|---|
| Prost Build 0.14.4 | Deterministic Prost source generation | Apache-2.0 | Used only by `xtask`, not the server runtime. |
| protox 0.9.1 | Pure-Rust Proto compiler, avoiding a host `protoc` dependency | MIT OR Apache-2.0 | Used only by `xtask`; vendored `protoc` is the fallback. |
| tempfile 3.27.0 | Isolated drift-generation directories with automatic cleanup | MIT OR Apache-2.0 | Used only by `xtask`; a manually managed OS temp directory is the fallback. |

`cargo deny` reports a small set of duplicate transitive versions inside the
Proto code-generation toolchain (`hashbrown`, `logos`, `logos-codegen`,
`logos-derive`, and `syn`). They are build tooling only; licenses and advisories
pass. They remain warnings so upstream-compatible versions are not forced.

## Web runtime dependencies

| Dependency | Need | License | Supply-chain and replacement note |
|---|---|---|---|
| React 19.2.7 | Contracted UI framework | MIT | Runtime bundle dependency; component boundaries allow later upgrades. |
| React DOM 19.2.7 | Browser renderer and server-free unit rendering | MIT | Runtime bundle dependency paired exactly with React. |

The initial production bundle is approximately 191 kB JavaScript (60 kB gzip)
and 1.1 kB CSS (0.6 kB gzip), below the UI budget. It contains no monitoring,
authentication or persistence client code.

## Web and contract tooling

| Group | Direct dependencies | Declared licenses | Reason |
|---|---|---|---|
| Build/types | Vite 8.1.5, React plugin 6.0.3, TypeScript 5.9.3, React type packages | MIT / Apache-2.0 | Contracted strict React/TypeScript production build. |
| Lint/test | ESLint 10.7.0, typescript-eslint 8.64.0, React hooks/refresh plugins, Vitest 4.1.10, Playwright 1.61.1 | MIT / Apache-2.0 | Static, unit and real-browser gates. |
| OpenAPI | openapi-typescript 7.13.0, Redocly CLI 2.39.0 | MIT | Generated API declarations and OpenAPI 3.1 linting. |
| Schema | AJV 8.20.0, ajv-formats 3.0.1, YAML 2.9.0 | MIT / ISC | Draft-2020-12 validation of the committed YAML example. |
| Gherkin | Cucumber Gherkin 41.0.0 and Messages 33.0.4 | MIT | Official parser for all acceptance feature files. |

The lockfile includes MPL-2.0 `lightningcss` platform packages as optional,
development-only Vite tooling and Python-2.0 `argparse` as transitive tooling.
Neither is embedded in the JavaScript/CSS output or the Rust server binary, and
their licenses impose no terms on generated Takt source or assets. No Git
dependency, wildcard version or required external runtime service is present.
