#![forbid(unsafe_code)]

mod common;

use std::{env, error::Error, io};

use sqlx::{PgPool, Row};
use takt_application::{BootstrapService, BootstrapStatus};
use takt_persistence::{Database, DatabaseConfig, ReadinessError, SchemaStatus, SqlxRepository};

use common::{
    FixedClock, SequenceIds, TEST_PASSWORD, TEST_RAW_RECOVERY_TOKEN, TEST_RAW_SESSION_TOKEN,
    TestPasswordHasher,
};

fn test_url() -> Result<String, Box<dyn Error>> {
    env::var("TAKT_TEST_POSTGRES_URL").map_err(|_| {
        io::Error::other(
            "TAKT_TEST_POSTGRES_URL is required for the real PostgreSQL contract suite",
        )
        .into()
    })
}

async fn reset_test_database(url: &str) -> Result<(), Box<dyn Error>> {
    let pool = PgPool::connect(url).await?;
    let database_name: String = sqlx::query("SELECT current_database() AS database_name")
        .fetch_one(&pool)
        .await?
        .try_get("database_name")?;
    if !database_name.starts_with("takt_test") {
        return Err(io::Error::other(
            "refusing to reset a PostgreSQL database whose name does not start with 'takt_test'",
        )
        .into());
    }
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE SCHEMA public").execute(&pool).await?;
    pool.close().await;
    Ok(())
}

async fn connect(url: &str) -> Result<Database, Box<dyn Error>> {
    let config = DatabaseConfig::postgresql_for_test(url.to_owned())?;
    Ok(Database::connect(&config).await?)
}

async fn assert_idempotency_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    let actor = common::resource_id(3)?.as_uuid();
    sqlx::query("INSERT INTO api_token_idempotency (actor_type,actor_id,method,path,idempotency_key,request_hash,created_at,expires_at) VALUES ('system',$1,'POST','/api/v1/api-tokens','schema-key-valid',decode(repeat('11',32),'hex'),TIMESTAMPTZ '2026-07-22 00:00:00Z',TIMESTAMPTZ '2026-07-23 00:00:00Z')")
        .bind(actor).execute(pool).await?;
    assert!(sqlx::query("INSERT INTO api_token_idempotency (actor_type,actor_id,method,path,idempotency_key,request_hash,replay_key_version,replay_nonce,replay_ciphertext,created_at,expires_at) VALUES ('system',$1,'POST','/api/v1/api-tokens','schema-key-partial',decode(repeat('11',32),'hex'),1,decode(repeat('22',12),'hex'),decode(repeat('33',17),'hex'),TIMESTAMPTZ '2026-07-22 00:00:00Z',TIMESTAMPTZ '2026-07-23 00:00:00Z')")
        .bind(actor).execute(pool).await.is_err());
    assert!(sqlx::query("INSERT INTO api_token_idempotency (actor_type,actor_id,method,path,idempotency_key,request_hash,created_at,expires_at) VALUES ('system',$1,'POST','/api/v1/api-tokens','schema-key-expiry',decode(repeat('11',32),'hex'),TIMESTAMPTZ '2026-07-22 00:00:00Z',TIMESTAMPTZ '2026-07-23 00:00:00.000001Z')")
        .bind(actor).execute(pool).await.is_err());
    assert!(sqlx::query("INSERT INTO api_token_idempotency (actor_type,actor_id,method,path,idempotency_key,request_hash,replay_key_version,replay_nonce,replay_ciphertext,created_at,expires_at) VALUES ('system',$1,'PATCH','/api/v1/api-tokens/019b3cf0-0000-7000-8000-000000000001','schema-key-patch',decode(repeat('11',32),'hex'),1,decode(repeat('22',12),'hex'),decode(repeat('33',17),'hex'),TIMESTAMPTZ '2026-07-22 00:00:00Z',TIMESTAMPTZ '2026-07-23 00:00:00Z')")
        .bind(actor).execute(pool).await.is_err());
    assert_eq!(sqlx::query("SELECT COUNT(*) AS count FROM information_schema.columns WHERE table_name='api_token_idempotency' AND column_name LIKE '%plaintext%'")
        .fetch_one(pool).await?.try_get::<i64, _>("count")?, 0);
    Ok(())
}

