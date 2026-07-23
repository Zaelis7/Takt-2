# Persistence, migrations and local administrator bootstrap

This milestone implements the local identity, server-side session, recovery and
API-token persistence slices of `PRD-IAM-001`. It prepares `PRD-IAM-003`,
`PRD-IAM-004` and `PRD-IAM-005` with organization/project boundaries, stable
roles, append-only audit storage and actor-bound write idempotency.

## Database configuration

Configuration follows built-in defaults, then `TAKT_*` environment variables,
then explicit migration CLI flags. Database URLs are not accepted as CLI
arguments because they may contain credentials.

| Variable | Local default | Constraint |
|---|---|---|
| `TAKT_PROFILE` | `local` | `local` or `production` |
| `TAKT_DATABASE_ENGINE` | `sqlite` | `sqlite` or `postgresql`; production requires explicit PostgreSQL |
| `TAKT_DATA_DIR` | OS application-data directory plus `Takt` | absolute path outside the current working tree/repository; SQLite file is `takt.sqlite3` below it |
| `TAKT_DATABASE_URL` | none | PostgreSQL URL; mutually exclusive with `_FILE` |
| `TAKT_DATABASE_URL_FILE` | none | file containing the PostgreSQL URL, max 8 KiB |
| `TAKT_DB_MAX_CONNECTIONS` | SQLite 5, PostgreSQL 10 | 1–100 |
| `TAKT_DB_CONNECTION_TIMEOUT_MS` | 5000 | 100–120000 |
| `TAKT_DB_QUERY_TIMEOUT_MS` | 5000 | 100–300000 |
| `TAKT_DB_SHUTDOWN_TIMEOUT_MS` | 5000 | 100–120000 |
| `TAKT_SQLITE_BUSY_TIMEOUT_MS` | 5000 | 100–120000 and capped by query timeout |

SQLite connections enable foreign keys, WAL and `synchronous=NORMAL`. The
database path is always below an explicit absolute data directory or the
platform application-data directory. Unix deployments additionally enforce
mode 0700 on that directory and 0600 on the database file; Windows inherits the
application-data ACL. PostgreSQL connections set a bounded
pool, acquisition timeout and server-side statement timeout. Configuration
debug output and failures redact database URLs and secret-source paths.

## Migration operation

Migration files are separate and forward-only:

- `migrations/postgres/0001_persistent_identity.sql` and `migrations/sqlite/0001_persistent_identity.sql`
- `migrations/postgres/0002_sessions.sql` and `migrations/sqlite/0002_sessions.sql`
- `migrations/postgres/0003_recovery_tokens.sql` and `migrations/sqlite/0003_recovery_tokens.sql`
- `migrations/postgres/0004_api_tokens.sql` and `migrations/sqlite/0004_api_tokens.sql`
- `migrations/postgres/0005_api_token_idempotency.sql` and `migrations/sqlite/0005_api_token_idempotency.sql`
- `migrations/postgres/0006_api_token_actor.sql` and `migrations/sqlite/0006_api_token_actor.sql`

The local profile migrates automatically. Production checks the current schema
and requires an explicit migration command:

```text
takt-server --migrate-only
takt-server --no-auto-migrate
```

`--migrate-only` applies the embedded engine-specific migration and exits.
`--no-auto-migrate` never applies pending work. A database with a migration
version newer than the binary supports aborts startup in both modes. Migration
checksums are validated by SQLx on every migration/repeated start. Readiness is
unavailable before or during migration; liveness never depends on the database.

## Initial schema

Both engines implement the same ten domain tables:

| Table | Purpose and principal constraints |
|---|---|
| `organizations` | UUIDv7 primary key, globally unique normalized slug, UTC timestamps, version |
| `projects` | UUIDv7, organization foreign key, unique organization/slug, UTC timestamps, version |
| `users` | UUIDv7, globally unique normalized local username, UTC timestamps, version |
| `local_credentials` | one credential per user, Argon2id PHC hash only, UTC timestamps, version |
| `memberships` | organization/project scope, user foreign key, stable role check, UTC timestamps, version |
| `audit_events` | append-only trigger, mutually exclusive user/API-token actor references, resource/request identifiers, redacted JSON metadata, UTC occurrence time |
| `sessions` | UUIDv7, organization/user scope, hashed cookie and CSRF values, expiry/revoke state, version |
| `recovery_tokens` | UUIDv7, organization/user scope, hashed token value, expiry/single-consumption state, version |
| `api_tokens` | UUIDv7, organization/project scope, non-secret lookup prefix, Argon2id hash, exact scopes, network/expiry/revoke state, version |
| `api_token_idempotency` | actor/method/path/key/request-hash identity, exact 24-hour expiry, safe mutation result or encrypted Create replay |

Role checks already accept `owner`, `admin`, `editor`, `operator` and `viewer`.
Foreign-key, slug, membership and audit-time indexes are created in the first
migration. PostgreSQL stores timestamps as `TIMESTAMPTZ(6)`; SQLite stores UTC
Unix epoch microseconds as integers.

## Local administrator command

```text
takt-server admin bootstrap --username admin --password-stdin [--output json]
```

The command has no password-value argument and opens no prompt. It reads UTF-8
stdin once, accepts 12 or more characters and at most 1024 bytes, and stores
only an Argon2id hash using the central production parameters (19 MiB, two
iterations, one lane). Usernames are trimmed, lowercased and restricted to 1–64
ASCII letters, digits, `.`, `_` or `-`, with alphanumeric endpoints.

One transaction creates the `default` organization, `default` project, local
user, organization-level `owner` membership and one redacted
`admin.bootstrap` audit event. Engine-specific serialization (PostgreSQL
transaction advisory lock; SQLite `BEGIN IMMEDIATE`) makes concurrent attempts
safe. An exact repeat returns `already_present` with the original identifiers;
a different username or password returns a conflict without changing data.

Stable exit codes:

| Code | Meaning |
|---:|---|
| 0 | created, already present, migration completed, or clean shutdown |
| 3 | CLI, username, password or stdin validation failure |
| 5 | existing bootstrap conflicts with supplied identity data |
| 10 | configuration, database, migration, serialization or network infrastructure failure |

With `--output json`, stdout contains one JSON object and diagnostics go only to
stderr. Neither output contains the password, password hash or database URL.