// PRD-DATA-001 / PRD-DATA-002 / PRD-DATA-004 / PRD-NFR-002 / PRD-IAM-001:
// this test requires a real PostgreSQL 16+ service; it is intentionally not
// skipped when the service configuration is absent.
#[tokio::test]
async fn postgres_migrations_repository_and_bootstrap_contracts() -> Result<(), Box<dyn Error>> {
    let url = test_url()?;
    reset_test_database(&url).await?;
    let database = connect(&url).await?;
    assert_eq!(
        database.schema_status().await?,
        SchemaStatus::MigrationRequired { found: 0 }
    );
    database.migrate().await?;
    database.migrate().await?;
    let raw = PgPool::connect(&url).await?;
    assert_idempotency_schema(&raw).await?;
    let server_version: i64 = sqlx::query("SHOW server_version_num")
        .fetch_one(&raw)
        .await?
        .try_get::<String, _>(0)?
        .parse()?;
    assert!(server_version >= 160_000);
    common::run_repository_contract(&SqlxRepository::new(database.clone())).await?;
    common::run_session_repository_contract(&SqlxRepository::new(database.clone())).await?;
    common::run_recovery_repository_contract(&SqlxRepository::new(database.clone())).await?;
    common::run_api_token_repository_contract(&SqlxRepository::new(database.clone())).await?;
    common::run_api_token_lifecycle_contract(&SqlxRepository::new(database.clone())).await?;
    common::run_api_token_create_idempotency_contract(&SqlxRepository::new(database.clone()))
        .await?;
    common::run_browser_authentication_contract(&SqlxRepository::new(database.clone())).await?;
    let row = sqlx::query(
        "SELECT string_agg(token_digest || ' ' || csrf_digest, ' ') AS stored, (SELECT string_agg(metadata::text, ' ') FROM audit_events WHERE resource_type = 'session') AS metadata FROM sessions",
    )
        .fetch_one(&raw)
        .await?;
    common::assert_persisted_session_is_redacted(
        &row.try_get::<String, _>("stored")?,
        &row.try_get::<String, _>("metadata")?,
    );
    assert!(
        sqlx::query("UPDATE sessions SET token_digest = $1")
            .bind(TEST_RAW_SESSION_TOKEN)
            .execute(&raw)
            .await
            .is_err(),
        "PostgreSQL must reject non-digest session values"
    );
    let recovery_row = sqlx::query(
        "SELECT string_agg(token_digest, ' ') AS stored, (SELECT string_agg(metadata::text, ' ') FROM audit_events WHERE resource_type = 'recovery_token') AS metadata FROM recovery_tokens",
    )
    .fetch_one(&raw)
    .await?;
    common::assert_persisted_recovery_is_redacted(
        &recovery_row.try_get::<String, _>("stored")?,
        &recovery_row.try_get::<String, _>("metadata")?,
    );
    assert!(
        sqlx::query("UPDATE recovery_tokens SET token_digest = $1")
            .bind(TEST_RAW_RECOVERY_TOKEN)
            .execute(&raw)
            .await
            .is_err(),
        "PostgreSQL must reject non-digest recovery values"
    );
    let token_row = sqlx::query(
        "SELECT string_agg(token_hash, ' ') AS stored, (SELECT string_agg(metadata::text, ' ') FROM audit_events WHERE resource_type = 'api_token') AS metadata FROM api_tokens",
    )
    .fetch_one(&raw)
    .await?;
    common::assert_persisted_api_tokens_are_redacted(
        &token_row.try_get::<String, _>("stored")?,
        &token_row.try_get::<String, _>("metadata")?,
    );
    assert!(
        sqlx::query("UPDATE api_tokens SET token_hash = $1")
            .bind(TEST_RAW_SESSION_TOKEN)
            .execute(&raw)
            .await
            .is_err(),
        "PostgreSQL must reject non-Argon2 API-token hashes"
    );
    let credential: String =
        sqlx::query("SELECT password_hash FROM local_credentials WHERE user_id = $1")
            .bind(common::resource_id(3)?.as_uuid())
            .fetch_one(&raw)
            .await?
            .try_get("password_hash")?;
    common::assert_replacement_password_hash(&credential)?;
    let audit_id = common::resource_id(5)?;
    assert!(
        sqlx::query("UPDATE audit_events SET action = 'mutated' WHERE id = $1")
            .bind(audit_id.as_uuid())
            .execute(&raw)
            .await
            .is_err(),
        "audit rows must be immutable in PostgreSQL"
    );
    raw.close().await;

    let (url_prefix, _) = url
        .rsplit_once('/')
        .ok_or_else(|| io::Error::other("PostgreSQL test URL has no database segment"))?;
    let admin = PgPool::connect(&format!("{url_prefix}/postgres")).await?;
    sqlx::query("ALTER DATABASE takt_test WITH ALLOW_CONNECTIONS false")
        .execute(&admin)
        .await?;
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'takt_test' AND pid <> pg_backend_pid()",
    )
    .execute(&admin)
    .await?;
    let outage_readiness = database.readiness().await;
    sqlx::query("ALTER DATABASE takt_test WITH ALLOW_CONNECTIONS true")
        .execute(&admin)
        .await?;
    admin.close().await;
    assert_eq!(outage_readiness, Err(ReadinessError::DatabaseUnavailable));
    database.close().await?;

    reset_test_database(&url).await?;
    let database = connect(&url).await?;
    database.migrate().await?;
    let repository = SqlxRepository::new(database.clone());
    common::run_bootstrap_contract(&repository).await?;
    let raw = PgPool::connect(&url).await?;
    let credential: String = sqlx::query("SELECT password_hash FROM local_credentials")
        .fetch_one(&raw)
        .await?
        .try_get("password_hash")?;
    assert!(credential.starts_with("$argon2id$"));
    assert!(!credential.contains(TEST_PASSWORD));
    let metadata: serde_json::Value = sqlx::query("SELECT metadata FROM audit_events")
        .fetch_one(&raw)
        .await?
        .try_get("metadata")?;
    assert!(!metadata.to_string().contains(TEST_PASSWORD));
    assert!(!metadata.to_string().contains("argon2"));
    raw.close().await;
    database.close().await?;

    reset_test_database(&url).await?;
    let database = connect(&url).await?;
    database.migrate().await?;
    let raw = PgPool::connect(&url).await?;
    sqlx::query(
        "CREATE FUNCTION fail_bootstrap_membership() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled failure'; END; $$",
    )
    .execute(&raw)
    .await?;
    sqlx::query(
        "CREATE TRIGGER fail_bootstrap_membership BEFORE INSERT ON memberships FOR EACH ROW EXECUTE FUNCTION fail_bootstrap_membership()",
    )
    .execute(&raw)
    .await?;
    let repository = SqlxRepository::new(database.clone());
    let hasher = TestPasswordHasher::new();
    let clock = FixedClock;
    let ids = SequenceIds::new(9_000);
    let service = BootstrapService::new(&repository, &hasher, &clock, &ids);
    assert!(service.execute("admin", TEST_PASSWORD).await.is_err());
    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM organizations) AS organizations, (SELECT COUNT(*) FROM projects) AS projects, (SELECT COUNT(*) FROM users) AS users, (SELECT COUNT(*) FROM local_credentials) AS local_credentials, (SELECT COUNT(*) FROM memberships) AS memberships, (SELECT COUNT(*) FROM audit_events) AS audit_events",
    )
    .fetch_one(&raw)
    .await?;
    for column in [
        "organizations",
        "projects",
        "users",
        "local_credentials",
        "memberships",
        "audit_events",
    ] {
        assert_eq!(
            row.try_get::<i64, _>(column)?,
            0,
            "{column} must be rolled back"
        );
    }
    raw.close().await;
    database.close().await?;

    reset_test_database(&url).await?;
    let database = connect(&url).await?;
    database.migrate().await?;
    let repository = SqlxRepository::new(database.clone());
    let hasher = TestPasswordHasher::new();
    let clock = FixedClock;
    let ids = SequenceIds::new(10_000);
    let service = BootstrapService::new(&repository, &hasher, &clock, &ids);
    let (left, right) = tokio::join!(
        service.execute("admin", TEST_PASSWORD),
        service.execute("admin", TEST_PASSWORD)
    );
    let left = left?;
    let right = right?;
    assert!(matches!(
        (left.status, right.status),
        (BootstrapStatus::Created, BootstrapStatus::AlreadyPresent)
            | (BootstrapStatus::AlreadyPresent, BootstrapStatus::Created)
    ));
    let raw = PgPool::connect(&url).await?;
    assert_eq!(
        sqlx::query("SELECT COUNT(*) AS count FROM users")
            .fetch_one(&raw)
            .await?
            .try_get::<i64, _>("count")?,
        1
    );
    sqlx::query("UPDATE _sqlx_migrations SET version = 6 WHERE version = 5")
        .execute(&raw)
        .await?;
    assert_eq!(
        database.schema_status().await?,
        SchemaStatus::TooNew {
            found: 6,
            supported: 5
        }
    );
    assert!(database.migrate().await.is_err());
    raw.close().await;
    common::run_shutdown_contract(database).await
}
